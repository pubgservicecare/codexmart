use rusqlite::Connection;
use serde::Deserialize;

use super::{audit, begin};
use crate::context::{Ctx, Permission};
use crate::domain::validation as v;
use crate::error::AppResult;
use crate::ids::new_id;
use crate::money::{format_amount, Money};
use crate::repo::suppliers::{self as repo, Supplier, SupplierFilter, SupplierLedgerEntry, SupplierRow};
use crate::repo::Page;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierInput {
    pub name: String,
    #[serde(default)]
    pub contact_person: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub opening_balance: Money,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

pub fn list(conn: &Connection, ctx: &Ctx, f: &SupplierFilter) -> AppResult<Page<Supplier>> {
    ctx.require(Permission::ManagePurchasing)?;
    repo::list(conn, f)
}

pub fn get(conn: &Connection, ctx: &Ctx, id: &str) -> AppResult<Supplier> {
    ctx.require(Permission::ManagePurchasing)?;
    repo::get(conn, id)
}

fn save(conn: &mut Connection, ctx: &Ctx, id: Option<&str>, input: SupplierInput) -> AppResult<Supplier> {
    ctx.require(Permission::ManagePurchasing)?;
    let name = v::required("Supplier name", &input.name, 100)?;
    let contact = v::optional("Contact person", input.contact_person.as_deref(), 80)?;
    let phone = v::optional("Phone", input.phone.as_deref(), 40)?;
    let email = v::email(input.email.as_deref())?;
    let address = v::optional_multiline("Address", input.address.as_deref(), 300)?;
    let notes = v::optional_multiline("Notes", input.notes.as_deref(), 500)?;
    let tx = begin(conn)?;
    let (supplier_id, old) = match id {
        Some(id) => (id.to_string(), Some(repo::get(&tx, id)?)),
        None => (new_id(), None),
    };
    let row = SupplierRow {
        id: &supplier_id,
        name: &name,
        contact_person: contact.as_deref(),
        phone: phone.as_deref(),
        email: email.as_deref(),
        address: address.as_deref(),
        opening_balance: input.opening_balance,
        notes: notes.as_deref(),
        is_active: input.is_active.unwrap_or_else(|| old.as_ref().map(|o| o.is_active).unwrap_or(true)),
        now: &ctx.ts(),
    };
    match &old {
        Some(o) => {
            repo::update(&tx, &row)?;
            let mut summary = format!("Supplier '{name}' updated");
            if o.opening_balance != input.opening_balance {
                summary.push_str(&format!(
                    "; opening balance {} → {}",
                    format_amount(o.opening_balance, 2),
                    format_amount(input.opening_balance, 2)
                ));
            }
            audit::record(&tx, ctx, "supplier.updated", "supplier", Some(&supplier_id), summary, None)?;
        }
        None => {
            repo::insert(&tx, &row)?;
            audit::record(
                &tx,
                ctx,
                "supplier.created",
                "supplier",
                Some(&supplier_id),
                format!("Supplier '{name}' added"),
                None,
            )?;
        }
    }
    tx.commit()?;
    repo::get(conn, &supplier_id)
}

pub fn create(conn: &mut Connection, ctx: &Ctx, input: SupplierInput) -> AppResult<Supplier> {
    save(conn, ctx, None, input)
}

pub fn update(conn: &mut Connection, ctx: &Ctx, id: &str, input: SupplierInput) -> AppResult<Supplier> {
    save(conn, ctx, Some(id), input)
}

pub fn ledger(conn: &Connection, ctx: &Ctx, id: &str) -> AppResult<Vec<SupplierLedgerEntry>> {
    ctx.require(Permission::ManagePurchasing)?;
    repo::ledger(conn, id)
}
