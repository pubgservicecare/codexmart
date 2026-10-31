//! Operating expenses (rent, electricity, salaries, ...).
//!
//! Expenses are never edited or deleted; a mistake is voided (with a reason)
//! and entered again, so the history always shows what happened.

use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;

use super::{audit, begin, user_id};
use crate::context::{Ctx, Permission};
use crate::domain::payments::PaymentMethod;
use crate::domain::validation as v;
use crate::error::{AppError, AppResult};
use crate::ids::new_id;
use crate::money::{format_amount, Money};
use crate::repo::catalog::{self as lookups, LookupKind};
use crate::repo::expenses::{self as repo, Expense, ExpenseFilter, ExpenseRow};
use crate::repo::{sequences, Page};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseInput {
    pub client_txn_id: String,
    pub category_id: String,
    pub amount: Money,
    pub method: PaymentMethod,
    #[serde(default)]
    pub payee: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub business_date: Option<String>,
}

pub fn create(conn: &mut Connection, ctx: &Ctx, input: ExpenseInput) -> AppResult<Expense> {
    ctx.require(Permission::ManageExpenses)?;
    let txn = v::client_txn_id(&input.client_txn_id)?;
    v::positive("Amount", input.amount)?;
    let payee = v::optional("Paid to", input.payee.as_deref(), 100)?;
    let note = v::optional_multiline("Note", input.note.as_deref(), 500)?;
    let date = v::business_date("Expense date", input.business_date.as_deref(), ctx.today)?;
    let uid = user_id(ctx)?.to_string();
    let tx = begin(conn)?;
    if let Some(existing) = repo::id_by_txn(&tx, &txn)? {
        return repo::get(&tx, &existing);
    }
    let category = lookups::get(&tx, LookupKind::ExpenseCategory, &input.category_id)?;
    if !category.is_active {
        return Err(AppError::validation(format!("The category '{}' is disabled.", category.name)));
    }
    let id = new_id();
    let no = sequences::next(&tx, "expense")?;
    repo::insert(
        &tx,
        &ExpenseRow {
            id: &id,
            expense_no: &no,
            client_txn_id: &txn,
            category_id: &category.id,
            amount: input.amount,
            method: input.method.as_str(),
            payee: payee.as_deref(),
            note: note.as_deref(),
            business_date: &date,
            user_id: &uid,
            created_at: &ctx.ts(),
        },
    )?;
    audit::record(
        &tx,
        ctx,
        "expense.created",
        "expense",
        Some(&id),
        format!("Expense {no}: {} for {}", format_amount(input.amount, 2), category.name),
        Some(json!({ "amount": input.amount, "date": date })),
    )?;
    tx.commit()?;
    repo::get(conn, &id)
}

pub fn void(conn: &mut Connection, ctx: &Ctx, id: &str, reason: &str) -> AppResult<Expense> {
    ctx.require(Permission::ManageExpenses)?;
    let reason = v::required("Reason", reason, 200)?;
    let tx = begin(conn)?;
    let e = repo::get(&tx, id)?;
    repo::mark_void(&tx, id, ctx.user_id(), &reason, &ctx.ts())?;
    audit::record(
        &tx,
        ctx,
        "expense.voided",
        "expense",
        Some(id),
        format!("Expense {} ({}) voided: {reason}", e.expense_no, format_amount(e.amount, 2)),
        None,
    )?;
    tx.commit()?;
    repo::get(conn, id)
}

pub fn list(conn: &Connection, ctx: &Ctx, f: &ExpenseFilter) -> AppResult<Page<Expense>> {
    ctx.require(Permission::ManageExpenses)?;
    repo::list(conn, f)
}
