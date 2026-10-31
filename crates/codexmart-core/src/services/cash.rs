//! End-of-day cash drawer closing.
//!
//! Expected cash = opening float + cash sales + cash received from
//! customers - cash refunds - expenses paid in cash - suppliers paid in cash.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{audit, begin, user_id};
use crate::context::{Ctx, Permission};
use crate::domain::validation as v;
use crate::error::{AppError, AppResult};
use crate::ids::new_id;
use crate::money::{format_amount, Money};
use crate::repo::sequences;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodTotal {
    pub method: String,
    pub amount: Money,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashDay {
    pub business_date: String,
    pub invoices: i64,
    pub cash_sales: Money,
    pub cash_customer_receipts: Money,
    pub cash_refunds: Money,
    pub cash_expenses: Money,
    pub cash_supplier_payments: Money,
    /// Net cash movement for the day (expected cash excluding the float).
    pub net_cash: Money,
    pub sales_by_method: Vec<MethodTotal>,
    pub closing: Option<CashClosing>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashClosing {
    pub id: String,
    pub closing_no: String,
    pub business_date: String,
    pub opening_float: Money,
    pub cash_sales: Money,
    pub cash_customer_receipts: Money,
    pub cash_refunds: Money,
    pub cash_expenses: Money,
    pub cash_supplier_payments: Money,
    pub expected_cash: Money,
    pub counted_cash: Money,
    pub difference: Money,
    pub note: Option<String>,
    pub user_name: String,
    pub created_at: String,
}

const CLOSING_SELECT: &str = "SELECT c.id, c.closing_no, c.business_date, c.opening_float, c.cash_sales,
        c.cash_customer_receipts, c.cash_refunds, c.cash_expenses, c.cash_supplier_payments, c.expected_cash,
        c.counted_cash, c.difference, c.note, u.full_name, c.created_at
     FROM cash_closings c JOIN users u ON u.id = c.user_id";

fn map_closing(r: &Row) -> rusqlite::Result<CashClosing> {
    Ok(CashClosing {
        id: r.get(0)?,
        closing_no: r.get(1)?,
        business_date: r.get(2)?,
        opening_float: r.get(3)?,
        cash_sales: r.get(4)?,
        cash_customer_receipts: r.get(5)?,
        cash_refunds: r.get(6)?,
        cash_expenses: r.get(7)?,
        cash_supplier_payments: r.get(8)?,
        expected_cash: r.get(9)?,
        counted_cash: r.get(10)?,
        difference: r.get(11)?,
        note: r.get(12)?,
        user_name: r.get(13)?,
        created_at: r.get(14)?,
    })
}

fn sum(conn: &Connection, sql: &str, date: &str) -> AppResult<Money> {
    Ok(conn.query_row(sql, [date], |r| r.get::<_, Option<Money>>(0))?.unwrap_or(0))
}

/// Cash figures for one business day.
pub fn day_figures(conn: &Connection, date: &str) -> AppResult<CashDay> {
    let cash_sales = sum(
        conn,
        "SELECT SUM(p.amount) FROM sale_payments p JOIN sales s ON s.id = p.sale_id
         WHERE s.business_date = ?1 AND s.status = 'completed' AND p.method = 'cash'",
        date,
    )?;
    let cash_customer_receipts =
        sum(conn, "SELECT SUM(amount) FROM customer_payments WHERE business_date = ?1 AND method = 'cash'", date)?;
    let cash_refunds = sum(
        conn,
        "SELECT SUM(total_refund) FROM sale_returns WHERE business_date = ?1 AND refund_method = 'cash'",
        date,
    )?;
    let cash_expenses = sum(
        conn,
        "SELECT SUM(amount) FROM expenses WHERE business_date = ?1 AND status = 'active' AND method = 'cash'",
        date,
    )?;
    let cash_supplier_payments =
        sum(conn, "SELECT SUM(amount) FROM supplier_payments WHERE business_date = ?1 AND method = 'cash'", date)?;
    let invoices: i64 =
        conn.query_row("SELECT COUNT(*) FROM sales WHERE business_date = ?1 AND status = 'completed'", [date], |r| {
            r.get(0)
        })?;
    let mut stmt = conn.prepare(
        "SELECT p.method, SUM(p.amount) FROM sale_payments p JOIN sales s ON s.id = p.sale_id
         WHERE s.business_date = ?1 AND s.status = 'completed' GROUP BY p.method ORDER BY 2 DESC",
    )?;
    let mut sales_by_method: Vec<MethodTotal> = stmt
        .query_map([date], |r| Ok(MethodTotal { method: r.get(0)?, amount: r.get(1)? }))?
        .collect::<Result<_, _>>()?;
    let credit =
        sum(conn, "SELECT SUM(credit_amount) FROM sales WHERE business_date = ?1 AND status = 'completed'", date)?;
    if credit > 0 {
        sales_by_method.push(MethodTotal { method: "credit".into(), amount: credit });
    }
    let closing =
        conn.query_row(&format!("{CLOSING_SELECT} WHERE c.business_date = ?1"), [date], map_closing).optional()?;
    Ok(CashDay {
        business_date: date.to_string(),
        invoices,
        cash_sales,
        cash_customer_receipts,
        cash_refunds,
        cash_expenses,
        cash_supplier_payments,
        net_cash: cash_sales + cash_customer_receipts - cash_refunds - cash_expenses - cash_supplier_payments,
        sales_by_method,
        closing,
    })
}

pub fn preview(conn: &Connection, ctx: &Ctx, date: Option<&str>) -> AppResult<CashDay> {
    ctx.require(Permission::CloseCash)?;
    let date = v::business_date("Date", date, ctx.today)?;
    day_figures(conn, &date)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseInput {
    #[serde(default)]
    pub business_date: Option<String>,
    pub opening_float: Money,
    pub counted_cash: Money,
    #[serde(default)]
    pub note: Option<String>,
}

pub fn close(conn: &mut Connection, ctx: &Ctx, input: CloseInput) -> AppResult<CashClosing> {
    ctx.require(Permission::CloseCash)?;
    let date = v::business_date("Date", input.business_date.as_deref(), ctx.today)?;
    v::non_negative("Opening float", input.opening_float)?;
    v::non_negative("Counted cash", input.counted_cash)?;
    let note = v::optional_multiline("Note", input.note.as_deref(), 500)?;
    let uid = user_id(ctx)?.to_string();
    let tx = begin(conn)?;
    let day = day_figures(&tx, &date)?;
    if day.closing.is_some() {
        return Err(AppError::Conflict("The cash for this day has already been closed.".into()));
    }
    let expected = input.opening_float + day.net_cash;
    let difference = input.counted_cash - expected;
    let id = new_id();
    let no = sequences::next(&tx, "cash_closing")?;
    tx.execute(
        "INSERT INTO cash_closings (id, closing_no, business_date, opening_float, cash_sales, cash_customer_receipts,
            cash_refunds, cash_expenses, cash_supplier_payments, expected_cash, counted_cash, difference, note, user_id,
            created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            id,
            no,
            date,
            input.opening_float,
            day.cash_sales,
            day.cash_customer_receipts,
            day.cash_refunds,
            day.cash_expenses,
            day.cash_supplier_payments,
            expected,
            input.counted_cash,
            difference,
            note,
            uid,
            ctx.ts()
        ],
    )?;
    audit::record(
        &tx,
        ctx,
        "cash.closed",
        "cash_closing",
        Some(&id),
        format!(
            "Cash closed for {date}: expected {}, counted {}, difference {}",
            format_amount(expected, 2),
            format_amount(input.counted_cash, 2),
            format_amount(difference, 2)
        ),
        Some(json!({ "expected": expected, "counted": input.counted_cash })),
    )?;
    tx.commit()?;
    Ok(conn.query_row(&format!("{CLOSING_SELECT} WHERE c.id = ?1"), [&id], map_closing)?)
}

pub fn list_closings(conn: &Connection, ctx: &Ctx, from: &str, to: &str) -> AppResult<Vec<CashClosing>> {
    ctx.require(Permission::CloseCash)?;
    let mut stmt = conn
        .prepare(&format!("{CLOSING_SELECT} WHERE c.business_date BETWEEN ?1 AND ?2 ORDER BY c.business_date DESC"))?;
    let rows = stmt.query_map([from, to], map_closing)?;
    Ok(rows.collect::<Result<_, _>>()?)
}
