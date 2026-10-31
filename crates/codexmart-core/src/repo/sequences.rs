//! Gap-free document numbering. Numbers are taken inside the same
//! transaction as the document, so a rolled-back sale never burns a number.

use rusqlite::{Connection, OptionalExtension};

use crate::error::{AppError, AppResult};

pub fn next_value(conn: &Connection, name: &str) -> AppResult<(String, i64, usize)> {
    let row: Option<(String, i64, i64)> = conn
        .query_row("SELECT prefix, next_value, padding FROM sequences WHERE name = ?1", [name], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?;
    let (prefix, value, padding) =
        row.ok_or_else(|| AppError::Internal(format!("Document numbering '{name}' is not set up.")))?;
    conn.execute("UPDATE sequences SET next_value = next_value + 1 WHERE name = ?1", [name])?;
    Ok((prefix, value, padding.max(1) as usize))
}

/// Next formatted document number, e.g. `INV-000123`.
pub fn next(conn: &Connection, name: &str) -> AppResult<String> {
    let (prefix, value, padding) = next_value(conn, name)?;
    Ok(format!("{prefix}{value:0padding$}"))
}

pub fn prefix(conn: &Connection, name: &str) -> AppResult<String> {
    Ok(conn.query_row("SELECT prefix FROM sequences WHERE name = ?1", [name], |r| r.get(0))?)
}

pub fn set_prefix(conn: &Connection, name: &str, prefix: &str) -> AppResult<()> {
    conn.execute("UPDATE sequences SET prefix = ?2 WHERE name = ?1", [name, prefix])?;
    Ok(())
}
