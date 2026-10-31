use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::context::Role;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    pub full_name: String,
    pub role: Role,
    pub is_active: bool,
    pub last_login_at: Option<String>,
    pub created_at: String,
}

/// Credentials row; never serialised to the UI.
pub struct UserAuth {
    pub user: User,
    pub password_hash: String,
    pub failed_attempts: i64,
    pub locked_until: Option<String>,
}

const SELECT: &str = "SELECT id, username, full_name, role, is_active, last_login_at, created_at, password_hash, \
    failed_attempts, locked_until FROM users";

fn map_auth(r: &Row) -> rusqlite::Result<UserAuth> {
    let role: String = r.get(3)?;
    Ok(UserAuth {
        user: User {
            id: r.get(0)?,
            username: r.get(1)?,
            full_name: r.get(2)?,
            role: Role::parse(&role).unwrap_or(Role::Cashier),
            is_active: r.get(4)?,
            last_login_at: r.get(5)?,
            created_at: r.get(6)?,
        },
        password_hash: r.get(7)?,
        failed_attempts: r.get(8)?,
        locked_until: r.get(9)?,
    })
}

pub fn count(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?)
}

pub fn count_active_admins(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = 1", [], |r| r.get(0))?)
}

pub fn list(conn: &Connection) -> AppResult<Vec<User>> {
    let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY is_active DESC, full_name COLLATE NOCASE"))?;
    let rows = stmt.query_map([], map_auth)?;
    Ok(rows.map(|r| r.map(|a| a.user)).collect::<Result<_, _>>()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<User> {
    Ok(get_auth(conn, id)?.user)
}

pub fn get_auth(conn: &Connection, id: &str) -> AppResult<UserAuth> {
    conn.query_row(&format!("{SELECT} WHERE id = ?1"), [id], map_auth)
        .optional()?
        .ok_or_else(|| AppError::not_found("User"))
}

pub fn find_auth_by_username(conn: &Connection, username: &str) -> AppResult<Option<UserAuth>> {
    Ok(conn
        .query_row(&format!("{SELECT} WHERE username = ?1 COLLATE NOCASE"), [username.trim()], map_auth)
        .optional()?)
}

pub fn insert(
    conn: &Connection,
    id: &str,
    username: &str,
    full_name: &str,
    role: Role,
    hash: &str,
    now: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO users (id, username, full_name, role, password_hash, is_active, failed_attempts, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, 0, ?6, ?6)",
        params![id, username, full_name, role.as_str(), hash, now],
    )?;
    Ok(())
}

pub fn update_profile(
    conn: &Connection,
    id: &str,
    full_name: &str,
    role: Role,
    active: bool,
    now: &str,
) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE users SET full_name = ?2, role = ?3, is_active = ?4, updated_at = ?5 WHERE id = ?1",
        params![id, full_name, role.as_str(), active, now],
    )?;
    if n == 0 {
        return Err(AppError::not_found("User"));
    }
    Ok(())
}

pub fn set_password(conn: &Connection, id: &str, hash: &str, now: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE users SET password_hash = ?2, failed_attempts = 0, locked_until = NULL, updated_at = ?3 WHERE id = ?1",
        params![id, hash, now],
    )?;
    Ok(())
}

pub fn record_login(conn: &Connection, id: &str, now: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE users SET last_login_at = ?2, failed_attempts = 0, locked_until = NULL WHERE id = ?1",
        params![id, now],
    )?;
    Ok(())
}

pub fn record_failure(conn: &Connection, id: &str, attempts: i64, locked_until: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE users SET failed_attempts = ?2, locked_until = ?3 WHERE id = ?1",
        params![id, attempts, locked_until],
    )?;
    Ok(())
}
