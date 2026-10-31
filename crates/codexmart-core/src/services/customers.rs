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
use crate::repo::customers::{
    self as repo, Customer, CustomerFilter, CustomerPayment, CustomerRow, LedgerEntry, PaymentFilter, PaymentRow,
};
use crate::repo::{sequences, Page};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerInput {
    pub name: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub opening_balance: Money,
    #[serde(default)]
    pub credit_limit: Option<Money>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

struct Clean {
    name: String,
    phone: Option<String>,
    email: Option<String>,
    address: Option<String>,
    notes: Option<String>,
}

fn clean(input: &CustomerInput) -> AppResult<Clean> {
    if let Some(l) = input.credit_limit {
        v::non_negative("Credit limit", l)?;
    }
    Ok(Clean {
        name: v::required("Customer name", &input.name, 100)?,
        phone: v::optional("Phone", input.phone.as_deref(), 40)?,
        email: v::email(input.email.as_deref())?,
        address: v::optional_multiline("Address", input.address.as_deref(), 300)?,
        notes: v::optional_multiline("Notes", input.notes.as_deref(), 500)?,
    })
}

pub fn list(conn: &Connection, ctx: &Ctx, f: &CustomerFilter) -> AppResult<Page<Customer>> {
    ctx.require(Permission::ViewSales)?;
    repo::list(conn, f)
}

pub fn get(conn: &Connection, ctx: &Ctx, id: &str) -> AppResult<Customer> {
    ctx.require(Permission::ViewSales)?;
    repo::get(conn, id)
}

pub fn create(conn: &mut Connection, ctx: &Ctx, input: CustomerInput) -> AppResult<Customer> {
    ctx.require(Permission::ManageCustomers)?;
    let c = clean(&input)?;
    if input.opening_balance != 0 && !ctx.can(Permission::ManagePurchasing) {
        return Err(AppError::Forbidden("Only a manager can set an opening balance.".into()));
    }
    let tx = begin(conn)?;
    let id = new_id();
    repo::insert(
        &tx,
        &CustomerRow {
            id: &id,
            name: &c.name,
            phone: c.phone.as_deref(),
            email: c.email.as_deref(),
            address: c.address.as_deref(),
            opening_balance: input.opening_balance,
            credit_limit: input.credit_limit,
            notes: c.notes.as_deref(),
            is_active: input.is_active.unwrap_or(true),
            now: &ctx.ts(),
        },
    )?;
    audit::record(&tx, ctx, "customer.created", "customer", Some(&id), format!("Customer '{}' added", c.name), None)?;
    tx.commit()?;
    repo::get(conn, &id)
}

pub fn update(conn: &mut Connection, ctx: &Ctx, id: &str, input: CustomerInput) -> AppResult<Customer> {
    ctx.require(Permission::ManageCustomers)?;
    let c = clean(&input)?;
    let tx = begin(conn)?;
    let old = repo::get(&tx, id)?;
    if old.opening_balance != input.opening_balance && !ctx.can(Permission::ManagePurchasing) {
        return Err(AppError::Forbidden("Only a manager can change an opening balance.".into()));
    }
    repo::update(
        &tx,
        &CustomerRow {
            id,
            name: &c.name,
            phone: c.phone.as_deref(),
            email: c.email.as_deref(),
            address: c.address.as_deref(),
            opening_balance: input.opening_balance,
            credit_limit: input.credit_limit,
            notes: c.notes.as_deref(),
            is_active: input.is_active.unwrap_or(old.is_active),
            now: &ctx.ts(),
        },
    )?;
    let mut summary = format!("Customer '{}' updated", c.name);
    if old.opening_balance != input.opening_balance {
        summary.push_str(&format!(
            "; opening balance {} → {}",
            format_amount(old.opening_balance, 2),
            format_amount(input.opening_balance, 2)
        ));
    }
    audit::record(&tx, ctx, "customer.updated", "customer", Some(id), summary, None)?;
    tx.commit()?;
    repo::get(conn, id)
}

pub fn ledger(conn: &Connection, ctx: &Ctx, id: &str) -> AppResult<Vec<LedgerEntry>> {
    ctx.require(Permission::ViewSales)?;
    repo::ledger(conn, id)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivePaymentInput {
    pub client_txn_id: String,
    pub customer_id: String,
    pub amount: Money,
    pub method: PaymentMethod,
    #[serde(default)]
    pub note: Option<String>,
}

pub fn receive_payment(conn: &mut Connection, ctx: &Ctx, input: ReceivePaymentInput) -> AppResult<CustomerPayment> {
    ctx.require(Permission::ReceiveCustomerPayment)?;
    let txn = v::client_txn_id(&input.client_txn_id)?;
    v::positive("Amount", input.amount)?;
    let note = v::optional("Note", input.note.as_deref(), 200)?;
    let uid = user_id(ctx)?.to_string();
    let tx = begin(conn)?;
    if let Some(existing) = repo::payment_id_by_txn(&tx, &txn)? {
        return repo::get_payment(&tx, &existing);
    }
    let customer = repo::get(&tx, &input.customer_id)?;
    let id = new_id();
    let receipt_no = sequences::next(&tx, "customer_payment")?;
    repo::insert_payment(
        &tx,
        &PaymentRow {
            id: &id,
            receipt_no: &receipt_no,
            client_txn_id: &txn,
            customer_id: &customer.id,
            amount: input.amount,
            method: input.method.as_str(),
            note: note.as_deref(),
            user_id: &uid,
            business_date: &ctx.date(),
            created_at: &ctx.ts(),
        },
    )?;
    audit::record(
        &tx,
        ctx,
        "customer.payment",
        "customer_payment",
        Some(&id),
        format!("Payment {receipt_no} of {} received from {}", format_amount(input.amount, 2), customer.name),
        Some(json!({ "customerId": customer.id, "amount": input.amount, "method": input.method })),
    )?;
    tx.commit()?;
    repo::get_payment(conn, &id)
}

pub fn list_payments(conn: &Connection, ctx: &Ctx, f: &PaymentFilter) -> AppResult<Page<CustomerPayment>> {
    ctx.require(Permission::ViewSales)?;
    repo::list_payments(conn, f)
}
