//! Stock adjustments, stock history, batches and ledger verification.

use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;

use super::stock::{self, BatchTarget, Refs};
use super::{audit, begin, user_id};
use crate::context::{Ctx, Permission};
use crate::domain::validation as v;
use crate::error::{AppError, AppResult};
use crate::ids::new_id;
use crate::money::{self, Money, Qty};
use crate::repo::stock::{
    self as repo, Adjustment, AdjustmentFilter, Batch, BatchFilter, LedgerMismatch, Movement, MovementFilter,
    NewAdjustment,
};
use crate::repo::{products, sequences, Page};

pub const REASONS: &[&str] =
    &["opening_stock", "stock_count", "found", "damaged", "expired", "lost", "theft", "internal_use", "other"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdjustMode {
    /// Increase by qty.
    Add,
    /// Decrease by qty.
    Remove,
    /// Set the on-hand quantity to qty (physical count).
    Set,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdjustmentLineInput {
    pub product_id: String,
    pub mode: AdjustMode,
    pub qty: Qty,
    #[serde(default)]
    pub unit_cost: Option<Money>,
    #[serde(default)]
    pub batch_id: Option<String>,
    #[serde(default)]
    pub batch_no: Option<String>,
    #[serde(default)]
    pub expiry_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdjustmentInput {
    pub client_txn_id: String,
    pub reason: String,
    #[serde(default)]
    pub note: Option<String>,
    pub lines: Vec<AdjustmentLineInput>,
}

fn reason_allows(reason: &str, mode: AdjustMode) -> bool {
    match reason {
        "opening_stock" | "found" => mode == AdjustMode::Add,
        "damaged" | "expired" | "lost" | "theft" | "internal_use" => mode == AdjustMode::Remove,
        "stock_count" => mode == AdjustMode::Set,
        _ => mode != AdjustMode::Set,
    }
}

pub fn adjust(conn: &mut Connection, ctx: &Ctx, input: AdjustmentInput) -> AppResult<Adjustment> {
    ctx.require(Permission::ManageInventory)?;
    let txn = v::client_txn_id(&input.client_txn_id)?;
    if !REASONS.contains(&input.reason.as_str()) {
        return Err(AppError::validation("Choose a valid reason for the adjustment."));
    }
    if input.lines.is_empty() {
        return Err(AppError::validation("Add at least one product to adjust."));
    }
    let note = v::optional_multiline("Note", input.note.as_deref(), 500)?;
    let uid = user_id(ctx)?.to_string();
    let tx = begin(conn)?;
    if let Some(id) = repo::adjustment_id_by_txn(&tx, &txn)? {
        drop(tx);
        return repo::get_adjustment(conn, &id);
    }
    let id = new_id();
    let no = sequences::next(&tx, "adjustment")?;
    repo::insert_adjustment(
        &tx,
        &NewAdjustment {
            id: &id,
            adjustment_no: &no,
            client_txn_id: &txn,
            reason: &input.reason,
            note: note.as_deref(),
            user_id: &uid,
            business_date: &ctx.date(),
            created_at: &ctx.ts(),
        },
    )?;
    let reason_label = input.reason.replace('_', " ");
    let mut posted = 0;
    let mut total_value: Money = 0;
    for (i, line) in input.lines.iter().enumerate() {
        let what = format!("Line {}", i + 1);
        if !reason_allows(&input.reason, line.mode) {
            return Err(AppError::validation(format!("{what}: this reason does not allow that kind of change.")));
        }
        let p = products::get(&tx, &line.product_id)?;
        let delta = match line.mode {
            AdjustMode::Add => {
                v::quantity(&what, line.qty, p.allow_decimal)?;
                line.qty
            }
            AdjustMode::Remove => {
                v::quantity(&what, line.qty, p.allow_decimal)?;
                -line.qty
            }
            AdjustMode::Set => {
                if line.qty < 0 {
                    return Err(AppError::validation(format!("{what}: counted quantity cannot be negative.")));
                }
                if line.qty > 0 {
                    v::quantity(&what, line.qty, p.allow_decimal)?;
                }
                line.qty - p.stock_qty
            }
        };
        if delta == 0 {
            continue;
        }
        let refs = Refs {
            ref_type: "adjustment",
            ref_id: &id,
            ref_no: &no,
            ref_line_id: None,
            reason: Some(&reason_label),
            note: note.as_deref(),
        };
        let unit_cost = line.unit_cost.unwrap_or(p.cost_price.unwrap_or(0));
        v::non_negative("Unit cost", unit_cost)?;
        let (batch_id, value_change) = if delta > 0 {
            let movement = if input.reason == "opening_stock" { "opening" } else { "adjustment_in" };
            let expiry = v::optional_date("Expiry date", line.expiry_date.as_deref())?;
            let batch_no = v::optional("Batch number", line.batch_no.as_deref(), 40)?;
            let target = match line.batch_id.as_deref() {
                Some(b) => BatchTarget::Existing(b),
                None => BatchTarget::New {
                    batch_no: batch_no.as_deref(),
                    expiry_date: expiry.as_deref(),
                    source_type: "adjustment",
                    source_id: &id,
                },
            };
            let r = stock::receive(&tx, ctx, &p.id, delta, money::mul_qty(unit_cost, delta), movement, &refs, target)?;
            (r.batch_id, r.value_change)
        } else {
            let issued =
                stock::issue(&tx, ctx, &p.id, -delta, "adjustment_out", &refs, false, line.batch_id.as_deref())?;
            (line.batch_id.clone(), issued.value_change)
        };
        repo::insert_adjustment_item(&tx, &id, &p.id, batch_id.as_deref(), delta, unit_cost, value_change)?;
        total_value += value_change;
        posted += 1;
    }
    if posted == 0 {
        return Err(AppError::validation("Nothing to adjust: the quantities already match."));
    }
    audit::record(
        &tx,
        ctx,
        "stock.adjusted",
        "adjustment",
        Some(&id),
        format!("Stock adjustment {no} ({reason_label}), {posted} line(s)"),
        Some(json!({ "valueChange": total_value })),
    )?;
    tx.commit()?;
    get_adjustment(conn, ctx, &id)
}

fn redact_adjustment(ctx: &Ctx, mut a: Adjustment) -> Adjustment {
    if !ctx.can(Permission::ViewCosts) {
        a.value_change = None;
        for l in &mut a.lines {
            l.value_change = None;
        }
    }
    a
}

pub fn get_adjustment(conn: &Connection, ctx: &Ctx, id: &str) -> AppResult<Adjustment> {
    ctx.require(Permission::ManageInventory)?;
    Ok(redact_adjustment(ctx, repo::get_adjustment(conn, id)?))
}

pub fn list_adjustments(conn: &Connection, ctx: &Ctx, f: &AdjustmentFilter) -> AppResult<Page<Adjustment>> {
    ctx.require(Permission::ManageInventory)?;
    let page = repo::list_adjustments(conn, f)?;
    Ok(Page { total: page.total, items: page.items.into_iter().map(|a| redact_adjustment(ctx, a)).collect() })
}

pub fn list_movements(conn: &Connection, ctx: &Ctx, f: &MovementFilter) -> AppResult<Page<Movement>> {
    ctx.require(Permission::ViewCatalog)?;
    let mut page = repo::list_movements(conn, f)?;
    if !ctx.can(Permission::ViewCosts) {
        for m in &mut page.items {
            m.value_change = None;
        }
    }
    Ok(page)
}

pub fn list_batches(conn: &Connection, ctx: &Ctx, f: &BatchFilter) -> AppResult<Vec<Batch>> {
    ctx.require(Permission::ViewCatalog)?;
    let mut rows = repo::list_batches(conn, f, &ctx.date())?;
    if !ctx.can(Permission::ViewCosts) {
        for b in &mut rows {
            b.unit_cost = None;
        }
    }
    Ok(rows)
}

/// Products whose stock does not match their movement history.
pub fn verify_ledger(conn: &Connection, ctx: &Ctx) -> AppResult<Vec<LedgerMismatch>> {
    ctx.require(Permission::ManageInventory)?;
    repo::ledger_mismatches(conn)
}
