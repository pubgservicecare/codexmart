//! Application state: data folder locations, the database connection and
//! the signed-in user.
//!
//! A single SQLite connection is shared behind a mutex. CODE X MART runs on
//! one till, so serialising access is simple and removes any chance of two
//! operations interleaving.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use chrono::Utc;
use codexmart_core::backup::{self, HistoryEntry};
use codexmart_core::context::timestamp;
use codexmart_core::db::{self, migrations, Connection};
use codexmart_core::services::settings;
use codexmart_core::{Actor, AppError, AppResult, Ctx};
use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    /// Root data folder, e.g. C:\Users\<name>\AppData\Roaming\CODE X MART
    pub root: PathBuf,
    pub db_file: PathBuf,
    pub backup_dir: PathBuf,
    pub work_dir: PathBuf,
    pub history_file: PathBuf,
    pub log_dir: PathBuf,
}

impl AppPaths {
    /// Data lives in the user's AppData folder, never in the installation
    /// folder, so it survives updates and reinstalls. `CODEXMART_DATA_DIR`
    /// overrides the location (used for testing).
    pub fn resolve(app: &AppHandle) -> Result<Self, String> {
        let root = match std::env::var("CODEXMART_DATA_DIR") {
            Ok(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
            _ => app.path().data_dir().map_err(|e| e.to_string())?.join("CODE X MART"),
        };
        let paths = AppPaths {
            db_file: root.join("data").join("codexmart.db"),
            backup_dir: root.join("backups"),
            work_dir: root.join("temp"),
            history_file: root.join("backup-history.jsonl"),
            log_dir: app.path().app_log_dir().unwrap_or_else(|_| root.join("logs")),
            root,
        };
        for dir in [paths.db_file.parent().unwrap(), &paths.backup_dir, &paths.work_dir] {
            std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
        }
        Ok(paths)
    }
}

pub struct AppState {
    pub paths: AppPaths,
    db: Mutex<Option<Connection>>,
    db_error: Mutex<Option<String>>,
    session: Mutex<Option<Actor>>,
    /// Set once someone signs in; used to decide on a backup at exit.
    pub had_session: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic while holding the lock leaves the connection usable (any open
    // transaction was rolled back during unwinding), so recover the guard.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn record_history(paths: &AppPaths, action: &str, kind: &str, result: &Result<(String, u64), String>) {
    let entry = HistoryEntry {
        at: timestamp(Utc::now()),
        action: action.into(),
        kind: kind.into(),
        status: if result.is_ok() { "success" } else { "failed" }.into(),
        path: result.as_ref().ok().map(|r| r.0.clone()),
        size_bytes: result.as_ref().ok().map(|r| r.1),
        message: result.as_ref().err().cloned().unwrap_or_default(),
    };
    if let Err(e) = backup::history_append(&paths.history_file, &entry) {
        log::warn!("Could not write backup history: {e}");
    }
}

/// Opens (and if needed upgrades) the database. Never creates a fresh
/// database over an existing file that fails to open: errors are returned
/// so the user can restore a backup instead.
pub fn open_database(paths: &AppPaths) -> AppResult<Connection> {
    let existed = std::fs::metadata(&paths.db_file).map(|m| m.len() > 0).unwrap_or(false);
    let mut conn = db::open(&paths.db_file)?;
    db::quick_check(&conn)?;
    let current = migrations::current_version(&conn)?;
    if existed && current > 0 && !db::is_codexmart_db(&conn) {
        return Err(AppError::Database("The data file does not belong to CODE X MART.".into()));
    }
    if current > 0 && migrations::pending_count(&conn)? > 0 {
        // Safety copy before changing the structure of existing data.
        let store = settings::load(&conn).ok().map(|s| s.store_name);
        let result = backup::create_backup(&conn, &paths.backup_dir, &paths.work_dir, "pre-upgrade", store, Utc::now());
        record_history(
            paths,
            "backup",
            "pre-upgrade",
            &result.as_ref().map(|r| (r.path.clone(), r.size_bytes)).map_err(|e| e.to_string()),
        );
        result.map_err(|e| AppError::Database(format!("Upgrade stopped: the safety backup failed ({e}).")))?;
    }
    let report = migrations::migrate(&mut conn, &timestamp(Utc::now()))?;
    if !report.applied.is_empty() {
        log::info!("Database upgraded from schema {} to {}", report.from_version, report.to_version);
    }
    db::ensure_app_meta(&conn, &timestamp(Utc::now()))?;
    Ok(conn)
}

impl AppState {
    pub fn initialize(paths: AppPaths) -> Self {
        let (conn, error) = match open_database(&paths) {
            Ok(c) => (Some(c), None),
            Err(e) => {
                log::error!("Database could not be opened: {e}");
                (None, Some(e.to_string()))
            }
        };
        AppState {
            paths,
            db: Mutex::new(conn),
            db_error: Mutex::new(error),
            session: Mutex::new(None),
            had_session: AtomicBool::new(false),
        }
    }

    pub fn db_error(&self) -> Option<String> {
        lock(&self.db_error).clone()
    }

    pub fn session(&self) -> Option<Actor> {
        lock(&self.session).clone()
    }

    pub fn set_session(&self, actor: Option<Actor>) {
        if actor.is_some() {
            self.had_session.store(true, Ordering::Relaxed);
        }
        *lock(&self.session) = actor;
    }

    /// Runs `f` with the database connection, without a signed-in user.
    pub fn with_conn<T>(&self, f: impl FnOnce(&mut Connection) -> AppResult<T>) -> AppResult<T> {
        let mut guard = lock(&self.db);
        let conn = guard.as_mut().ok_or_else(|| {
            AppError::Database(self.db_error().unwrap_or_else(|| "The database is not available.".into()))
        })?;
        f(conn)
    }

    /// Runs `f` as the signed-in user.
    pub fn run<T>(&self, f: impl FnOnce(&mut Connection, &Ctx) -> AppResult<T>) -> AppResult<T> {
        let actor = self.session().ok_or(AppError::Unauthenticated)?;
        let ctx = Ctx::new(actor);
        self.with_conn(|conn| f(conn, &ctx))
    }

    /// Closes the database, swaps the file via `replace`, then reopens it.
    /// If anything fails the previous file (or error state) is reopened.
    pub fn replace_database(&self, replace: impl FnOnce(&Path) -> AppResult<()>) -> AppResult<()> {
        let mut guard = lock(&self.db);
        *guard = None; // closes the connection
        let outcome = replace(&self.paths.db_file);
        match open_database(&self.paths) {
            Ok(conn) => {
                *guard = Some(conn);
                *lock(&self.db_error) = None;
            }
            Err(e) => {
                *lock(&self.db_error) = Some(e.to_string());
                outcome?;
                return Err(e);
            }
        }
        *lock(&self.session) = None;
        outcome
    }

    /// Tries again to open the database after a start-up failure (for example
    /// when the file was briefly locked by antivirus or a sync tool).
    pub fn retry_open(&self) -> AppResult<()> {
        let mut guard = lock(&self.db);
        if guard.is_some() {
            return Ok(());
        }
        match open_database(&self.paths) {
            Ok(conn) => {
                *guard = Some(conn);
                *lock(&self.db_error) = None;
                Ok(())
            }
            Err(e) => {
                *lock(&self.db_error) = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Closes the database cleanly when the app exits: the write-ahead log
    /// is checkpointed into the main file, so the data file is complete on
    /// its own. (Data is safe even without this; SQLite replays the log.)
    pub fn shutdown(&self) {
        if let Some(conn) = lock(&self.db).take() {
            if let Err(e) = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())) {
                log::warn!("Could not checkpoint the database on exit: {e}");
            }
            if let Err((_, e)) = conn.close() {
                log::warn!("Could not close the database cleanly: {e}");
            }
        }
    }
}
