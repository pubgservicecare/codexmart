use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{like, limit_offset, Page, Where};
use crate::error::{AppError, AppResult};
use crate::money::Money;

pub struct ExpenseRow<'a> {
    pub id: &'a str,
    pub expense_no: &'a str,
    pub client_txn_id: &'a str,
    pub category_id: &'a str,
    pub amount: Money,
    pub method: &'a str,
    pub payee: Option<&'a str>,
    pub note: Option<&'a str>,
    pub business_date: &'a str,
    pub user_id: &'a str,
    pub created_at: &'a str,
}

pub(crate) fn insert(conn: &Connection, e: &ExpenseRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO expenses (id, expense_no, client_txn_id, category_id, amount, method, payee, note, business_date,
            status, user_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'active', ?10, ?11)",
        params![
            e.id,
            e.expense_no,
            e.client_txn_id,
            e.category_id,
            e.amount,
            e.method,
            e.payee,
            e.note,
            e.business_date,
            e.user_id,
            e.created_at
        ],
    )?;
    Ok(())
}

pub(crate) fn mark_void(conn: &Connection, id: &str, user_id: Option<&str>, reason: &str, now: &str) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE expenses SET status = 'void', void_reason = ?2, voided_at = ?3, voided_by = ?4
         WHERE id = ?1 AND status = 'active'",
        params![id, reason, now, user_id],
    )?;
    if n == 0 {
        return Err(AppError::Conflict("This expense is already void.".into()));
    }
    Ok(())
}

pub fn id_by_txn(conn: &Connection, txn: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT id FROM expenses WHERE client_txn_id = ?1", [txn], |r| r.get(0)).optional()?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Expense {
    pub id: String,
    pub expense_no: String,
    pub category_id: String,
    pub category_name: String,
    pub amount: Money,
    pub method: String,
    pub payee: Option<String>,
    pub note: Option<String>,
    pub business_date: String,
    pub status: String,
    pub void_reason: Option<String>,
    pub user_name: String,
    pub created_at: String,
}

const SELECT: &str = "SELECT e.id, e.expense_no, e.category_id, c.name, e.amount, e.method, e.payee, e.note,
        e.business_date, e.status, e.void_reason, u.full_name, e.created_at
     FROM expenses e JOIN expense_categories c ON c.id = e.category_id JOIN users u ON u.id = e.user_id";

fn map(r: &Row) -> rusqlite::Result<Expense> {
    Ok(Expense {
        id: r.get(0)?,
        expense_no: r.get(1)?,
        category_id: r.get(2)?,
        category_name: r.get(3)?,
        amount: r.get(4)?,
        method: r.get(5)?,
        payee: r.get(6)?,
        note: r.get(7)?,
        business_date: r.get(8)?,
        status: r.get(9)?,
        void_reason: r.get(10)?,
        user_name: r.get(11)?,
        created_at: r.get(12)?,
    })
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Expense> {
    conn.query_row(&format!("{SELECT} WHERE e.id = ?1"), [id], map)
        .optional()?
        .ok_or_else(|| AppError::not_found("Expense"))
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExpenseFilter {
    pub category_id: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn list(conn: &Connection, f: &ExpenseFilter) -> AppResult<Page<Expense>> {
    let mut w = Where::new();
    w.opt("e.category_id = ?", &f.category_id);
    w.opt("e.status = ?", &f.status);
    w.opt("e.business_date >= ?", &f.date_from);
    w.opt("e.business_date <= ?", &f.date_to);
    if let Some(s) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        w.repeat(
            "(e.expense_no LIKE ? ESCAPE '\\' OR IFNULL(e.payee,'') LIKE ? ESCAPE '\\' OR IFNULL(e.note,'') LIKE ? ESCAPE '\\')",
            &like(s),
            3,
        );
    }
    let where_sql = w.sql();
    let total: i64 =
        conn.query_row(&format!("SELECT COUNT(*) FROM expenses e{where_sql}"), params_from_iter(w.args.iter()), |r| {
            r.get(0)
        })?;
    let (limit, offset) = limit_offset(f.limit, f.offset, 100);
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    let mut stmt = conn
        .prepare(&format!("{SELECT}{where_sql} ORDER BY e.business_date DESC, e.created_at DESC LIMIT ? OFFSET ?"))?;
    let items = stmt.query_map(params_from_iter(args.iter()), map)?.collect::<Result<_, _>>()?;
    Ok(Page { items, total })
}
