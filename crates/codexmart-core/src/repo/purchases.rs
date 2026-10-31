//! Purchase invoices (goods received), purchase orders, supplier payments
//! and purchase returns.

use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{like, limit_offset, Page, Where};
use crate::error::{AppError, AppResult};
use crate::ids::new_id;
use crate::money::{Money, Qty};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DocFilter {
    pub supplier_id: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn product_label(name: String, size: Option<String>, color: Option<String>) -> String {
    super::products::display_name(&name, size.as_deref(), color.as_deref())
}

// ------------------------------------------------------------ purchases

pub struct PurchaseRow<'a> {
    pub id: &'a str,
    pub purchase_no: &'a str,
    pub client_txn_id: &'a str,
    pub supplier_id: &'a str,
    pub supplier_invoice_no: Option<&'a str>,
    pub po_id: Option<&'a str>,
    pub subtotal: Money,
    pub discount: Money,
    pub total: Money,
    pub note: Option<&'a str>,
    pub user_id: &'a str,
    pub business_date: &'a str,
    pub created_at: &'a str,
}

pub(crate) fn insert_purchase(conn: &Connection, p: &PurchaseRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO purchases (id, purchase_no, client_txn_id, supplier_id, supplier_invoice_no, po_id, subtotal,
            discount, total, note, user_id, business_date, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            p.id,
            p.purchase_no,
            p.client_txn_id,
            p.supplier_id,
            p.supplier_invoice_no,
            p.po_id,
            p.subtotal,
            p.discount,
            p.total,
            p.note,
            p.user_id,
            p.business_date,
            p.created_at
        ],
    )?;
    Ok(())
}

pub struct PurchaseItemRow<'a> {
    pub purchase_id: &'a str,
    pub line_no: i64,
    pub product_id: &'a str,
    pub qty: Qty,
    pub unit_cost: Money,
    pub line_total: Money,
    pub discount_share: Money,
    pub net_cost: Money,
    pub batch_id: Option<&'a str>,
    pub batch_no: Option<&'a str>,
    pub expiry_date: Option<&'a str>,
    pub po_item_id: Option<&'a str>,
}

pub(crate) fn insert_purchase_item(conn: &Connection, i: &PurchaseItemRow) -> AppResult<String> {
    let id = new_id();
    conn.execute(
        "INSERT INTO purchase_items (id, purchase_id, line_no, product_id, qty, unit_cost, line_total, discount_share,
            net_cost, batch_id, batch_no, expiry_date, po_item_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            id,
            i.purchase_id,
            i.line_no,
            i.product_id,
            i.qty,
            i.unit_cost,
            i.line_total,
            i.discount_share,
            i.net_cost,
            i.batch_id,
            i.batch_no,
            i.expiry_date,
            i.po_item_id
        ],
    )?;
    Ok(id)
}

pub fn purchase_id_by_txn(conn: &Connection, txn: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT id FROM purchases WHERE client_txn_id = ?1", [txn], |r| r.get(0)).optional()?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseSummary {
    pub id: String,
    pub purchase_no: String,
    pub supplier_id: String,
    pub supplier_name: String,
    pub supplier_invoice_no: Option<String>,
    pub po_id: Option<String>,
    pub po_no: Option<String>,
    pub item_count: i64,
    pub subtotal: Money,
    pub discount: Money,
    pub total: Money,
    pub paid_amount: Money,
    pub returned_amount: Money,
    pub note: Option<String>,
    pub user_name: String,
    pub business_date: String,
    pub created_at: String,
}

const PURCHASE_SELECT: &str =
    "SELECT p.id, p.purchase_no, p.supplier_id, s.name, p.supplier_invoice_no, p.po_id, po.po_no,
        (SELECT COUNT(*) FROM purchase_items i WHERE i.purchase_id = p.id), p.subtotal, p.discount, p.total,
        COALESCE((SELECT SUM(sp.amount) FROM supplier_payments sp WHERE sp.purchase_id = p.id), 0),
        COALESCE((SELECT SUM(pr.total) FROM purchase_returns pr WHERE pr.purchase_id = p.id), 0),
        p.note, u.full_name, p.business_date, p.created_at
     FROM purchases p JOIN suppliers s ON s.id = p.supplier_id JOIN users u ON u.id = p.user_id
     LEFT JOIN purchase_orders po ON po.id = p.po_id";

fn map_purchase(r: &Row) -> rusqlite::Result<PurchaseSummary> {
    Ok(PurchaseSummary {
        id: r.get(0)?,
        purchase_no: r.get(1)?,
        supplier_id: r.get(2)?,
        supplier_name: r.get(3)?,
        supplier_invoice_no: r.get(4)?,
        po_id: r.get(5)?,
        po_no: r.get(6)?,
        item_count: r.get(7)?,
        subtotal: r.get(8)?,
        discount: r.get(9)?,
        total: r.get(10)?,
        paid_amount: r.get(11)?,
        returned_amount: r.get(12)?,
        note: r.get(13)?,
        user_name: r.get(14)?,
        business_date: r.get(15)?,
        created_at: r.get(16)?,
    })
}

pub fn list_purchases(conn: &Connection, f: &DocFilter) -> AppResult<Page<PurchaseSummary>> {
    let mut w = Where::new();
    w.opt("p.supplier_id = ?", &f.supplier_id);
    w.opt("p.business_date >= ?", &f.date_from);
    w.opt("p.business_date <= ?", &f.date_to);
    if let Some(s) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        w.repeat(
            "(p.purchase_no LIKE ? ESCAPE '\\' OR IFNULL(p.supplier_invoice_no,'') LIKE ? ESCAPE '\\' OR s.name LIKE ? ESCAPE '\\')",
            &like(s),
            3,
        );
    }
    let where_sql = w.sql();
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM purchases p JOIN suppliers s ON s.id = p.supplier_id{where_sql}"),
        params_from_iter(w.args.iter()),
        |r| r.get(0),
    )?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 100);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let mut stmt = conn.prepare(&format!(
        "{PURCHASE_SELECT}{where_sql} ORDER BY p.business_date DESC, p.created_at DESC LIMIT ? OFFSET ?"
    ))?;
    let items = stmt.query_map(params_from_iter(args.iter()), map_purchase)?.collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseItem {
    pub id: String,
    pub line_no: i64,
    pub product_id: String,
    pub product_name: String,
    pub sku: String,
    pub unit_short: String,
    pub qty: Qty,
    pub unit_cost: Money,
    pub line_total: Money,
    pub discount_share: Money,
    pub net_cost: Money,
    pub batch_id: Option<String>,
    pub batch_no: Option<String>,
    pub expiry_date: Option<String>,
    pub returned_qty: Qty,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseDetail {
    #[serde(flatten)]
    pub summary: PurchaseSummary,
    pub items: Vec<PurchaseItem>,
}

/// Quantity and value already returned per purchase line.
pub fn returned_by_purchase_item(conn: &Connection, purchase_id: &str) -> AppResult<HashMap<String, (Qty, Money)>> {
    let mut stmt = conn.prepare(
        "SELECT ri.purchase_item_id, SUM(ri.qty), SUM(ri.line_total)
         FROM purchase_return_items ri JOIN purchase_returns r ON r.id = ri.return_id
         WHERE r.purchase_id = ?1 GROUP BY ri.purchase_item_id",
    )?;
    let rows = stmt.query_map([purchase_id], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?))))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get_purchase(conn: &Connection, id: &str) -> AppResult<PurchaseDetail> {
    let summary = conn
        .query_row(&format!("{PURCHASE_SELECT} WHERE p.id = ?1"), [id], map_purchase)
        .optional()?
        .ok_or_else(|| AppError::not_found("Purchase"))?;
    let returned = returned_by_purchase_item(conn, id)?;
    let mut stmt = conn.prepare(
        "SELECT i.id, i.line_no, i.product_id, p.name, p.size, p.color, p.sku, u.short_name, i.qty, i.unit_cost,
            i.line_total, i.discount_share, i.net_cost, i.batch_id, i.batch_no, i.expiry_date
         FROM purchase_items i JOIN products p ON p.id = i.product_id JOIN units u ON u.id = p.unit_id
         WHERE i.purchase_id = ?1 ORDER BY i.line_no",
    )?;
    let items = stmt
        .query_map([id], |r| {
            let item_id: String = r.get(0)?;
            Ok(PurchaseItem {
                returned_qty: returned.get(&item_id).map(|v| v.0).unwrap_or(0),
                id: item_id,
                line_no: r.get(1)?,
                product_id: r.get(2)?,
                product_name: product_label(r.get(3)?, r.get(4)?, r.get(5)?),
                sku: r.get(6)?,
                unit_short: r.get(7)?,
                qty: r.get(8)?,
                unit_cost: r.get(9)?,
                line_total: r.get(10)?,
                discount_share: r.get(11)?,
                net_cost: r.get(12)?,
                batch_id: r.get(13)?,
                batch_no: r.get(14)?,
                expiry_date: r.get(15)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(PurchaseDetail { summary, items })
}

pub struct PurchaseItemRef {
    pub product_id: String,
    pub qty: Qty,
    pub net_cost: Money,
    pub batch_id: Option<String>,
}

pub fn purchase_item(conn: &Connection, purchase_id: &str, item_id: &str) -> AppResult<PurchaseItemRef> {
    conn.query_row(
        "SELECT product_id, qty, net_cost, batch_id FROM purchase_items WHERE id = ?1 AND purchase_id = ?2",
        [item_id, purchase_id],
        |r| Ok(PurchaseItemRef { product_id: r.get(0)?, qty: r.get(1)?, net_cost: r.get(2)?, batch_id: r.get(3)? }),
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("Purchase line"))
}

/// (supplier_id, total) for a purchase.
pub fn purchase_head(conn: &Connection, id: &str) -> AppResult<(String, String, Money)> {
    conn.query_row("SELECT supplier_id, purchase_no, total FROM purchases WHERE id = ?1", [id], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })
    .optional()?
    .ok_or_else(|| AppError::not_found("Purchase"))
}

// ------------------------------------------------------- purchase orders

pub struct PoRow<'a> {
    pub id: &'a str,
    pub po_no: &'a str,
    pub supplier_id: &'a str,
    pub status: &'a str,
    pub order_date: &'a str,
    pub expected_date: Option<&'a str>,
    pub note: Option<&'a str>,
    pub total_estimate: Money,
    pub created_by: &'a str,
    pub now: &'a str,
}

pub(crate) fn insert_po(conn: &Connection, p: &PoRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO purchase_orders (id, po_no, supplier_id, status, order_date, expected_date, note, total_estimate,
            created_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            p.id,
            p.po_no,
            p.supplier_id,
            p.status,
            p.order_date,
            p.expected_date,
            p.note,
            p.total_estimate,
            p.created_by,
            p.now
        ],
    )?;
    Ok(())
}

pub(crate) fn update_po_header(conn: &Connection, p: &PoRow) -> AppResult<()> {
    conn.execute(
        "UPDATE purchase_orders SET supplier_id = ?2, status = ?3, order_date = ?4, expected_date = ?5, note = ?6,
            total_estimate = ?7, updated_at = ?8 WHERE id = ?1",
        params![p.id, p.supplier_id, p.status, p.order_date, p.expected_date, p.note, p.total_estimate, p.now],
    )?;
    Ok(())
}

pub(crate) fn replace_po_items(conn: &Connection, po_id: &str, items: &[(String, Qty, Money)]) -> AppResult<()> {
    conn.execute("DELETE FROM purchase_order_items WHERE po_id = ?1", [po_id])?;
    for (i, (product_id, qty, cost)) in items.iter().enumerate() {
        conn.execute(
            "INSERT INTO purchase_order_items (id, po_id, line_no, product_id, qty_ordered, qty_received, unit_cost)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![new_id(), po_id, i as i64 + 1, product_id, qty, cost],
        )?;
    }
    Ok(())
}

pub(crate) fn set_po_status(conn: &Connection, id: &str, status: &str, now: &str) -> AppResult<()> {
    conn.execute("UPDATE purchase_orders SET status = ?2, updated_at = ?3 WHERE id = ?1", params![id, status, now])?;
    Ok(())
}

pub(crate) fn add_po_received(conn: &Connection, po_item_id: &str, po_id: &str, qty: Qty) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE purchase_order_items SET qty_received = qty_received + ?3 WHERE id = ?1 AND po_id = ?2",
        params![po_item_id, po_id, qty],
    )?;
    if n == 0 {
        return Err(AppError::validation("A received line does not belong to the selected purchase order."));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoItem {
    pub id: String,
    pub line_no: i64,
    pub product_id: String,
    pub product_name: String,
    pub sku: String,
    pub unit_short: String,
    pub allow_decimal: bool,
    pub qty_ordered: Qty,
    pub qty_received: Qty,
    pub unit_cost: Money,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrder {
    pub id: String,
    pub po_no: String,
    pub supplier_id: String,
    pub supplier_name: String,
    pub status: String,
    pub order_date: String,
    pub expected_date: Option<String>,
    pub note: Option<String>,
    pub total_estimate: Money,
    pub item_count: i64,
    pub created_by_name: String,
    pub created_at: String,
    pub updated_at: String,
    pub items: Vec<PoItem>,
}

const PO_SELECT: &str = "SELECT po.id, po.po_no, po.supplier_id, s.name, po.status, po.order_date, po.expected_date,
        po.note, po.total_estimate, (SELECT COUNT(*) FROM purchase_order_items i WHERE i.po_id = po.id),
        u.full_name, po.created_at, po.updated_at
     FROM purchase_orders po JOIN suppliers s ON s.id = po.supplier_id JOIN users u ON u.id = po.created_by";

fn map_po(r: &Row) -> rusqlite::Result<PurchaseOrder> {
    Ok(PurchaseOrder {
        id: r.get(0)?,
        po_no: r.get(1)?,
        supplier_id: r.get(2)?,
        supplier_name: r.get(3)?,
        status: r.get(4)?,
        order_date: r.get(5)?,
        expected_date: r.get(6)?,
        note: r.get(7)?,
        total_estimate: r.get(8)?,
        item_count: r.get(9)?,
        created_by_name: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
        items: Vec::new(),
    })
}

pub fn get_po(conn: &Connection, id: &str) -> AppResult<PurchaseOrder> {
    let mut po = conn
        .query_row(&format!("{PO_SELECT} WHERE po.id = ?1"), [id], map_po)
        .optional()?
        .ok_or_else(|| AppError::not_found("Purchase order"))?;
    let mut stmt = conn.prepare(
        "SELECT i.id, i.line_no, i.product_id, p.name, p.size, p.color, p.sku, u.short_name, u.allow_decimal,
            i.qty_ordered, i.qty_received, i.unit_cost
         FROM purchase_order_items i JOIN products p ON p.id = i.product_id JOIN units u ON u.id = p.unit_id
         WHERE i.po_id = ?1 ORDER BY i.line_no",
    )?;
    po.items = stmt
        .query_map([id], |r| {
            Ok(PoItem {
                id: r.get(0)?,
                line_no: r.get(1)?,
                product_id: r.get(2)?,
                product_name: product_label(r.get(3)?, r.get(4)?, r.get(5)?),
                sku: r.get(6)?,
                unit_short: r.get(7)?,
                allow_decimal: r.get(8)?,
                qty_ordered: r.get(9)?,
                qty_received: r.get(10)?,
                unit_cost: r.get(11)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(po)
}

pub fn list_pos(conn: &Connection, f: &DocFilter) -> AppResult<Page<PurchaseOrder>> {
    let mut w = Where::new();
    w.opt("po.supplier_id = ?", &f.supplier_id);
    w.opt("po.status = ?", &f.status);
    w.opt("po.order_date >= ?", &f.date_from);
    w.opt("po.order_date <= ?", &f.date_to);
    if let Some(s) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        w.repeat("(po.po_no LIKE ? ESCAPE '\\' OR s.name LIKE ? ESCAPE '\\')", &like(s), 2);
    }
    let where_sql = w.sql();
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM purchase_orders po JOIN suppliers s ON s.id = po.supplier_id{where_sql}"),
        params_from_iter(w.args.iter()),
        |r| r.get(0),
    )?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 100);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let mut stmt = conn.prepare(&format!("{PO_SELECT}{where_sql} ORDER BY po.created_at DESC LIMIT ? OFFSET ?"))?;
    let items = stmt.query_map(params_from_iter(args.iter()), map_po)?.collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}

// ------------------------------------------------------ supplier payments

pub struct SupplierPaymentRow<'a> {
    pub id: &'a str,
    pub payment_no: &'a str,
    pub client_txn_id: &'a str,
    pub supplier_id: &'a str,
    pub purchase_id: Option<&'a str>,
    pub amount: Money,
    pub method: &'a str,
    pub reference: Option<&'a str>,
    pub note: Option<&'a str>,
    pub user_id: &'a str,
    pub business_date: &'a str,
    pub created_at: &'a str,
}

pub(crate) fn insert_supplier_payment(conn: &Connection, p: &SupplierPaymentRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO supplier_payments (id, payment_no, client_txn_id, supplier_id, purchase_id, amount, method,
            reference, note, user_id, business_date, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            p.id,
            p.payment_no,
            p.client_txn_id,
            p.supplier_id,
            p.purchase_id,
            p.amount,
            p.method,
            p.reference,
            p.note,
            p.user_id,
            p.business_date,
            p.created_at
        ],
    )?;
    Ok(())
}

pub fn supplier_payment_id_by_txn(conn: &Connection, txn: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT id FROM supplier_payments WHERE client_txn_id = ?1", [txn], |r| r.get(0)).optional()?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierPayment {
    pub id: String,
    pub payment_no: String,
    pub supplier_id: String,
    pub supplier_name: String,
    pub purchase_id: Option<String>,
    pub purchase_no: Option<String>,
    pub amount: Money,
    pub method: String,
    pub reference: Option<String>,
    pub note: Option<String>,
    pub user_name: String,
    pub business_date: String,
    pub created_at: String,
}

const SP_SELECT: &str = "SELECT sp.id, sp.payment_no, sp.supplier_id, s.name, sp.purchase_id, p.purchase_no, sp.amount,
        sp.method, sp.reference, sp.note, u.full_name, sp.business_date, sp.created_at
     FROM supplier_payments sp JOIN suppliers s ON s.id = sp.supplier_id JOIN users u ON u.id = sp.user_id
     LEFT JOIN purchases p ON p.id = sp.purchase_id";

fn map_sp(r: &Row) -> rusqlite::Result<SupplierPayment> {
    Ok(SupplierPayment {
        id: r.get(0)?,
        payment_no: r.get(1)?,
        supplier_id: r.get(2)?,
        supplier_name: r.get(3)?,
        purchase_id: r.get(4)?,
        purchase_no: r.get(5)?,
        amount: r.get(6)?,
        method: r.get(7)?,
        reference: r.get(8)?,
        note: r.get(9)?,
        user_name: r.get(10)?,
        business_date: r.get(11)?,
        created_at: r.get(12)?,
    })
}

pub fn get_supplier_payment(conn: &Connection, id: &str) -> AppResult<SupplierPayment> {
    conn.query_row(&format!("{SP_SELECT} WHERE sp.id = ?1"), [id], map_sp)
        .optional()?
        .ok_or_else(|| AppError::not_found("Supplier payment"))
}

pub fn list_supplier_payments(conn: &Connection, f: &DocFilter) -> AppResult<Page<SupplierPayment>> {
    let mut w = Where::new();
    w.opt("sp.supplier_id = ?", &f.supplier_id);
    w.opt("sp.business_date >= ?", &f.date_from);
    w.opt("sp.business_date <= ?", &f.date_to);
    let where_sql = w.sql();
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM supplier_payments sp{where_sql}"),
        params_from_iter(w.args.iter()),
        |r| r.get(0),
    )?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 100);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let mut stmt = conn.prepare(&format!(
        "{SP_SELECT}{where_sql} ORDER BY sp.business_date DESC, sp.created_at DESC LIMIT ? OFFSET ?"
    ))?;
    let items = stmt.query_map(params_from_iter(args.iter()), map_sp)?.collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}

// ------------------------------------------------------ purchase returns

pub struct PurchaseReturnRow<'a> {
    pub id: &'a str,
    pub return_no: &'a str,
    pub client_txn_id: &'a str,
    pub supplier_id: &'a str,
    pub purchase_id: &'a str,
    pub total: Money,
    pub reason: Option<&'a str>,
    pub user_id: &'a str,
    pub business_date: &'a str,
    pub created_at: &'a str,
}

pub(crate) fn insert_purchase_return(conn: &Connection, r: &PurchaseReturnRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO purchase_returns (id, return_no, client_txn_id, supplier_id, purchase_id, total, reason, user_id,
            business_date, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            r.id,
            r.return_no,
            r.client_txn_id,
            r.supplier_id,
            r.purchase_id,
            r.total,
            r.reason,
            r.user_id,
            r.business_date,
            r.created_at
        ],
    )?;
    Ok(())
}

pub(crate) fn insert_purchase_return_item(
    conn: &Connection,
    return_id: &str,
    purchase_item_id: &str,
    product_id: &str,
    batch_id: Option<&str>,
    qty: Qty,
    line_total: Money,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO purchase_return_items (id, return_id, purchase_item_id, product_id, batch_id, qty, line_total)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![new_id(), return_id, purchase_item_id, product_id, batch_id, qty, line_total],
    )?;
    Ok(())
}

pub fn purchase_return_id_by_txn(conn: &Connection, txn: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT id FROM purchase_returns WHERE client_txn_id = ?1", [txn], |r| r.get(0)).optional()?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseReturnLine {
    pub product_name: String,
    pub sku: String,
    pub unit_short: String,
    pub qty: Qty,
    pub line_total: Money,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseReturn {
    pub id: String,
    pub return_no: String,
    pub supplier_id: String,
    pub supplier_name: String,
    pub purchase_id: String,
    pub purchase_no: String,
    pub total: Money,
    pub reason: Option<String>,
    pub user_name: String,
    pub business_date: String,
    pub created_at: String,
    pub lines: Vec<PurchaseReturnLine>,
}

const PR_SELECT: &str =
    "SELECT r.id, r.return_no, r.supplier_id, s.name, r.purchase_id, p.purchase_no, r.total, r.reason,
        u.full_name, r.business_date, r.created_at
     FROM purchase_returns r JOIN suppliers s ON s.id = r.supplier_id JOIN purchases p ON p.id = r.purchase_id
     JOIN users u ON u.id = r.user_id";

fn map_pr(r: &Row) -> rusqlite::Result<PurchaseReturn> {
    Ok(PurchaseReturn {
        id: r.get(0)?,
        return_no: r.get(1)?,
        supplier_id: r.get(2)?,
        supplier_name: r.get(3)?,
        purchase_id: r.get(4)?,
        purchase_no: r.get(5)?,
        total: r.get(6)?,
        reason: r.get(7)?,
        user_name: r.get(8)?,
        business_date: r.get(9)?,
        created_at: r.get(10)?,
        lines: Vec::new(),
    })
}

pub fn get_purchase_return(conn: &Connection, id: &str) -> AppResult<PurchaseReturn> {
    let mut pr = conn
        .query_row(&format!("{PR_SELECT} WHERE r.id = ?1"), [id], map_pr)
        .optional()?
        .ok_or_else(|| AppError::not_found("Purchase return"))?;
    let mut stmt = conn.prepare(
        "SELECT p.name, p.size, p.color, p.sku, u.short_name, i.qty, i.line_total
         FROM purchase_return_items i JOIN products p ON p.id = i.product_id JOIN units u ON u.id = p.unit_id
         WHERE i.return_id = ?1",
    )?;
    pr.lines = stmt
        .query_map([id], |r| {
            Ok(PurchaseReturnLine {
                product_name: product_label(r.get(0)?, r.get(1)?, r.get(2)?),
                sku: r.get(3)?,
                unit_short: r.get(4)?,
                qty: r.get(5)?,
                line_total: r.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(pr)
}

pub fn list_purchase_returns(conn: &Connection, f: &DocFilter) -> AppResult<Page<PurchaseReturn>> {
    let mut w = Where::new();
    w.opt("r.supplier_id = ?", &f.supplier_id);
    w.opt("r.business_date >= ?", &f.date_from);
    w.opt("r.business_date <= ?", &f.date_to);
    let where_sql = w.sql();
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM purchase_returns r{where_sql}"),
        params_from_iter(w.args.iter()),
        |r| r.get(0),
    )?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 100);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let mut stmt = conn.prepare(&format!("{PR_SELECT}{where_sql} ORDER BY r.created_at DESC LIMIT ? OFFSET ?"))?;
    let items = stmt.query_map(params_from_iter(args.iter()), map_pr)?.collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}
