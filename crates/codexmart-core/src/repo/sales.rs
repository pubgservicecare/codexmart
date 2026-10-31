//! Sales, sale lines, payments and customer returns.

use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{like, limit_offset, Page, Where};
use crate::error::{AppError, AppResult};
use crate::ids::new_id;
use crate::money::{Money, Qty};

// ------------------------------------------------------------- writing

pub struct SaleHeaderRow<'a> {
    pub id: &'a str,
    pub invoice_no: &'a str,
    pub client_txn_id: &'a str,
    pub customer_id: Option<&'a str>,
    pub price_tier: &'a str,
    pub item_count: i64,
    pub total_qty: Qty,
    pub subtotal: Money,
    pub line_discount_total: Money,
    pub bill_discount: Money,
    pub total: Money,
    pub paid_amount: Money,
    pub tendered_cash: Money,
    pub change_amount: Money,
    pub credit_amount: Money,
    pub cogs_total: Money,
    pub note: Option<&'a str>,
    pub cashier_id: &'a str,
    pub business_date: &'a str,
    pub created_at: &'a str,
}

pub(crate) fn insert_header(conn: &Connection, s: &SaleHeaderRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sales (id, invoice_no, client_txn_id, status, customer_id, price_tier, item_count, total_qty,
            subtotal, line_discount_total, bill_discount, total, paid_amount, tendered_cash, change_amount,
            credit_amount, cogs_total, note, cashier_id, business_date, created_at)
         VALUES (?1, ?2, ?3, 'completed', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            s.id, s.invoice_no, s.client_txn_id, s.customer_id, s.price_tier, s.item_count, s.total_qty,
            s.subtotal, s.line_discount_total, s.bill_discount, s.total, s.paid_amount, s.tendered_cash,
            s.change_amount, s.credit_amount, s.cogs_total, s.note, s.cashier_id, s.business_date, s.created_at
        ],
    )?;
    Ok(())
}

pub struct SaleItemRow<'a> {
    pub id: &'a str,
    pub sale_id: &'a str,
    pub line_no: i64,
    pub product_id: &'a str,
    pub product_name: &'a str,
    pub sku: &'a str,
    pub barcode: Option<&'a str>,
    pub unit_name: &'a str,
    pub qty: Qty,
    pub unit_price: Money,
    pub gross_amount: Money,
    pub line_discount: Money,
    pub bill_discount_share: Money,
    pub net_amount: Money,
    pub cost_amount: Money,
}

pub(crate) fn insert_item(conn: &Connection, i: &SaleItemRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sale_items (id, sale_id, line_no, product_id, product_name, sku, barcode, unit_name, qty,
            unit_price, gross_amount, line_discount, bill_discount_share, net_amount, cost_amount)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            i.id,
            i.sale_id,
            i.line_no,
            i.product_id,
            i.product_name,
            i.sku,
            i.barcode,
            i.unit_name,
            i.qty,
            i.unit_price,
            i.gross_amount,
            i.line_discount,
            i.bill_discount_share,
            i.net_amount,
            i.cost_amount
        ],
    )?;
    Ok(())
}

pub(crate) fn insert_payment(
    conn: &Connection,
    sale_id: &str,
    method: &str,
    amount: Money,
    reference: Option<&str>,
    now: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sale_payments (id, sale_id, method, amount, reference, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![new_id(), sale_id, method, amount, reference, now],
    )?;
    Ok(())
}

pub(crate) fn mark_void(conn: &Connection, id: &str, user_id: Option<&str>, reason: &str, now: &str) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE sales SET status = 'void', voided_at = ?2, voided_by = ?3, void_reason = ?4
         WHERE id = ?1 AND status = 'completed'",
        params![id, now, user_id, reason],
    )?;
    if n == 0 {
        return Err(AppError::Conflict("This sale has already been voided.".into()));
    }
    Ok(())
}

pub fn id_by_txn(conn: &Connection, client_txn_id: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT id FROM sales WHERE client_txn_id = ?1", [client_txn_id], |r| r.get(0)).optional()?)
}

// ------------------------------------------------------------- reading

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleSummary {
    pub id: String,
    pub invoice_no: String,
    pub status: String,
    pub customer_id: Option<String>,
    pub customer_name: Option<String>,
    pub item_count: i64,
    pub total_qty: Qty,
    pub subtotal: Money,
    pub discount_total: Money,
    pub total: Money,
    pub paid_amount: Money,
    pub credit_amount: Money,
    pub returned_amount: Money,
    pub cashier_name: String,
    pub business_date: String,
    pub created_at: String,
}

const SUMMARY_COLUMNS: &str = "s.id, s.invoice_no, s.status, s.customer_id, c.name, s.item_count, s.total_qty, \
    s.subtotal, s.line_discount_total + s.bill_discount, s.total, s.paid_amount, s.credit_amount, \
    (SELECT COALESCE(SUM(r.total_refund), 0) FROM sale_returns r WHERE r.sale_id = s.id), \
    u.full_name, s.business_date, s.created_at";

const SUMMARY_FROM: &str =
    " FROM sales s LEFT JOIN customers c ON c.id = s.customer_id JOIN users u ON u.id = s.cashier_id";

fn map_summary(r: &Row) -> rusqlite::Result<SaleSummary> {
    Ok(SaleSummary {
        id: r.get(0)?,
        invoice_no: r.get(1)?,
        status: r.get(2)?,
        customer_id: r.get(3)?,
        customer_name: r.get(4)?,
        item_count: r.get(5)?,
        total_qty: r.get(6)?,
        subtotal: r.get(7)?,
        discount_total: r.get(8)?,
        total: r.get(9)?,
        paid_amount: r.get(10)?,
        credit_amount: r.get(11)?,
        returned_amount: r.get(12)?,
        cashier_name: r.get(13)?,
        business_date: r.get(14)?,
        created_at: r.get(15)?,
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SaleFilter {
    pub search: Option<String>,
    pub status: Option<String>,
    pub customer_id: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn list(conn: &Connection, f: &SaleFilter) -> AppResult<Page<SaleSummary>> {
    let mut w = Where::new();
    w.opt("s.status = ?", &f.status);
    w.opt("s.customer_id = ?", &f.customer_id);
    w.opt("s.business_date >= ?", &f.date_from);
    w.opt("s.business_date <= ?", &f.date_to);
    if let Some(s) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        w.repeat(
            "(s.invoice_no LIKE ? ESCAPE '\\' OR IFNULL(c.name,'') LIKE ? ESCAPE '\\' OR IFNULL(c.phone,'') LIKE ? ESCAPE '\\')",
            &like(s),
            3,
        );
    }
    let where_sql = w.sql();
    let total: i64 =
        conn.query_row(&format!("SELECT COUNT(*){SUMMARY_FROM}{where_sql}"), params_from_iter(w.args.iter()), |r| {
            r.get(0)
        })?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 100);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let sql = format!(
        "SELECT {SUMMARY_COLUMNS}{SUMMARY_FROM}{where_sql} ORDER BY s.created_at DESC, s.invoice_no DESC LIMIT ? OFFSET ?"
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt.query_map(params_from_iter(args.iter()), map_summary)?.collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleItem {
    pub id: String,
    pub line_no: i64,
    pub product_id: String,
    pub product_name: String,
    pub sku: String,
    pub barcode: Option<String>,
    pub unit_name: String,
    pub qty: Qty,
    pub unit_price: Money,
    pub gross_amount: Money,
    pub line_discount: Money,
    pub bill_discount_share: Money,
    pub net_amount: Money,
    pub cost_amount: Option<Money>,
    pub returned_qty: Qty,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SalePayment {
    pub method: String,
    pub amount: Money,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleDetail {
    #[serde(flatten)]
    pub summary: SaleSummary,
    pub price_tier: String,
    pub line_discount_total: Money,
    pub bill_discount: Money,
    pub tendered_cash: Money,
    pub change_amount: Money,
    pub cogs_total: Option<Money>,
    pub note: Option<String>,
    pub customer_phone: Option<String>,
    pub voided_at: Option<String>,
    pub void_reason: Option<String>,
    pub voided_by_name: Option<String>,
    pub items: Vec<SaleItem>,
    pub payments: Vec<SalePayment>,
    pub returns: Vec<SaleReturnSummary>,
}

impl SaleDetail {
    pub fn redact_costs(&mut self) {
        self.cogs_total = None;
        for i in &mut self.items {
            i.cost_amount = None;
        }
    }
}

pub fn get(conn: &Connection, id: &str) -> AppResult<SaleDetail> {
    let sql = format!(
        "SELECT {SUMMARY_COLUMNS}, s.price_tier, s.line_discount_total, s.bill_discount, s.tendered_cash,
            s.change_amount, s.cogs_total, s.note, c.phone, s.voided_at, s.void_reason, vu.full_name
         {SUMMARY_FROM} LEFT JOIN users vu ON vu.id = s.voided_by WHERE s.id = ?1"
    );
    let mut detail = conn
        .query_row(&sql, [id], |r| {
            Ok(SaleDetail {
                summary: map_summary(r)?,
                price_tier: r.get(16)?,
                line_discount_total: r.get(17)?,
                bill_discount: r.get(18)?,
                tendered_cash: r.get(19)?,
                change_amount: r.get(20)?,
                cogs_total: Some(r.get(21)?),
                note: r.get(22)?,
                customer_phone: r.get(23)?,
                voided_at: r.get(24)?,
                void_reason: r.get(25)?,
                voided_by_name: r.get(26)?,
                items: Vec::new(),
                payments: Vec::new(),
                returns: Vec::new(),
            })
        })
        .optional()?
        .ok_or_else(|| AppError::not_found("Sale"))?;

    let returned = returned_by_item(conn, id)?;
    let mut stmt = conn.prepare(
        "SELECT id, line_no, product_id, product_name, sku, barcode, unit_name, qty, unit_price, gross_amount,
            line_discount, bill_discount_share, net_amount, cost_amount
         FROM sale_items WHERE sale_id = ?1 ORDER BY line_no",
    )?;
    detail.items = stmt
        .query_map([id], |r| {
            let item_id: String = r.get(0)?;
            Ok(SaleItem {
                returned_qty: returned.get(&item_id).map(|t| t.qty).unwrap_or(0),
                id: item_id,
                line_no: r.get(1)?,
                product_id: r.get(2)?,
                product_name: r.get(3)?,
                sku: r.get(4)?,
                barcode: r.get(5)?,
                unit_name: r.get(6)?,
                qty: r.get(7)?,
                unit_price: r.get(8)?,
                gross_amount: r.get(9)?,
                line_discount: r.get(10)?,
                bill_discount_share: r.get(11)?,
                net_amount: r.get(12)?,
                cost_amount: Some(r.get(13)?),
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut stmt = conn
        .prepare("SELECT method, amount, reference FROM sale_payments WHERE sale_id = ?1 ORDER BY created_at, rowid")?;
    detail.payments = stmt
        .query_map([id], |r| Ok(SalePayment { method: r.get(0)?, amount: r.get(1)?, reference: r.get(2)? }))?
        .collect::<Result<_, _>>()?;

    detail.returns = list_returns(conn, &ReturnFilter { sale_id: Some(id.to_string()), ..Default::default() })?.items;
    Ok(detail)
}

pub struct ReturnedTotals {
    pub qty: Qty,
    pub refund: Money,
    pub cost: Money,
}

/// What has already been returned per sale line.
pub fn returned_by_item(conn: &Connection, sale_id: &str) -> AppResult<HashMap<String, ReturnedTotals>> {
    let mut stmt = conn.prepare(
        "SELECT ri.sale_item_id, SUM(ri.qty), SUM(ri.refund_amount), SUM(ri.cost_amount)
         FROM sale_return_items ri JOIN sale_returns r ON r.id = ri.return_id
         WHERE r.sale_id = ?1 GROUP BY ri.sale_item_id",
    )?;
    let rows = stmt.query_map([sale_id], |r| {
        Ok((r.get::<_, String>(0)?, ReturnedTotals { qty: r.get(1)?, refund: r.get(2)?, cost: r.get(3)? }))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

// -------------------------------------------------------------- returns

pub struct ReturnRow<'a> {
    pub id: &'a str,
    pub return_no: &'a str,
    pub client_txn_id: &'a str,
    pub sale_id: &'a str,
    pub customer_id: Option<&'a str>,
    pub refund_method: &'a str,
    pub total_refund: Money,
    pub cost_reversed: Money,
    pub reason: Option<&'a str>,
    pub user_id: &'a str,
    pub business_date: &'a str,
    pub created_at: &'a str,
}

pub(crate) fn insert_return(conn: &Connection, r: &ReturnRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sale_returns (id, return_no, client_txn_id, sale_id, customer_id, refund_method, total_refund,
            cost_reversed, reason, user_id, business_date, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            r.id,
            r.return_no,
            r.client_txn_id,
            r.sale_id,
            r.customer_id,
            r.refund_method,
            r.total_refund,
            r.cost_reversed,
            r.reason,
            r.user_id,
            r.business_date,
            r.created_at
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn insert_return_item(
    conn: &Connection,
    return_id: &str,
    sale_item_id: &str,
    product_id: &str,
    qty: Qty,
    refund: Money,
    restock: bool,
    cost: Money,
) -> AppResult<String> {
    let id = new_id();
    conn.execute(
        "INSERT INTO sale_return_items (id, return_id, sale_item_id, product_id, qty, refund_amount, restock, cost_amount)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, return_id, sale_item_id, product_id, qty, refund, restock, cost],
    )?;
    Ok(id)
}

pub fn return_id_by_txn(conn: &Connection, client_txn_id: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row("SELECT id FROM sale_returns WHERE client_txn_id = ?1", [client_txn_id], |r| r.get(0))
        .optional()?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleReturnSummary {
    pub id: String,
    pub return_no: String,
    pub sale_id: String,
    pub invoice_no: String,
    pub customer_name: Option<String>,
    pub refund_method: String,
    pub total_refund: Money,
    pub reason: Option<String>,
    pub user_name: String,
    pub business_date: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleReturnLine {
    pub sale_item_id: String,
    pub product_name: String,
    pub sku: String,
    pub qty: Qty,
    pub refund_amount: Money,
    pub restock: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleReturnDetail {
    #[serde(flatten)]
    pub summary: SaleReturnSummary,
    pub lines: Vec<SaleReturnLine>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReturnFilter {
    pub sale_id: Option<String>,
    pub search: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

const RETURN_SELECT: &str =
    "SELECT r.id, r.return_no, r.sale_id, s.invoice_no, c.name, r.refund_method, r.total_refund,
        r.reason, u.full_name, r.business_date, r.created_at
     FROM sale_returns r JOIN sales s ON s.id = r.sale_id LEFT JOIN customers c ON c.id = r.customer_id
     JOIN users u ON u.id = r.user_id";

fn map_return(r: &Row) -> rusqlite::Result<SaleReturnSummary> {
    Ok(SaleReturnSummary {
        id: r.get(0)?,
        return_no: r.get(1)?,
        sale_id: r.get(2)?,
        invoice_no: r.get(3)?,
        customer_name: r.get(4)?,
        refund_method: r.get(5)?,
        total_refund: r.get(6)?,
        reason: r.get(7)?,
        user_name: r.get(8)?,
        business_date: r.get(9)?,
        created_at: r.get(10)?,
    })
}

pub fn list_returns(conn: &Connection, f: &ReturnFilter) -> AppResult<Page<SaleReturnSummary>> {
    let mut w = Where::new();
    w.opt("r.sale_id = ?", &f.sale_id);
    w.opt("r.business_date >= ?", &f.date_from);
    w.opt("r.business_date <= ?", &f.date_to);
    if let Some(s) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        w.repeat("(r.return_no LIKE ? ESCAPE '\\' OR s.invoice_no LIKE ? ESCAPE '\\')", &like(s), 2);
    }
    let where_sql = w.sql();
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM sale_returns r JOIN sales s ON s.id = r.sale_id{where_sql}"),
        params_from_iter(w.args.iter()),
        |r| r.get(0),
    )?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 100);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let mut stmt = conn.prepare(&format!("{RETURN_SELECT}{where_sql} ORDER BY r.created_at DESC LIMIT ? OFFSET ?"))?;
    let items = stmt.query_map(params_from_iter(args.iter()), map_return)?.collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}

pub fn get_return(conn: &Connection, id: &str) -> AppResult<SaleReturnDetail> {
    let summary = conn
        .query_row(&format!("{RETURN_SELECT} WHERE r.id = ?1"), [id], map_return)
        .optional()?
        .ok_or_else(|| AppError::not_found("Return"))?;
    let mut stmt = conn.prepare(
        "SELECT ri.sale_item_id, si.product_name, si.sku, ri.qty, ri.refund_amount, ri.restock
         FROM sale_return_items ri JOIN sale_items si ON si.id = ri.sale_item_id
         WHERE ri.return_id = ?1 ORDER BY si.line_no",
    )?;
    let lines = stmt
        .query_map([id], |r| {
            Ok(SaleReturnLine {
                sale_item_id: r.get(0)?,
                product_name: r.get(1)?,
                sku: r.get(2)?,
                qty: r.get(3)?,
                refund_amount: r.get(4)?,
                restock: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(SaleReturnDetail { summary, lines })
}
