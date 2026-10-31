//! Stock ledger, batches and adjustment documents.

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{limit_offset, Page, Where};
use crate::error::{AppError, AppResult};
use crate::ids::new_id;
use crate::money::{Money, Qty};

pub struct NewMovement<'a> {
    pub product_id: &'a str,
    pub batch_id: Option<&'a str>,
    pub movement_type: &'a str,
    pub qty_change: Qty,
    pub value_change: Money,
    pub qty_after: Qty,
    pub value_after: Money,
    pub ref_type: &'a str,
    pub ref_id: &'a str,
    pub ref_no: &'a str,
    pub ref_line_id: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub note: Option<&'a str>,
    pub user_id: Option<&'a str>,
    pub business_date: &'a str,
    pub created_at: &'a str,
}

pub(crate) fn insert_movement(conn: &Connection, m: &NewMovement) -> AppResult<String> {
    let id = new_id();
    conn.execute(
        "INSERT INTO stock_movements (id, product_id, batch_id, movement_type, qty_change, value_change,
            qty_after, value_after, ref_type, ref_id, ref_no, ref_line_id, reason, note, user_id,
            business_date, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            id,
            m.product_id,
            m.batch_id,
            m.movement_type,
            m.qty_change,
            m.value_change,
            m.qty_after,
            m.value_after,
            m.ref_type,
            m.ref_id,
            m.ref_no,
            m.ref_line_id,
            m.reason,
            m.note,
            m.user_id,
            m.business_date,
            m.created_at
        ],
    )?;
    Ok(id)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Movement {
    pub id: String,
    pub product_id: String,
    pub product_name: String,
    pub sku: String,
    pub unit_short: String,
    pub batch_id: Option<String>,
    pub batch_no: Option<String>,
    pub movement_type: String,
    pub qty_change: Qty,
    pub value_change: Option<Money>,
    pub qty_after: Qty,
    pub ref_type: Option<String>,
    pub ref_id: Option<String>,
    pub ref_no: Option<String>,
    pub reason: Option<String>,
    pub note: Option<String>,
    pub user_name: Option<String>,
    pub business_date: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MovementFilter {
    pub product_id: Option<String>,
    pub movement_type: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn list_movements(conn: &Connection, f: &MovementFilter) -> AppResult<Page<Movement>> {
    let mut w = Where::new();
    w.opt("m.product_id = ?", &f.product_id);
    w.opt("m.movement_type = ?", &f.movement_type);
    w.opt("m.business_date >= ?", &f.date_from);
    w.opt("m.business_date <= ?", &f.date_to);
    let from = " FROM stock_movements m JOIN products p ON p.id = m.product_id JOIN units u ON u.id = p.unit_id \
        LEFT JOIN stock_batches sb ON sb.id = m.batch_id LEFT JOIN users us ON us.id = m.user_id";
    let where_sql = w.sql();
    let total: i64 =
        conn.query_row(&format!("SELECT COUNT(*){from}{where_sql}"), params_from_iter(w.args.iter()), |r| r.get(0))?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 200);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let sql = format!(
        "SELECT m.id, m.product_id, p.name, p.size, p.color, p.sku, u.short_name, m.batch_id, sb.batch_no,
            m.movement_type, m.qty_change, m.value_change, m.qty_after, m.ref_type, m.ref_id, m.ref_no,
            m.reason, m.note, us.full_name, m.business_date, m.created_at
         {from}{where_sql} ORDER BY m.created_at DESC, m.id DESC LIMIT ? OFFSET ?"
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(params_from_iter(args.iter()), |r| {
            let name: String = r.get(2)?;
            let size: Option<String> = r.get(3)?;
            let color: Option<String> = r.get(4)?;
            Ok(Movement {
                id: r.get(0)?,
                product_id: r.get(1)?,
                product_name: super::products::display_name(&name, size.as_deref(), color.as_deref()),
                sku: r.get(5)?,
                unit_short: r.get(6)?,
                batch_id: r.get(7)?,
                batch_no: r.get(8)?,
                movement_type: r.get(9)?,
                qty_change: r.get(10)?,
                value_change: Some(r.get(11)?),
                qty_after: r.get(12)?,
                ref_type: r.get(13)?,
                ref_id: r.get(14)?,
                ref_no: r.get(15)?,
                reason: r.get(16)?,
                note: r.get(17)?,
                user_name: r.get(18)?,
                business_date: r.get(19)?,
                created_at: r.get(20)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}

/// Batch the first stock-out of a document line came from (used to put
/// returned goods back into the same batch).
pub fn first_batch_for_line(conn: &Connection, ref_line_id: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT batch_id FROM stock_movements WHERE ref_line_id = ?1 AND batch_id IS NOT NULL
             ORDER BY created_at, id LIMIT 1",
            [ref_line_id],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}

// ------------------------------------------------------------- batches

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    pub id: String,
    pub product_id: String,
    pub product_name: String,
    pub sku: String,
    pub unit_short: String,
    pub batch_no: Option<String>,
    pub expiry_date: Option<String>,
    pub qty_received: Qty,
    pub qty_remaining: Qty,
    pub unit_cost: Option<Money>,
    pub received_date: String,
    pub days_to_expiry: Option<i64>,
}

pub struct NewBatch<'a> {
    pub product_id: &'a str,
    pub batch_no: Option<&'a str>,
    pub expiry_date: Option<&'a str>,
    pub qty: Qty,
    pub unit_cost: Money,
    pub source_type: &'a str,
    pub source_id: Option<&'a str>,
    pub received_date: &'a str,
    pub now: &'a str,
}

pub(crate) fn insert_batch(conn: &Connection, b: &NewBatch) -> AppResult<String> {
    let id = new_id();
    conn.execute(
        "INSERT INTO stock_batches (id, product_id, batch_no, expiry_date, qty_received, qty_remaining,
            unit_cost, source_type, source_id, received_date, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            id,
            b.product_id,
            b.batch_no,
            b.expiry_date,
            b.qty,
            b.unit_cost,
            b.source_type,
            b.source_id,
            b.received_date,
            b.now
        ],
    )?;
    Ok(id)
}

pub(crate) fn change_batch_qty(conn: &Connection, batch_id: &str, delta: Qty, now: &str) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE stock_batches SET qty_remaining = qty_remaining + ?2, updated_at = ?3
         WHERE id = ?1 AND qty_remaining + ?2 >= 0",
        params![batch_id, delta, now],
    )?;
    if n == 0 {
        return Err(AppError::InsufficientStock("The selected batch does not have enough quantity.".into()));
    }
    Ok(())
}

pub fn batch_remaining(conn: &Connection, batch_id: &str, product_id: &str) -> AppResult<Qty> {
    conn.query_row(
        "SELECT qty_remaining FROM stock_batches WHERE id = ?1 AND product_id = ?2",
        [batch_id, product_id],
        |r| r.get(0),
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("Batch"))
}

/// Batches with stock in first-expiry-first-out order (undated last).
pub fn batches_fefo(conn: &Connection, product_id: &str) -> AppResult<Vec<(String, Qty)>> {
    let mut stmt = conn.prepare(
        "SELECT id, qty_remaining FROM stock_batches WHERE product_id = ?1 AND qty_remaining > 0
         ORDER BY expiry_date IS NULL, expiry_date, received_date, created_at",
    )?;
    let rows = stmt.query_map([product_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BatchFilter {
    pub product_id: Option<String>,
    pub only_available: bool,
    /// Only batches expiring on or before today + N days (including expired).
    pub expiring_within_days: Option<i64>,
}

fn map_batch(r: &Row) -> rusqlite::Result<Batch> {
    let name: String = r.get(2)?;
    let size: Option<String> = r.get(3)?;
    let color: Option<String> = r.get(4)?;
    Ok(Batch {
        id: r.get(0)?,
        product_id: r.get(1)?,
        product_name: super::products::display_name(&name, size.as_deref(), color.as_deref()),
        sku: r.get(5)?,
        unit_short: r.get(6)?,
        batch_no: r.get(7)?,
        expiry_date: r.get(8)?,
        qty_received: r.get(9)?,
        qty_remaining: r.get(10)?,
        unit_cost: Some(r.get(11)?),
        received_date: r.get(12)?,
        days_to_expiry: r.get(13)?,
    })
}

pub fn list_batches(conn: &Connection, f: &BatchFilter, today: &str) -> AppResult<Vec<Batch>> {
    let mut w = Where::new();
    w.opt("b.product_id = ?", &f.product_id);
    if f.only_available {
        w.raw("b.qty_remaining > 0");
    }
    if let Some(days) = f.expiring_within_days {
        w.raw("b.expiry_date IS NOT NULL");
        w.eq("b.expiry_date <= date(?, '+' || ? || ' days')", today.to_string());
        w.args.push(Value::Integer(days.max(0)));
    }
    let mut args = vec![Value::Text(today.to_string())];
    args.extend(w.args.iter().cloned());
    let sql = format!(
        "SELECT b.id, b.product_id, p.name, p.size, p.color, p.sku, u.short_name, b.batch_no, b.expiry_date,
            b.qty_received, b.qty_remaining, b.unit_cost, b.received_date,
            CASE WHEN b.expiry_date IS NULL THEN NULL
                 ELSE CAST(julianday(b.expiry_date) - julianday(?) AS INTEGER) END
         FROM stock_batches b JOIN products p ON p.id = b.product_id JOIN units u ON u.id = p.unit_id
         {} ORDER BY b.expiry_date IS NULL, b.expiry_date, p.name COLLATE NOCASE LIMIT 2000",
        w.sql()
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args.iter()), map_batch)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

// --------------------------------------------------------- adjustments

pub struct NewAdjustment<'a> {
    pub id: &'a str,
    pub adjustment_no: &'a str,
    pub client_txn_id: &'a str,
    pub reason: &'a str,
    pub note: Option<&'a str>,
    pub user_id: &'a str,
    pub business_date: &'a str,
    pub created_at: &'a str,
}

pub(crate) fn insert_adjustment(conn: &Connection, a: &NewAdjustment) -> AppResult<()> {
    conn.execute(
        "INSERT INTO stock_adjustments (id, adjustment_no, client_txn_id, reason, note,
            user_id, business_date, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![a.id, a.adjustment_no, a.client_txn_id, a.reason, a.note, a.user_id, a.business_date, a.created_at],
    )?;
    Ok(())
}

pub(crate) fn insert_adjustment_item(
    conn: &Connection,
    adjustment_id: &str,
    product_id: &str,
    batch_id: Option<&str>,
    qty_change: Qty,
    unit_cost: Money,
    value_change: Money,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO stock_adjustment_items (id, adjustment_id, product_id, batch_id, qty_change, unit_cost, value_change)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![new_id(), adjustment_id, product_id, batch_id, qty_change, unit_cost, value_change],
    )?;
    Ok(())
}

pub fn adjustment_id_by_txn(conn: &Connection, client_txn_id: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row("SELECT id FROM stock_adjustments WHERE client_txn_id = ?1", [client_txn_id], |r| r.get(0))
        .optional()?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdjustmentLine {
    pub product_id: String,
    pub product_name: String,
    pub sku: String,
    pub unit_short: String,
    pub batch_no: Option<String>,
    pub qty_change: Qty,
    pub value_change: Option<Money>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adjustment {
    pub id: String,
    pub adjustment_no: String,
    pub reason: String,
    pub note: Option<String>,
    pub value_change: Option<Money>,
    pub user_name: Option<String>,
    pub business_date: String,
    pub created_at: String,
    pub lines: Vec<AdjustmentLine>,
}

fn adjustment_lines(conn: &Connection, adjustment_id: &str) -> AppResult<Vec<AdjustmentLine>> {
    let mut stmt = conn.prepare(
        "SELECT i.product_id, p.name, p.size, p.color, p.sku, u.short_name, b.batch_no, i.qty_change, i.value_change
         FROM stock_adjustment_items i JOIN products p ON p.id = i.product_id JOIN units u ON u.id = p.unit_id
         LEFT JOIN stock_batches b ON b.id = i.batch_id WHERE i.adjustment_id = ?1",
    )?;
    let rows = stmt.query_map([adjustment_id], |r| {
        let name: String = r.get(1)?;
        let size: Option<String> = r.get(2)?;
        let color: Option<String> = r.get(3)?;
        Ok(AdjustmentLine {
            product_id: r.get(0)?,
            product_name: super::products::display_name(&name, size.as_deref(), color.as_deref()),
            sku: r.get(4)?,
            unit_short: r.get(5)?,
            batch_no: r.get(6)?,
            qty_change: r.get(7)?,
            value_change: Some(r.get(8)?),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Header value = sum of its lines (computed, so the header row itself
/// stays insert-only).
const ADJ_SELECT: &str = "SELECT a.id, a.adjustment_no, a.reason, a.note,
        (SELECT COALESCE(SUM(value_change), 0) FROM stock_adjustment_items i WHERE i.adjustment_id = a.id),
        us.full_name, a.business_date, a.created_at
     FROM stock_adjustments a LEFT JOIN users us ON us.id = a.user_id";

pub fn get_adjustment(conn: &Connection, id: &str) -> AppResult<Adjustment> {
    let mut adj = conn
        .query_row(&format!("{ADJ_SELECT} WHERE a.id = ?1"), [id], |r| {
            Ok(Adjustment {
                id: r.get(0)?,
                adjustment_no: r.get(1)?,
                reason: r.get(2)?,
                note: r.get(3)?,
                value_change: Some(r.get(4)?),
                user_name: r.get(5)?,
                business_date: r.get(6)?,
                created_at: r.get(7)?,
                lines: Vec::new(),
            })
        })
        .optional()?
        .ok_or_else(|| AppError::not_found("Stock adjustment"))?;
    adj.lines = adjustment_lines(conn, id)?;
    Ok(adj)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AdjustmentFilter {
    pub reason: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn list_adjustments(conn: &Connection, f: &AdjustmentFilter) -> AppResult<Page<Adjustment>> {
    let mut w = Where::new();
    w.opt("a.reason = ?", &f.reason);
    w.opt("a.business_date >= ?", &f.date_from);
    w.opt("a.business_date <= ?", &f.date_to);
    let where_sql = w.sql();
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM stock_adjustments a{where_sql}"),
        params_from_iter(w.args.iter()),
        |r| r.get(0),
    )?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 100);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(&format!(
            "SELECT a.id FROM stock_adjustments a{where_sql} ORDER BY a.created_at DESC LIMIT ? OFFSET ?"
        ))?;
        let rows = stmt.query_map(params_from_iter(args.iter()), |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let items = ids.iter().map(|id| get_adjustment(conn, id)).collect::<AppResult<Vec<_>>>()?;
    Ok(Page { items, total })
}

// --------------------------------------------------------- integrity

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerMismatch {
    pub product_id: String,
    pub product_name: String,
    pub stock_qty: Qty,
    pub ledger_qty: Qty,
    pub stock_value: Money,
    pub ledger_value: Money,
}

/// Products whose stored stock differs from the sum of their movements.
/// An empty result means the stock ledger is fully consistent.
pub fn ledger_mismatches(conn: &Connection) -> AppResult<Vec<LedgerMismatch>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.stock_qty, COALESCE(SUM(m.qty_change), 0), p.stock_value, COALESCE(SUM(m.value_change), 0)
         FROM products p LEFT JOIN stock_movements m ON m.product_id = p.id
         GROUP BY p.id
         HAVING p.stock_qty <> COALESCE(SUM(m.qty_change), 0) OR p.stock_value <> COALESCE(SUM(m.value_change), 0)",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(LedgerMismatch {
            product_id: r.get(0)?,
            product_name: r.get(1)?,
            stock_qty: r.get(2)?,
            ledger_qty: r.get(3)?,
            stock_value: r.get(4)?,
            ledger_value: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
