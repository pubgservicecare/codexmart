//! Customers, their balances (udhaar) and payments received.

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{like, limit_offset, Page, Where};
use crate::error::{AppError, AppResult};
use crate::money::Money;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Customer {
    pub id: String,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub opening_balance: Money,
    pub credit_limit: Option<Money>,
    pub notes: Option<String>,
    pub is_active: bool,
    /// Amount the customer owes the shop (negative = shop owes customer).
    pub balance: Money,
    pub total_purchases: Money,
    pub last_purchase_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Balance = opening + credit sales - payments - refunds to account.
const BALANCE_SQL: &str = "c.opening_balance
    + COALESCE((SELECT SUM(s.credit_amount) FROM sales s WHERE s.customer_id = c.id AND s.status = 'completed'), 0)
    - COALESCE((SELECT SUM(p.amount) FROM customer_payments p WHERE p.customer_id = c.id), 0)
    - COALESCE((SELECT SUM(r.total_refund) FROM sale_returns r WHERE r.customer_id = c.id AND r.refund_method = 'account'), 0)";

fn select() -> String {
    format!(
        "SELECT c.id, c.name, c.phone, c.email, c.address, c.opening_balance, c.credit_limit, c.notes, c.is_active,
            {BALANCE_SQL},
            COALESCE((SELECT SUM(s.total) FROM sales s WHERE s.customer_id = c.id AND s.status = 'completed'), 0),
            (SELECT MAX(s.business_date) FROM sales s WHERE s.customer_id = c.id AND s.status = 'completed'),
            c.created_at, c.updated_at
         FROM customers c"
    )
}

fn map(r: &Row) -> rusqlite::Result<Customer> {
    Ok(Customer {
        id: r.get(0)?,
        name: r.get(1)?,
        phone: r.get(2)?,
        email: r.get(3)?,
        address: r.get(4)?,
        opening_balance: r.get(5)?,
        credit_limit: r.get(6)?,
        notes: r.get(7)?,
        is_active: r.get(8)?,
        balance: r.get(9)?,
        total_purchases: r.get(10)?,
        last_purchase_date: r.get(11)?,
        created_at: r.get(12)?,
        updated_at: r.get(13)?,
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomerFilter {
    pub search: Option<String>,
    pub status: Option<String>,
    pub with_balance: bool,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn list(conn: &Connection, f: &CustomerFilter) -> AppResult<Page<Customer>> {
    let mut w = Where::new();
    match f.status.as_deref() {
        Some("inactive") => {
            w.raw("c.is_active = 0");
        }
        Some("all") => {}
        _ => {
            w.raw("c.is_active = 1");
        }
    }
    if let Some(s) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        w.repeat("(c.name LIKE ? ESCAPE '\\' OR IFNULL(c.phone,'') LIKE ? ESCAPE '\\')", &like(s), 2);
    }
    if f.with_balance {
        w.raw(&format!("({BALANCE_SQL}) <> 0"));
    }
    let where_sql = w.sql();
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM customers c{where_sql}"),
        params_from_iter(w.args.iter()),
        |r| r.get(0),
    )?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 200);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let mut stmt = conn.prepare(&format!("{}{where_sql} ORDER BY c.name COLLATE NOCASE LIMIT ? OFFSET ?", select()))?;
    let items = stmt.query_map(params_from_iter(args.iter()), map)?.collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Customer> {
    conn.query_row(&format!("{} WHERE c.id = ?1", select()), [id], map)
        .optional()?
        .ok_or_else(|| AppError::not_found("Customer"))
}

pub fn balance(conn: &Connection, id: &str) -> AppResult<Money> {
    Ok(conn.query_row(&format!("SELECT {BALANCE_SQL} FROM customers c WHERE c.id = ?1"), [id], |r| r.get(0))?)
}

pub struct CustomerRow<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub phone: Option<&'a str>,
    pub email: Option<&'a str>,
    pub address: Option<&'a str>,
    pub opening_balance: Money,
    pub credit_limit: Option<Money>,
    pub notes: Option<&'a str>,
    pub is_active: bool,
    pub now: &'a str,
}

pub fn insert(conn: &Connection, c: &CustomerRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO customers (id, name, phone, email, address, opening_balance, credit_limit, notes, is_active,
            created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            c.id,
            c.name,
            c.phone,
            c.email,
            c.address,
            c.opening_balance,
            c.credit_limit,
            c.notes,
            c.is_active,
            c.now
        ],
    )?;
    Ok(())
}

pub fn update(conn: &Connection, c: &CustomerRow) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE customers SET name = ?2, phone = ?3, email = ?4, address = ?5, opening_balance = ?6,
            credit_limit = ?7, notes = ?8, is_active = ?9, updated_at = ?10 WHERE id = ?1",
        params![
            c.id,
            c.name,
            c.phone,
            c.email,
            c.address,
            c.opening_balance,
            c.credit_limit,
            c.notes,
            c.is_active,
            c.now
        ],
    )?;
    if n == 0 {
        return Err(AppError::not_found("Customer"));
    }
    Ok(())
}

// ------------------------------------------------------------ payments

pub struct PaymentRow<'a> {
    pub id: &'a str,
    pub receipt_no: &'a str,
    pub client_txn_id: &'a str,
    pub customer_id: &'a str,
    pub amount: Money,
    pub method: &'a str,
    pub note: Option<&'a str>,
    pub user_id: &'a str,
    pub business_date: &'a str,
    pub created_at: &'a str,
}

pub(crate) fn insert_payment(conn: &Connection, p: &PaymentRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO customer_payments (id, receipt_no, client_txn_id, customer_id, amount, method, note, user_id,
            business_date, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            p.id,
            p.receipt_no,
            p.client_txn_id,
            p.customer_id,
            p.amount,
            p.method,
            p.note,
            p.user_id,
            p.business_date,
            p.created_at
        ],
    )?;
    Ok(())
}

pub fn payment_id_by_txn(conn: &Connection, client_txn_id: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row("SELECT id FROM customer_payments WHERE client_txn_id = ?1", [client_txn_id], |r| r.get(0))
        .optional()?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerPayment {
    pub id: String,
    pub receipt_no: String,
    pub customer_id: String,
    pub customer_name: String,
    pub amount: Money,
    pub method: String,
    pub note: Option<String>,
    pub user_name: String,
    pub business_date: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PaymentFilter {
    pub customer_id: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

const PAYMENT_SELECT: &str = "SELECT p.id, p.receipt_no, p.customer_id, c.name, p.amount, p.method, p.note,
        u.full_name, p.business_date, p.created_at
     FROM customer_payments p JOIN customers c ON c.id = p.customer_id JOIN users u ON u.id = p.user_id";

fn map_payment(r: &Row) -> rusqlite::Result<CustomerPayment> {
    Ok(CustomerPayment {
        id: r.get(0)?,
        receipt_no: r.get(1)?,
        customer_id: r.get(2)?,
        customer_name: r.get(3)?,
        amount: r.get(4)?,
        method: r.get(5)?,
        note: r.get(6)?,
        user_name: r.get(7)?,
        business_date: r.get(8)?,
        created_at: r.get(9)?,
    })
}

pub fn get_payment(conn: &Connection, id: &str) -> AppResult<CustomerPayment> {
    conn.query_row(&format!("{PAYMENT_SELECT} WHERE p.id = ?1"), [id], map_payment)
        .optional()?
        .ok_or_else(|| AppError::not_found("Payment"))
}

pub fn list_payments(conn: &Connection, f: &PaymentFilter) -> AppResult<Page<CustomerPayment>> {
    let mut w = Where::new();
    w.opt("p.customer_id = ?", &f.customer_id);
    w.opt("p.business_date >= ?", &f.date_from);
    w.opt("p.business_date <= ?", &f.date_to);
    let where_sql = w.sql();
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM customer_payments p{where_sql}"),
        params_from_iter(w.args.iter()),
        |r| r.get(0),
    )?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 100);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let mut stmt = conn.prepare(&format!("{PAYMENT_SELECT}{where_sql} ORDER BY p.created_at DESC LIMIT ? OFFSET ?"))?;
    let items = stmt.query_map(params_from_iter(args.iter()), map_payment)?.collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}

// -------------------------------------------------------------- ledger

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEntry {
    pub date: String,
    pub created_at: String,
    pub kind: String,
    pub reference: String,
    pub ref_id: Option<String>,
    pub description: String,
    /// Increases what the customer owes.
    pub debit: Money,
    /// Decreases what the customer owes.
    pub credit: Money,
    pub balance: Money,
}

/// Chronological account statement with running balance.
pub fn ledger(conn: &Connection, customer_id: &str) -> AppResult<Vec<LedgerEntry>> {
    let customer = get(conn, customer_id)?;
    let mut stmt = conn.prepare(
        "SELECT business_date, created_at, 'sale', invoice_no, id, 'Sale on credit', credit_amount, 0
           FROM sales WHERE customer_id = ?1 AND status = 'completed'
         UNION ALL
         SELECT business_date, created_at, 'payment', receipt_no, id, 'Payment received (' || method || ')', 0, amount
           FROM customer_payments WHERE customer_id = ?1
         UNION ALL
         SELECT business_date, created_at, 'refund', return_no, sale_id, 'Return credited to account', 0, total_refund
           FROM sale_returns WHERE customer_id = ?1 AND refund_method = 'account'
         ORDER BY 2",
    )?;
    let rows = stmt.query_map([customer_id], |r| {
        Ok(LedgerEntry {
            date: r.get(0)?,
            created_at: r.get(1)?,
            kind: r.get(2)?,
            reference: r.get(3)?,
            ref_id: r.get(4)?,
            description: r.get(5)?,
            debit: r.get(6)?,
            credit: r.get(7)?,
            balance: 0,
        })
    })?;
    let mut entries = vec![LedgerEntry {
        date: customer.created_at.chars().take(10).collect(),
        created_at: customer.created_at.clone(),
        kind: "opening".into(),
        reference: "-".into(),
        ref_id: None,
        description: "Opening balance".into(),
        debit: customer.opening_balance.max(0),
        credit: (-customer.opening_balance).max(0),
        balance: customer.opening_balance,
    }];
    let mut running = customer.opening_balance;
    for row in rows {
        let mut e = row?;
        // Sale rows with nothing on credit only matter for history.
        if e.kind == "sale" && e.debit == 0 {
            continue;
        }
        running += e.debit - e.credit;
        e.balance = running;
        entries.push(e);
    }
    Ok(entries)
}
