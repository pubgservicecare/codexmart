//! Versioned, checksummed schema migrations.
//!
//! Migrations are embedded in the binary and applied in order, each inside
//! its own transaction. Applied migrations are recorded with a checksum so
//! an edited migration is detected instead of silently diverging. A database
//! written by a newer version of the app is refused rather than modified.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration { version: 1, name: "initial_schema", sql: include_str!("migrations/0001_initial_schema.sql") },
    Migration { version: 2, name: "reference_data", sql: include_str!("migrations/0002_reference_data.sql") },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationReport {
    pub from_version: i64,
    pub to_version: i64,
    pub applied: Vec<String>,
}

pub fn latest_version() -> i64 {
    MIGRATIONS.last().map(|m| m.version).unwrap_or(0)
}

/// Checksum over the SQL with normalised line endings, so a CRLF checkout
/// produces the same value as an LF one.
pub fn checksum(sql: &str) -> String {
    let normalised = sql.replace("\r\n", "\n");
    hex::encode(Sha256::digest(normalised.as_bytes()))
}

fn ensure_table(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            checksum   TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );",
    )?;
    Ok(())
}

/// Highest applied version, or 0 for a new database.
pub fn current_version(conn: &Connection) -> AppResult<i64> {
    let exists: Option<String> = conn
        .query_row("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'", [], |r| {
            r.get(0)
        })
        .optional()?;
    if exists.is_none() {
        return Ok(0);
    }
    Ok(conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))?)
}

pub fn pending_count(conn: &Connection) -> AppResult<usize> {
    let current = current_version(conn)?;
    Ok(MIGRATIONS.iter().filter(|m| m.version > current).count())
}

fn verify_applied(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare("SELECT version, checksum FROM schema_migrations ORDER BY version")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (version, stored) = row?;
        match MIGRATIONS.iter().find(|m| m.version == version) {
            Some(m) if checksum(m.sql) == stored => {}
            Some(m) => {
                return Err(AppError::Database(format!(
                    "Database migration {version} ({}) does not match this version of the app. \
                     The database was not changed. Restore a backup or reinstall the matching version.",
                    m.name
                )))
            }
            None if version > latest_version() => {}
            None => {
                return Err(AppError::Database(format!(
                    "Unknown database migration {version}. The database was not changed."
                )))
            }
        }
    }
    Ok(())
}

/// Applies all pending migrations up to `target` (inclusive).
pub fn migrate_to(conn: &mut Connection, target: i64, now: &str) -> AppResult<MigrationReport> {
    ensure_table(conn)?;
    let from = current_version(conn)?;
    if from > latest_version() {
        return Err(AppError::Database(format!(
            "This database was created by a newer version of CODE X MART (schema {from}, \
             this app supports up to {}). Please install the latest version. The database was not changed.",
            latest_version()
        )));
    }
    verify_applied(conn)?;

    let mut applied = Vec::new();
    for m in MIGRATIONS.iter().filter(|m| m.version > from && m.version <= target) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)
            .map_err(|e| AppError::Database(format!("Migration {} ({}) failed: {e}", m.version, m.name)))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
            params![m.version, m.name, checksum(m.sql), now],
        )?;
        tx.commit()?;
        log::info!("Applied database migration {} ({})", m.version, m.name);
        applied.push(format!("{:04}_{}", m.version, m.name));
    }
    Ok(MigrationReport { from_version: from, to_version: current_version(conn)?, applied })
}

pub fn migrate(conn: &mut Connection, now: &str) -> AppResult<MigrationReport> {
    migrate_to(conn, latest_version(), now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_versions_are_sequential() {
        for (i, m) in MIGRATIONS.iter().enumerate() {
            assert_eq!(m.version, i as i64 + 1);
        }
    }

    #[test]
    fn checksum_ignores_line_endings() {
        assert_eq!(checksum("a;\nb;\n"), checksum("a;\r\nb;\r\n"));
    }
}
