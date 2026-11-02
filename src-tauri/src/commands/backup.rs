use std::path::{Path, PathBuf};

use codexmart_core::backup::{self, BackupCheck, BackupFile, HistoryEntry};
use codexmart_core::services::{audit, settings};
use codexmart_core::{AppError, Ctx, Permission};
use serde::Serialize;
use tauri::State;

use super::CmdResult;
use crate::backup_jobs::{self, BackupRun, RestoreReport};
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    pub backup_dir: String,
    pub secondary_dir: String,
    pub secondary_available: bool,
    pub auto_enabled: bool,
    pub interval_hours: i64,
    pub retention: i64,
    pub last_success_at: Option<String>,
    /// True when every backup copy is on the same drive as the database.
    pub same_drive_only: bool,
    pub history: Vec<HistoryEntry>,
    pub files: Vec<BackupFile>,
}

#[tauri::command]
pub async fn backup_status(state: State<'_, AppState>) -> CmdResult<BackupStatus> {
    // Backups stay available when activation is required: shop data must
    // always be safe to copy.
    let s = state.run_always(|c, ctx| {
        ctx.require(Permission::CreateBackup)?;
        settings::load(c)
    })?;
    let secondary = s.backup_secondary_dir.trim().to_string();
    let secondary_available = !secondary.is_empty() && Path::new(&secondary).is_dir();
    let same_drive_only = backup_jobs::same_drive(&state, &state.paths.backup_dir)
        && (secondary.is_empty() || backup_jobs::same_drive(&state, Path::new(&secondary)));
    Ok(BackupStatus {
        backup_dir: state.paths.backup_dir.to_string_lossy().to_string(),
        secondary_dir: secondary,
        secondary_available,
        auto_enabled: s.backup_auto_enabled,
        interval_hours: s.backup_interval_hours,
        retention: s.backup_retention,
        last_success_at: backup::last_success(&state.paths.history_file),
        same_drive_only,
        history: backup::history_read(&state.paths.history_file, 50),
        files: backup::list_files(&state.paths.backup_dir),
    })
}

/// Backs up now to the default folder, or to `dest_dir` (e.g. a USB drive).
#[tauri::command]
pub async fn backup_create(state: State<'_, AppState>, dest_dir: Option<String>) -> CmdResult<BackupRun> {
    let actor = state.session().ok_or(AppError::Unauthenticated)?;
    Ctx::new(actor.clone()).require(Permission::CreateBackup)?;
    let dest = dest_dir.filter(|d| !d.trim().is_empty()).map(PathBuf::from);
    let run = backup_jobs::run_backup(&state, "manual", dest)?;
    let ctx = Ctx::new(actor);
    let _ = state.with_conn(|c| {
        audit::record(
            c,
            &ctx,
            "backup.created",
            "backup",
            None,
            format!("Manual backup saved to {}", run.result.path),
            None,
        )
    });
    Ok(run)
}

#[tauri::command]
pub async fn backup_list_folder(state: State<'_, AppState>, dir: String) -> CmdResult<Vec<BackupFile>> {
    state.session().ok_or(AppError::Unauthenticated)?;
    Ok(backup::list_files(Path::new(&dir)))
}

/// Checks a backup file without changing anything. Allowed without signing
/// in only while the database cannot be opened (recovery mode).
#[tauri::command]
pub async fn backup_validate(state: State<'_, AppState>, path: String) -> CmdResult<BackupCheck> {
    if state.db_error().is_none() {
        let actor = state.session().ok_or(AppError::Unauthenticated)?;
        Ctx::new(actor).require(Permission::CreateBackup)?;
    }
    Ok(backup::validate_file(Path::new(&path), &state.paths.work_dir))
}

/// Replaces all current data with the backup. Requires an administrator,
/// or recovery mode when the current database cannot be opened.
#[tauri::command]
pub async fn backup_restore(state: State<'_, AppState>, path: String, confirm: String) -> CmdResult<RestoreReport> {
    if confirm != "RESTORE" {
        return Err(AppError::validation("Type RESTORE to confirm."));
    }
    let actor = state.session();
    if state.db_error().is_none() {
        let a = actor.clone().ok_or(AppError::Unauthenticated)?;
        Ctx::new(a).require(Permission::RestoreBackup)?;
    }
    let report = backup_jobs::restore(&state, Path::new(&path))?;
    let ctx = actor.map(Ctx::new).unwrap_or_else(Ctx::system);
    let _ = state.with_conn(|c| {
        audit::record(
            c,
            &ctx,
            "backup.restored",
            "backup",
            None,
            format!("Data restored from backup {} (made {})", report.restored_from, report.backup_created_at),
            None,
        )
    });
    Ok(report)
}
