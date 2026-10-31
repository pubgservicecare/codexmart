//! Connection setup, integrity checks and database metadata.

pub mod migrations;

use std::path::Path;
use std::time::Duration;

use rusqlite::OptionalExtension;

/// Re-exported so the desktop shell does not depend on rusqlite directly.
pub use rusqlite::Connection;

use crate::error::{AppError, AppResult};
use crate::ids::new_id;

/// Marker stored in `app_meta` so that restore can refuse foreign files.
pub const APP_MARKER: &str = "CODE X MART";

pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    configure(&conn, true)?;
    Ok(conn)
}

pub fn open_in_memory() -> AppResult<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn, false)?;
    Ok(conn)
}

fn configure(conn: &Connection, on_disk: bool) -> AppResult<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    if on_disk {
        let _mode: String = conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
        // FULL: a committed sale survives a power cut, not just a crash.
        conn.pragma_update(None, "synchronous", "FULL")?;
    }
    Ok(())
}

/// Fast structural check run on every start-up.
pub fn quick_check(conn: &Connection) -> AppResult<()> {
    let result: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if result == "ok" {
        Ok(())
    } else {
        Err(AppError::Database(format!("The database failed its integrity check: {result}")))
    }
}

/// Full integrity + foreign-key check. Returns the list of problems found.
pub fn integrity_report(conn: &Connection) -> AppResult<Vec<String>> {
    let mut problems = Vec::new();
    {
        let mut stmt = conn.prepare("PRAGMA integrity_check")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for row in rows {
            let line = row?;
            if line != "ok" {
                problems.push(line);
            }
        }
    }
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let rows = stmt.query_map([], |r| {
        Ok(format!("Broken link in table {} (row {})", r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    for row in rows {
        problems.push(row?);
    }
    Ok(problems)
}

pub fn meta_get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT value FROM app_meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

pub fn meta_set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

/// Identity of this database and of the device that owns it. Created once
/// and never changed; future synchronisation uses `device_id`.
pub fn ensure_app_meta(conn: &Connection, now: &str) -> AppResult<()> {
    let defaults = [
        ("app", APP_MARKER.to_string()),
        ("db_id", new_id()),
        ("device_id", new_id()),
        ("created_at", now.to_string()),
    ];
    for (key, value) in defaults {
        conn.execute("INSERT OR IGNORE INTO app_meta (key, value) VALUES (?1, ?2)", [key, value.as_str()])?;
    }
    Ok(())
}

/// True when the file holds data written by CODE X MART.
pub fn is_codexmart_db(conn: &Connection) -> bool {
    matches!(meta_get(conn, "app"), Ok(Some(v)) if v == APP_MARKER)
}
