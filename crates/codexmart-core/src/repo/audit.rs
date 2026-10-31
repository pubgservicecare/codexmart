use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection};
use serde::{Deserialize, Serialize};

use super::{like, limit_offset, Page, Where};
use crate::error::AppResult;

#[allow(clippy::too_many_arguments)]
pub fn insert(
    conn: &Connection,
    at: &str,
    user_id: Option<&str>,
    username: &str,
    action: &str,
    entity_type: &str,
    entity_id: Option<&str>,
    summary: &str,
    details: Option<&str>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO audit_log (at, user_id, username, action, entity_type, entity_id, summary, details)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![at, user_id, username, action, entity_type, entity_id, summary, details],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: i64,
    pub at: String,
    pub username: Option<String>,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub summary: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AuditFilter {
    pub search: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn list(conn: &Connection, f: &AuditFilter) -> AppResult<Page<AuditEntry>> {
    let mut w = Where::new();
    w.opt("entity_type = ?", &f.entity_type);
    w.opt("entity_id = ?", &f.entity_id);
    w.opt("substr(at, 1, 10) >= ?", &f.date_from);
    w.opt("substr(at, 1, 10) <= ?", &f.date_to);
    if let Some(s) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        w.repeat(
            "(summary LIKE ? ESCAPE '\\' OR action LIKE ? ESCAPE '\\' OR IFNULL(username,'') LIKE ? ESCAPE '\\')",
            &like(s),
            3,
        );
    }
    let where_sql = w.sql();
    let total: i64 =
        conn.query_row(&format!("SELECT COUNT(*) FROM audit_log{where_sql}"), params_from_iter(w.args.iter()), |r| {
            r.get(0)
        })?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 100);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let mut stmt = conn.prepare(&format!(
        "SELECT id, at, username, action, entity_type, entity_id, summary, details FROM audit_log{where_sql}
         ORDER BY id DESC LIMIT ? OFFSET ?"
    ))?;
    let items = stmt
        .query_map(params_from_iter(args.iter()), |r| {
            Ok(AuditEntry {
                id: r.get(0)?,
                at: r.get(1)?,
                username: r.get(2)?,
                action: r.get(3)?,
                entity_type: r.get(4)?,
                entity_id: r.get(5)?,
                summary: r.get(6)?,
                details: r.get(7)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}
