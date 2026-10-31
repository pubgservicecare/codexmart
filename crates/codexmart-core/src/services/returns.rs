//! Customer returns and refunds against an original invoice.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::stock::{self, BatchTarget, Refs};
use super::{audit, begin, user_id};
use crate::context::{Ctx, Permission};
use crate::domain::validation as v;
use crate::error::{AppError, AppResult};
use crate::ids::new_id;
use crate::money::{self, format_amount, Money, Qty};
use crate::repo::sales::{self as repo, ReturnFilter, ReturnRow, SaleReturnDetail, SaleReturnSummary};
use crate::repo::stock as stock_repo;
use crate::repo::{products, sequences, Page};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnableLine {
    pub sale_item_id: String,
    pub product_id: String,
    pub product_name: String,
    pub sku: String,
    pub unit_name: String,
    pub allow_decimal: bool,
    pub qty_sold: Qty,
    pub qty_returned: Qty,
    pub qty_returnable: Qty,
    pub unit_price: Money,
    pub net_amount: Money,
    pub refundable_amount: Money,
}

pub fn returnable(conn: &Connection, ctx: &Ctx, sale_id: &str) -> AppResult<Vec<ReturnableLine>> {
    ctx.require(Permission::ProcessReturn)?;
    let sale = repo::get(conn, sale_id)?;
    let returned = repo::returned_by_item(conn, sale_id)?;
    let mut out = Vec::new();
    for item in sale.items {
        let (rq, rr) = returned.get(&item.id).map(|t| (t.qty, t.refund)).unwrap_or((0, 0));
        let allow_decimal = products::get(conn, &item.product_id).map(|p| p.allow_decimal).unwrap_or(false);
        out.push(ReturnableLine {
            sale_item_id: item.id,
            product_id: item.product_id,
            product_name: item.product_name,
            sku: item.sku,
            unit_name: item.unit_name,
            allow_decimal,
            qty_sold: item.qty,
            qty_returned: rq,
            qty_returnable: item.qty - rq,
            unit_price: item.unit_price,
            net_amount: item.net_amount,
            refundable_amount: item.net_amount - rr,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnLineInput {
    pub sale_item_id: String,
    pub qty: Qty,
    /// Put the goods back on the shelf (false for damaged goods).
    pub restock: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnInput {
    pub client_txn_id: String,
    pub sale_id: String,
    pub lines: Vec<ReturnLineInput>,
    /// cash, card, bank, wallet, cheque, or account (credit the customer's balance).
    pub refund_method: String,
    #[serde(default)]
    pub reason: Option<String>,
}

pub fn create_return(conn: &mut Connection, ctx: &Ctx, input: ReturnInput) -> AppResult<SaleReturnDetail> {
    ctx.require(Permission::ProcessReturn)?;
    let txn = v::client_txn_id(&input.client_txn_id)?;
    let reason = v::optional("Reason", input.reason.as_deref(), 200)?;
    let method = input.refund_method.as_str();
    if !["cash", "card", "bank", "wallet", "cheque", "account"].contains(&method) {
        return Err(AppError::validation("Choose how the refund is given."));
    }
    let uid = user_id(ctx)?.to_string();
    let tx = begin(conn)?;
    if let Some(existing) = repo::return_id_by_txn(&tx, &txn)? {
        return repo::get_return(&tx, &existing);
    }
    let sale = repo::get(&tx, &input.sale_id)?;
    if sale.summary.status != "completed" {
        return Err(AppError::validation("A voided sale cannot have returns."));
    }
    if method == "account" && sale.summary.customer_id.is_none() {
        return Err(AppError::validation("This sale has no customer, so the refund cannot go to an account."));
    }
    let lines: Vec<&ReturnLineInput> = input.lines.iter().filter(|l| l.qty != 0).collect();
    if lines.is_empty() {
        return Err(AppError::validation("Enter the quantity being returned."));
    }
    let returned = repo::returned_by_item(&tx, &sale.summary.id)?;

    // Lines go in before the header total is known.
    tx.pragma_update(None, "defer_foreign_keys", "ON")?;
    let return_id = new_id();
    let return_no = sequences::next(&tx, "sale_return")?;
    let mut total_refund: Money = 0;
    let mut cost_reversed: Money = 0;
    let mut seen = std::collections::HashSet::new();
    for line in lines {
        if !seen.insert(line.sale_item_id.as_str()) {
            return Err(AppError::validation("Each sold line can only appear once in a return."));
        }
        let item = sale
            .items
            .iter()
            .find(|i| i.id == line.sale_item_id)
            .ok_or_else(|| AppError::validation("A returned line does not belong to this sale."))?;
        let product = products::get(&tx, &item.product_id)?;
        v::quantity(&item.product_name, line.qty, product.allow_decimal)?;
        let (done_qty, done_refund, done_cost) =
            returned.get(&item.id).map(|t| (t.qty, t.refund, t.cost)).unwrap_or((0, 0, 0));
        let remaining = item.qty - done_qty;
        if line.qty > remaining {
            return Err(AppError::validation(format!(
                "{}: only {} can still be returned.",
                item.product_name,
                money::format_qty(remaining)
            )));
        }
        let item_cost = item.cost_amount.unwrap_or(0);
        // The last return of a line takes exactly what is left, so refunds
        // and costs always add up to the original line.
        let (refund, cost) = if line.qty == remaining {
            (item.net_amount - done_refund, item_cost - done_cost)
        } else {
            (money::prorate(item.net_amount, line.qty, item.qty), money::prorate(item_cost, line.qty, item.qty))
        };
        let ri_id = repo::insert_return_item(
            &tx,
            &return_id,
            &item.id,
            &item.product_id,
            line.qty,
            refund,
            line.restock,
            cost,
        )?;
        if line.restock {
            let refs = Refs {
                ref_type: "sale_return",
                ref_id: &return_id,
                ref_no: &return_no,
                ref_line_id: Some(&ri_id),
                reason: reason.as_deref(),
                note: None,
            };
            let batch = stock_repo::first_batch_for_line(&tx, &item.id)?;
            let target = match batch.as_deref() {
                Some(b) => BatchTarget::Existing(b),
                None => BatchTarget::None,
            };
            stock::receive(&tx, ctx, &item.product_id, line.qty, cost, "sale_return", &refs, target)?;
            cost_reversed += cost;
        }
        total_refund += refund;
    }
    repo::insert_return(
        &tx,
        &ReturnRow {
            id: &return_id,
            return_no: &return_no,
            client_txn_id: &txn,
            sale_id: &sale.summary.id,
            customer_id: sale.summary.customer_id.as_deref(),
            refund_method: method,
            total_refund,
            cost_reversed,
            reason: reason.as_deref(),
            user_id: &uid,
            business_date: &ctx.date(),
            created_at: &ctx.ts(),
        },
    )?;
    audit::record(
        &tx,
        ctx,
        "sale.returned",
        "sale_return",
        Some(&return_id),
        format!(
            "Return {return_no} against {}: refund {} by {method}",
            sale.summary.invoice_no,
            format_amount(total_refund, 2)
        ),
        Some(json!({ "saleId": sale.summary.id, "refund": total_refund })),
    )?;
    tx.commit()?;
    repo::get_return(conn, &return_id)
}

pub fn get_return(conn: &Connection, ctx: &Ctx, id: &str) -> AppResult<SaleReturnDetail> {
    ctx.require(Permission::ViewSales)?;
    repo::get_return(conn, id)
}

pub fn list_returns(conn: &Connection, ctx: &Ctx, filter: &ReturnFilter) -> AppResult<Page<SaleReturnSummary>> {
    ctx.require(Permission::ViewSales)?;
    repo::list_returns(conn, filter)
}
