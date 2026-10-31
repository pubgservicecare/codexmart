//! Backup scheduling, secondary-drive copies and restore orchestration.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use chrono::{DateTime, Utc};
use codexmart_core::backup::{self, BackupResult};
use codexmart_core::db::migrations;
use codexmart_core::services::settings::{self, Settings};
use codexmart_core::{AppError, AppResult};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::state::{record_history, AppState};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRun {
    #[serde(flatten)]
    pub result: BackupResult,
    /// Outcome of copying to the secondary folder (USB / second drive).
    pub secondary: Option<String>,
}

fn drive_of(p: &Path) -> String {
    p.to_string_lossy().chars().take(2).collect::<String>().to_uppercase()
}

/// True when the folder is on the same drive as the live database.
pub fn same_drive(state: &AppState, dir: &Path) -> bool {
    drive_of(dir) == drive_of(&state.paths.db_file)
}

/// Creates a backup of `kind` in `dest` (default: the app's backup folder).
/// Automatic backups are also copied to the secondary folder and pruned.
pub fn run_backup(state: &AppState, kind: &str, dest: Option<PathBuf>) -> AppResult<BackupRun> {
    let dest_dir = dest.unwrap_or_else(|| state.paths.backup_dir.clone());
    let mut cfg = Settings::default();
    let result = state.with_conn(|conn| {
        cfg = settings::load(conn)?;
        backup::create_backup(conn, &dest_dir, &state.paths.work_dir, kind, Some(cfg.store_name.clone()), Utc::now())
    });
    record_history(
        &state.paths,
        "backup",
        kind,
        &result.as_ref().map(|r| (r.path.clone(), r.size_bytes)).map_err(|e| e.to_string()),
    );
    let result = result?;
    log::info!("Backup created: {}", result.path);

    let mut secondary = None;
    if kind == "auto" {
        backup::prune_auto(&dest_dir, cfg.backup_retention as usize);
        let second = cfg.backup_secondary_dir.trim();
        if !second.is_empty() {
            let outcome = copy_to_secondary(Path::new(&result.path), Path::new(second), cfg.backup_retention as usize);
            record_history(&state.paths, "copy", "secondary", &outcome);
            secondary = Some(match outcome {
                Ok((p, _)) => format!("Copied to {p}"),
                Err(e) => {
                    log::warn!("Secondary backup copy failed: {e}");
                    format!("Not copied: {e}")
                }
            });
        }
    }
    Ok(BackupRun { result, secondary })
}

fn copy_to_secondary(file: &Path, dir: &Path, keep: usize) -> Result<(String, u64), String> {
    if !dir.exists() {
        return Err(format!("{} is not connected or does not exist", dir.display()));
    }
    let target = dir.join(file.file_name().ok_or("bad file name")?);
    let tmp = target.with_extension("cxmbak.part");
    std::fs::copy(file, &tmp).map_err(|e| e.to_string())?;
    let size = std::fs::metadata(&tmp).map_err(|e| e.to_string())?.len();
    if size != std::fs::metadata(file).map_err(|e| e.to_string())?.len() {
        let _ = std::fs::remove_file(&tmp);
        return Err("the copy is incomplete".into());
    }
    std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    backup::prune_auto(dir, keep);
    Ok((target.to_string_lossy().to_string(), size))
}

fn last_success_age(state: &AppState) -> Option<chrono::Duration> {
    backup::last_success(&state.paths.history_file)
        .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
        .map(|t| Utc::now() - t.with_timezone(&Utc))
}

/// Runs an automatic backup if one is due.
pub fn auto_backup_if_due(state: &AppState) {
    if state.db_error().is_some() {
        return;
    }
    let Ok(cfg) = state.with_conn(|c| settings::load(c)) else { return };
    if !cfg.backup_auto_enabled {
        return;
    }
    let due = match last_success_age(state) {
        Some(age) => age >= chrono::Duration::hours(cfg.backup_interval_hours),
        None => true,
    };
    if due {
        if let Err(e) = run_backup(state, "auto", None) {
            log::error!("Automatic backup failed: {e}");
        }
    }
}

/// Background thread that checks every 10 minutes whether a backup is due.
pub fn start_scheduler(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        loop {
            auto_backup_if_due(&app.state::<AppState>());
            std::thread::sleep(Duration::from_secs(600));
        }
    });
}

/// Called when the main window closes.
pub fn backup_on_exit(state: &AppState) {
    if state.db_error().is_some() || !state.had_session.load(Ordering::Relaxed) {
        return;
    }
    let enabled = state.with_conn(|c| settings::load(c)).map(|s| s.backup_on_exit).unwrap_or(false);
    let recent = last_success_age(state).map(|a| a < chrono::Duration::minutes(10)).unwrap_or(false);
    if enabled && !recent {
        if let Err(e) = run_backup(state, "auto", None) {
            log::error!("Backup on exit failed: {e}");
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub restored_from: String,
    pub safety_backup: Option<String>,
    pub backup_created_at: String,
}

/// Restores `file` over the live database after validating it and taking a
/// safety copy of the current data.
pub fn restore(state: &AppState, file: &Path) -> AppResult<RestoreReport> {
    let validated = backup::inspect(file, &state.paths.work_dir, migrations::latest_version())?;
    let safety = if state.db_error().is_none() {
        let run = run_backup(state, "pre-restore", None).map_err(|e| {
            AppError::Backup(format!("Restore cancelled: a safety backup of the current data could not be made ({e})."))
        })?;
        Some(run.result.path)
    } else {
        // The current file cannot be opened; keep a raw copy of it anyway.
        let stamp = Utc::now().format("%Y%m%d-%H%M%S");
        let copy = state.paths.backup_dir.join(format!("damaged-database-{stamp}.db"));
        std::fs::copy(&state.paths.db_file, &copy).ok().map(|_| copy.to_string_lossy().to_string())
    };
    let created_at = validated.manifest.created_at.clone();
    let outcome = state.replace_database(|target| backup::install(&validated, target));
    record_history(
        &state.paths,
        "restore",
        "restore",
        &outcome.as_ref().map(|_| (file.to_string_lossy().to_string(), 0)).map_err(|e| e.to_string()),
    );
    outcome?;
    log::warn!("Database restored from {}", file.display());
    Ok(RestoreReport {
        restored_from: file.to_string_lossy().to_string(),
        safety_backup: safety,
        backup_created_at: created_at,
    })
}
