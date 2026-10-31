use rusqlite::Connection;
use serde_json::Value;

use crate::context::{Ctx, Permission};
use crate::error::AppResult;
use crate::repo::audit::{self as repo, AuditEntry, AuditFilter};
use crate::repo::Page;

/// Appends to the audit log. Always called inside the same transaction as
/// the change it describes, so the log can never disagree with the data.
pub fn record(
    conn: &Connection,
    ctx: &Ctx,
    action: &str,
    entity_type: &str,
    entity_id: Option<&str>,
    summary: impl AsRef<str>,
    details: Option<Value>,
) -> AppResult<()> {
    let details = details.map(|d| d.to_string());
    repo::insert(
        conn,
        &ctx.ts(),
        ctx.user_id(),
        &ctx.actor.username,
        action,
        entity_type,
        entity_id,
        summary.as_ref(),
        details.as_deref(),
    )
}

pub fn list(conn: &Connection, ctx: &Ctx, filter: &AuditFilter) -> AppResult<Page<AuditEntry>> {
    ctx.require(Permission::ViewAudit)?;
    repo::list(conn, filter)
}
