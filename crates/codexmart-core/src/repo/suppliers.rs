//! Suppliers and their payable balances.

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{like, limit_offset, Page, Where};
use crate::error::{AppError, AppResult};
use crate::money::Money;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Supplier {
    pub id: String,
    pub name: String,
    pub contact_person: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub opening_balance: Money,
    pub notes: Option<String>,
    pub is_active: bool,
    pub total_purchases: Money,
    pub total_payments: Money,
    pub total_returns: Money,
    /// Amount the shop owes the supplier.
    pub balance: Money,
    pub last_purchase_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

const PURCHASES: &str = "COALESCE((SELECT SUM(x.total) FROM purchases x WHERE x.supplier_id = s.id), 0)";
const PAYMENTS: &str = "COALESCE((SELECT SUM(x.amount) FROM supplier_payments x WHERE x.supplier_id = s.id), 0)";
const RETURNS: &str = "COALESCE((SELECT SUM(x.total) FROM purchase_returns x WHERE x.supplier_id = s.id), 0)";

fn select() -> String {
    format!(
        "SELECT s.id, s.name, s.contact_person, s.phone, s.email, s.address, s.opening_balance, s.notes, s.is_active,
            {PURCHASES}, {PAYMENTS}, {RETURNS}, s.opening_balance + {PURCHASES} - {PAYMENTS} - {RETURNS},
            (SELECT MAX(x.business_date) FROM purchases x WHERE x.supplier_id = s.id), s.created_at, s.updated_at
         FROM suppliers s"
    )
}

fn map(r: &Row) -> rusqlite::Result<Supplier> {
    Ok(Supplier {
        id: r.get(0)?,
        name: r.get(1)?,
        contact_person: r.get(2)?,
        phone: r.get(3)?,
        email: r.get(4)?,
        address: r.get(5)?,
        opening_balance: r.get(6)?,
        notes: r.get(7)?,
        is_active: r.get(8)?,
        total_purchases: r.get(9)?,
        total_payments: r.get(10)?,
        total_returns: r.get(11)?,
        balance: r.get(12)?,
        last_purchase_date: r.get(13)?,
        created_at: r.get(14)?,
        updated_at: r.get(15)?,
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SupplierFilter {
    pub search: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn list(conn: &Connection, f: &SupplierFilter) -> AppResult<Page<Supplier>> {
    let mut w = Where::new();
    match f.status.as_deref() {
        Some("inactive") => {
            w.raw("s.is_active = 0");
        }
        Some("all") => {}
        _ => {
            w.raw("s.is_active = 1");
        }
    }
    if let Some(s) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        w.repeat(
            "(s.name LIKE ? ESCAPE '\\' OR IFNULL(s.phone,'') LIKE ? ESCAPE '\\' OR IFNULL(s.contact_person,'') LIKE ? ESCAPE '\\')",
            &like(s),
            3,
        );
    }
    let where_sql = w.sql();
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM suppliers s{where_sql}"),
        params_from_iter(w.args.iter()),
        |r| r.get(0),
    )?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 200);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let mut stmt = conn.prepare(&format!("{}{where_sql} ORDER BY s.name COLLATE NOCASE LIMIT ? OFFSET ?", select()))?;
    let items = stmt.query_map(params_from_iter(args.iter()), map)?.collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Supplier> {
    conn.query_row(&format!("{} WHERE s.id = ?1", select()), [id], map)
        .optional()?
        .ok_or_else(|| AppError::not_found("Supplier"))
}

pub struct SupplierRow<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub contact_person: Option<&'a str>,
    pub phone: Option<&'a str>,
    pub email: Option<&'a str>,
    pub address: Option<&'a str>,
    pub opening_balance: Money,
    pub notes: Option<&'a str>,
    pub is_active: bool,
    pub now: &'a str,
}

pub fn insert(conn: &Connection, s: &SupplierRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO suppliers (id, name, contact_person, phone, email, address, opening_balance, notes, is_active,
            created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            s.id,
            s.name,
            s.contact_person,
            s.phone,
            s.email,
            s.address,
            s.opening_balance,
            s.notes,
            s.is_active,
            s.now
        ],
    )?;
    Ok(())
}

pub fn update(conn: &Connection, s: &SupplierRow) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE suppliers SET name = ?2, contact_person = ?3, phone = ?4, email = ?5, address = ?6,
            opening_balance = ?7, notes = ?8, is_active = ?9, updated_at = ?10 WHERE id = ?1",
        params![
            s.id,
            s.name,
            s.contact_person,
            s.phone,
            s.email,
            s.address,
            s.opening_balance,
            s.notes,
            s.is_active,
            s.now
        ],
    )?;
    if n == 0 {
        return Err(AppError::not_found("Supplier"));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierLedgerEntry {
    pub date: String,
    pub created_at: String,
    pub kind: String,
    pub reference: String,
    pub ref_id: Option<String>,
    pub description: String,
    /// Increases what the shop owes (purchases).
    pub credit: Money,
    /// Decreases what the shop owes (payments, returns).
    pub debit: Money,
    pub balance: Money,
}

pub fn ledger(conn: &Connection, supplier_id: &str) -> AppResult<Vec<SupplierLedgerEntry>> {
    let supplier = get(conn, supplier_id)?;
    let mut stmt = conn.prepare(
        "SELECT business_date, created_at, 'purchase', purchase_no, id,
                'Purchase' || CASE WHEN supplier_invoice_no IS NULL THEN '' ELSE ' (invoice ' || supplier_invoice_no || ')' END,
                total, 0
           FROM purchases WHERE supplier_id = ?1
         UNION ALL
         SELECT business_date, created_at, 'payment', payment_no, id,
                'Payment (' || method || ')' || CASE WHEN reference IS NULL THEN '' ELSE ' ' || reference END, 0, amount
           FROM supplier_payments WHERE supplier_id = ?1
         UNION ALL
         SELECT business_date, created_at, 'return', return_no, id, 'Goods returned to supplier', 0, total
           FROM purchase_returns WHERE supplier_id = ?1
         ORDER BY 2",
    )?;
    let rows = stmt.query_map([supplier_id], |r| {
        Ok(SupplierLedgerEntry {
            date: r.get(0)?,
            created_at: r.get(1)?,
            kind: r.get(2)?,
            reference: r.get(3)?,
            ref_id: r.get(4)?,
            description: r.get(5)?,
            credit: r.get(6)?,
            debit: r.get(7)?,
            balance: 0,
        })
    })?;
    let mut running = supplier.opening_balance;
    let mut entries = vec![SupplierLedgerEntry {
        date: supplier.created_at.chars().take(10).collect(),
        created_at: supplier.created_at.clone(),
        kind: "opening".into(),
        reference: "-".into(),
        ref_id: None,
        description: "Opening balance".into(),
        credit: supplier.opening_balance.max(0),
        debit: (-supplier.opening_balance).max(0),
        balance: running,
    }];
    for row in rows {
        let mut e = row?;
        running += e.credit - e.debit;
        e.balance = running;
        entries.push(e);
    }
    Ok(entries)
}
