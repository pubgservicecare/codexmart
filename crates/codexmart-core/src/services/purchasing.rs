//! Receiving goods (purchase invoices), purchase orders, supplier payments
//! and returns to suppliers.
//!
//! A purchase adds stock at its actual net cost (line cost less its share of
//! any invoice discount) and increases what the shop owes the supplier.
//! Paying the supplier only settles that balance; it is not an expense.

use std::collections::HashSet;

use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;

use super::stock::{self, BatchTarget, Refs};
use super::{audit, begin, user_id};
use crate::context::{Ctx, Permission};
use crate::domain::payments::PaymentMethod;
use crate::domain::validation as v;
use crate::error::{AppError, AppResult};
use crate::ids::new_id;
use crate::money::{self, format_amount, Money, Qty};
use crate::repo::purchases::{
    self as repo, DocFilter, PoRow, PurchaseDetail, PurchaseItemRow, PurchaseOrder, PurchaseReturn, PurchaseReturnRow,
    PurchaseRow, PurchaseSummary, SupplierPayment, SupplierPaymentRow,
};
use crate::repo::{products, sequences, suppliers, Page};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseLineInput {
    pub product_id: String,
    pub qty: Qty,
    pub unit_cost: Money,
    #[serde(default)]
    pub batch_no: Option<String>,
    #[serde(default)]
    pub expiry_date: Option<String>,
    #[serde(default)]
    pub po_item_id: Option<String>,
    /// Optionally update the product's selling price while receiving.
    #[serde(default)]
    pub new_sale_price: Option<Money>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayNow {
    pub amount: Money,
    pub method: PaymentMethod,
    #[serde(default)]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseInput {
    pub client_txn_id: String,
    pub supplier_id: String,
    #[serde(default)]
    pub supplier_invoice_no: Option<String>,
    #[serde(default)]
    pub po_id: Option<String>,
    #[serde(default)]
    pub business_date: Option<String>,
    #[serde(default)]
    pub discount: Money,
    #[serde(default)]
    pub note: Option<String>,
    pub lines: Vec<PurchaseLineInput>,
    #[serde(default)]
    pub payment: Option<PayNow>,
}

fn ensure_supplier_active(conn: &Connection, id: &str) -> AppResult<String> {
    let s = suppliers::get(conn, id)?;
    if !s.is_active {
        return Err(AppError::validation(format!("Supplier '{}' is disabled.", s.name)));
    }
    Ok(s.name)
}

pub fn create_purchase(conn: &mut Connection, ctx: &Ctx, input: PurchaseInput) -> AppResult<PurchaseDetail> {
    ctx.require(Permission::ManagePurchasing)?;
    let txn = v::client_txn_id(&input.client_txn_id)?;
    let invoice_no = v::optional("Supplier invoice number", input.supplier_invoice_no.as_deref(), 60)?;
    let note = v::optional_multiline("Note", input.note.as_deref(), 500)?;
    let date = v::business_date("Invoice date", input.business_date.as_deref(), ctx.today)?;
    v::non_negative("Discount", input.discount)?;
    if input.lines.is_empty() {
        return Err(AppError::validation("Add at least one product to the purchase."));
    }
    let uid = user_id(ctx)?.to_string();
    // Stock movements and payments are dated on the invoice date.
    let mut doc_ctx = ctx.clone();
    doc_ctx.today = v::date("Invoice date", &date)?;
    let ctx = &doc_ctx;

    let tx = begin(conn)?;
    if let Some(existing) = repo::purchase_id_by_txn(&tx, &txn)? {
        return repo::get_purchase(&tx, &existing);
    }
    let supplier_name = ensure_supplier_active(&tx, &input.supplier_id)?;
    let po = match input.po_id.as_deref().filter(|p| !p.is_empty()) {
        Some(id) => {
            let po = repo::get_po(&tx, id)?;
            if po.supplier_id != input.supplier_id {
                return Err(AppError::validation("The purchase order belongs to a different supplier."));
            }
            if !["draft", "ordered", "partial"].contains(&po.status.as_str()) {
                return Err(AppError::validation(format!("Purchase order {} is {}.", po.po_no, po.status)));
            }
            Some(po)
        }
        None => None,
    };

    let mut line_totals = Vec::with_capacity(input.lines.len());
    for (i, l) in input.lines.iter().enumerate() {
        let p = products::get(&tx, &l.product_id)?;
        v::quantity(&format!("Line {} ({})", i + 1, p.display_name), l.qty, p.allow_decimal)?;
        v::non_negative("Unit cost", l.unit_cost)?;
        if let Some(sp) = l.new_sale_price {
            v::non_negative("Sale price", sp)?;
        }
        line_totals.push(money::mul_qty(l.unit_cost, l.qty));
    }
    let subtotal: Money = line_totals.iter().sum();
    if input.discount > subtotal {
        return Err(AppError::validation("The discount cannot be more than the invoice amount."));
    }
    let total = subtotal - input.discount;
    let shares = money::allocate(input.discount, &line_totals);

    let id = new_id();
    let purchase_no = sequences::next(&tx, "purchase")?;
    repo::insert_purchase(
        &tx,
        &PurchaseRow {
            id: &id,
            purchase_no: &purchase_no,
            client_txn_id: &txn,
            supplier_id: &input.supplier_id,
            supplier_invoice_no: invoice_no.as_deref(),
            po_id: po.as_ref().map(|p| p.id.as_str()),
            subtotal,
            discount: input.discount,
            total,
            note: note.as_deref(),
            user_id: &uid,
            business_date: &date,
            created_at: &ctx.ts(),
        },
    )?;

    for (i, l) in input.lines.iter().enumerate() {
        let net_cost = line_totals[i] - shares[i];
        let expiry = v::optional_date("Expiry date", l.expiry_date.as_deref())?;
        let batch_no = v::optional("Batch number", l.batch_no.as_deref(), 40)?;
        let refs = Refs {
            ref_type: "purchase",
            ref_id: &id,
            ref_no: &purchase_no,
            ref_line_id: None,
            reason: None,
            note: None,
        };
        let received = stock::receive(
            &tx,
            ctx,
            &l.product_id,
            l.qty,
            net_cost,
            "purchase",
            &refs,
            BatchTarget::New {
                batch_no: batch_no.as_deref(),
                expiry_date: expiry.as_deref(),
                source_type: "purchase",
                source_id: &id,
            },
        )?;
        let po_item_id = l.po_item_id.as_deref().filter(|s| !s.is_empty());
        if let (Some(po), Some(item_id)) = (&po, po_item_id) {
            repo::add_po_received(&tx, item_id, &po.id, l.qty)?;
        }
        repo::insert_purchase_item(
            &tx,
            &PurchaseItemRow {
                purchase_id: &id,
                line_no: i as i64 + 1,
                product_id: &l.product_id,
                qty: l.qty,
                unit_cost: l.unit_cost,
                line_total: line_totals[i],
                discount_share: shares[i],
                net_cost,
                batch_id: received.batch_id.as_deref(),
                batch_no: batch_no.as_deref(),
                expiry_date: expiry.as_deref(),
                po_item_id,
            },
        )?;
        if let Some(price) = l.new_sale_price {
            let p = products::get(&tx, &l.product_id)?;
            if p.sale_price != price {
                products::set_sale_price(&tx, &l.product_id, price, &ctx.ts())?;
                audit::record(
                    &tx,
                    ctx,
                    "product.price_changed",
                    "product",
                    Some(&l.product_id),
                    format!(
                        "Sale price of '{}' changed {} → {} while receiving {purchase_no}",
                        p.display_name,
                        format_amount(p.sale_price, 2),
                        format_amount(price, 2)
                    ),
                    None,
                )?;
            }
        }
    }

    if let Some(po) = &po {
        let refreshed = repo::get_po(&tx, &po.id)?;
        let complete = refreshed.items.iter().all(|i| i.qty_received >= i.qty_ordered);
        repo::set_po_status(&tx, &po.id, if complete { "received" } else { "partial" }, &ctx.ts())?;
    }

    if let Some(pay) = input.payment.as_ref().filter(|p| p.amount != 0) {
        v::positive("Payment amount", pay.amount)?;
        if pay.amount > total {
            return Err(AppError::validation("The amount paid now cannot be more than the invoice total."));
        }
        let pay_id = new_id();
        let payment_no = sequences::next(&tx, "supplier_payment")?;
        let reference = v::optional("Payment reference", pay.reference.as_deref(), 60)?;
        let pay_txn = format!("{txn}-pay");
        repo::insert_supplier_payment(
            &tx,
            &SupplierPaymentRow {
                id: &pay_id,
                payment_no: &payment_no,
                client_txn_id: &pay_txn,
                supplier_id: &input.supplier_id,
                purchase_id: Some(&id),
                amount: pay.amount,
                method: pay.method.as_str(),
                reference: reference.as_deref(),
                note: None,
                user_id: &uid,
                business_date: &date,
                created_at: &ctx.ts(),
            },
        )?;
    }

    audit::record(
        &tx,
        ctx,
        "purchase.received",
        "purchase",
        Some(&id),
        format!("Purchase {purchase_no} from {supplier_name}: {}", format_amount(total, 2)),
        Some(json!({ "total": total, "lines": input.lines.len(), "supplierInvoice": invoice_no })),
    )?;
    tx.commit()?;
    repo::get_purchase(conn, &id)
}

pub fn get_purchase(conn: &Connection, ctx: &Ctx, id: &str) -> AppResult<PurchaseDetail> {
    ctx.require(Permission::ManagePurchasing)?;
    repo::get_purchase(conn, id)
}

pub fn list_purchases(conn: &Connection, ctx: &Ctx, f: &DocFilter) -> AppResult<Page<PurchaseSummary>> {
    ctx.require(Permission::ManagePurchasing)?;
    repo::list_purchases(conn, f)
}

// ---------------------------------------------------------- purchase orders

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoLineInput {
    pub product_id: String,
    pub qty: Qty,
    pub unit_cost: Money,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoInput {
    pub supplier_id: String,
    #[serde(default)]
    pub order_date: Option<String>,
    #[serde(default)]
    pub expected_date: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    /// "draft" or "ordered".
    #[serde(default)]
    pub status: Option<String>,
    pub lines: Vec<PoLineInput>,
}

/// (product id, quantity, unit cost)
type PoLine = (String, Qty, Money);

fn clean_po_lines(conn: &Connection, lines: &[PoLineInput]) -> AppResult<(Vec<PoLine>, Money)> {
    if lines.is_empty() {
        return Err(AppError::validation("Add at least one product to the order."));
    }
    let mut out = Vec::new();
    let mut total = 0;
    for (i, l) in lines.iter().enumerate() {
        let p = products::get(conn, &l.product_id)?;
        v::quantity(&format!("Line {} ({})", i + 1, p.display_name), l.qty, p.allow_decimal)?;
        v::non_negative("Unit cost", l.unit_cost)?;
        total += money::mul_qty(l.unit_cost, l.qty);
        out.push((l.product_id.clone(), l.qty, l.unit_cost));
    }
    Ok((out, total))
}

fn po_status(input: &PoInput) -> AppResult<&'static str> {
    match input.status.as_deref() {
        None | Some("draft") => Ok("draft"),
        Some("ordered") => Ok("ordered"),
        Some(_) => Err(AppError::validation("A new order can only be a draft or ordered.")),
    }
}

pub fn create_po(conn: &mut Connection, ctx: &Ctx, input: PoInput) -> AppResult<PurchaseOrder> {
    ctx.require(Permission::ManagePurchasing)?;
    let uid = user_id(ctx)?.to_string();
    let status = po_status(&input)?;
    let order_date = v::business_date("Order date", input.order_date.as_deref(), ctx.today)?;
    let expected = v::optional_date("Expected date", input.expected_date.as_deref())?;
    let note = v::optional_multiline("Note", input.note.as_deref(), 500)?;
    let tx = begin(conn)?;
    ensure_supplier_active(&tx, &input.supplier_id)?;
    let (lines, total) = clean_po_lines(&tx, &input.lines)?;
    let id = new_id();
    let po_no = sequences::next(&tx, "purchase_order")?;
    repo::insert_po(
        &tx,
        &PoRow {
            id: &id,
            po_no: &po_no,
            supplier_id: &input.supplier_id,
            status,
            order_date: &order_date,
            expected_date: expected.as_deref(),
            note: note.as_deref(),
            total_estimate: total,
            created_by: &uid,
            now: &ctx.ts(),
        },
    )?;
    repo::replace_po_items(&tx, &id, &lines)?;
    audit::record(
        &tx,
        ctx,
        "po.created",
        "purchase_order",
        Some(&id),
        format!("Purchase order {po_no} created ({status})"),
        None,
    )?;
    tx.commit()?;
    repo::get_po(conn, &id)
}

pub fn update_po(conn: &mut Connection, ctx: &Ctx, id: &str, input: PoInput) -> AppResult<PurchaseOrder> {
    ctx.require(Permission::ManagePurchasing)?;
    let uid = user_id(ctx)?.to_string();
    let status = po_status(&input)?;
    let order_date = v::business_date("Order date", input.order_date.as_deref(), ctx.today)?;
    let expected = v::optional_date("Expected date", input.expected_date.as_deref())?;
    let note = v::optional_multiline("Note", input.note.as_deref(), 500)?;
    let tx = begin(conn)?;
    let po = repo::get_po(&tx, id)?;
    if !["draft", "ordered"].contains(&po.status.as_str()) || po.items.iter().any(|i| i.qty_received > 0) {
        return Err(AppError::validation("Only orders that have not been received can be edited."));
    }
    ensure_supplier_active(&tx, &input.supplier_id)?;
    let (lines, total) = clean_po_lines(&tx, &input.lines)?;
    repo::update_po_header(
        &tx,
        &PoRow {
            id,
            po_no: &po.po_no,
            supplier_id: &input.supplier_id,
            status,
            order_date: &order_date,
            expected_date: expected.as_deref(),
            note: note.as_deref(),
            total_estimate: total,
            created_by: &uid,
            now: &ctx.ts(),
        },
    )?;
    repo::replace_po_items(&tx, id, &lines)?;
    audit::record(
        &tx,
        ctx,
        "po.updated",
        "purchase_order",
        Some(id),
        format!("Purchase order {} updated", po.po_no),
        None,
    )?;
    tx.commit()?;
    repo::get_po(conn, id)
}

/// `ordered` (send a draft), `cancelled` (nothing received yet) or
/// `closed` (stop expecting the rest of a partly received order).
pub fn set_po_status(conn: &mut Connection, ctx: &Ctx, id: &str, action: &str) -> AppResult<PurchaseOrder> {
    ctx.require(Permission::ManagePurchasing)?;
    let tx = begin(conn)?;
    let po = repo::get_po(&tx, id)?;
    let received_any = po.items.iter().any(|i| i.qty_received > 0);
    let new_status = match (action, po.status.as_str()) {
        ("ordered", "draft") => "ordered",
        ("cancelled", "draft" | "ordered") if !received_any => "cancelled",
        ("closed", "partial") => "received",
        _ => return Err(AppError::validation(format!("A {} order cannot be {action}.", po.status))),
    };
    repo::set_po_status(&tx, id, new_status, &ctx.ts())?;
    audit::record(
        &tx,
        ctx,
        "po.status",
        "purchase_order",
        Some(id),
        format!("Purchase order {} marked {new_status}", po.po_no),
        None,
    )?;
    tx.commit()?;
    repo::get_po(conn, id)
}

pub fn get_po(conn: &Connection, ctx: &Ctx, id: &str) -> AppResult<PurchaseOrder> {
    ctx.require(Permission::ManagePurchasing)?;
    repo::get_po(conn, id)
}

pub fn list_pos(conn: &Connection, ctx: &Ctx, f: &DocFilter) -> AppResult<Page<PurchaseOrder>> {
    ctx.require(Permission::ManagePurchasing)?;
    repo::list_pos(conn, f)
}

// -------------------------------------------------------- supplier payments

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierPaymentInput {
    pub client_txn_id: String,
    pub supplier_id: String,
    #[serde(default)]
    pub purchase_id: Option<String>,
    pub amount: Money,
    pub method: PaymentMethod,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub business_date: Option<String>,
}

pub fn pay_supplier(conn: &mut Connection, ctx: &Ctx, input: SupplierPaymentInput) -> AppResult<SupplierPayment> {
    ctx.require(Permission::ManagePurchasing)?;
    let txn = v::client_txn_id(&input.client_txn_id)?;
    v::positive("Amount", input.amount)?;
    let reference = v::optional("Reference", input.reference.as_deref(), 60)?;
    let note = v::optional("Note", input.note.as_deref(), 200)?;
    let date = v::business_date("Payment date", input.business_date.as_deref(), ctx.today)?;
    let uid = user_id(ctx)?.to_string();
    let tx = begin(conn)?;
    if let Some(existing) = repo::supplier_payment_id_by_txn(&tx, &txn)? {
        return repo::get_supplier_payment(&tx, &existing);
    }
    let supplier = suppliers::get(&tx, &input.supplier_id)?;
    let purchase_id = input.purchase_id.as_deref().filter(|p| !p.is_empty());
    if let Some(pid) = purchase_id {
        let (sid, _, _) = repo::purchase_head(&tx, pid)?;
        if sid != supplier.id {
            return Err(AppError::validation("That purchase belongs to a different supplier."));
        }
    }
    let id = new_id();
    let payment_no = sequences::next(&tx, "supplier_payment")?;
    repo::insert_supplier_payment(
        &tx,
        &SupplierPaymentRow {
            id: &id,
            payment_no: &payment_no,
            client_txn_id: &txn,
            supplier_id: &supplier.id,
            purchase_id,
            amount: input.amount,
            method: input.method.as_str(),
            reference: reference.as_deref(),
            note: note.as_deref(),
            user_id: &uid,
            business_date: &date,
            created_at: &ctx.ts(),
        },
    )?;
    audit::record(
        &tx,
        ctx,
        "supplier.payment",
        "supplier_payment",
        Some(&id),
        format!("Payment {payment_no} of {} to {}", format_amount(input.amount, 2), supplier.name),
        Some(json!({ "supplierId": supplier.id, "amount": input.amount })),
    )?;
    tx.commit()?;
    repo::get_supplier_payment(conn, &id)
}

pub fn list_supplier_payments(conn: &Connection, ctx: &Ctx, f: &DocFilter) -> AppResult<Page<SupplierPayment>> {
    ctx.require(Permission::ManagePurchasing)?;
    repo::list_supplier_payments(conn, f)
}

// --------------------------------------------------------- purchase returns

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseReturnLineInput {
    pub purchase_item_id: String,
    pub qty: Qty,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseReturnInput {
    pub client_txn_id: String,
    pub purchase_id: String,
    pub lines: Vec<PurchaseReturnLineInput>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Sends goods back to the supplier at the cost they were bought for and
/// reduces the amount owed to the supplier by the same value.
pub fn return_to_supplier(conn: &mut Connection, ctx: &Ctx, input: PurchaseReturnInput) -> AppResult<PurchaseReturn> {
    ctx.require(Permission::ManagePurchasing)?;
    let txn = v::client_txn_id(&input.client_txn_id)?;
    let reason = v::optional("Reason", input.reason.as_deref(), 200)?;
    let uid = user_id(ctx)?.to_string();
    let tx = begin(conn)?;
    if let Some(existing) = repo::purchase_return_id_by_txn(&tx, &txn)? {
        return repo::get_purchase_return(&tx, &existing);
    }
    let (supplier_id, purchase_no, _) = repo::purchase_head(&tx, &input.purchase_id)?;
    let lines: Vec<&PurchaseReturnLineInput> = input.lines.iter().filter(|l| l.qty != 0).collect();
    if lines.is_empty() {
        return Err(AppError::validation("Enter the quantity being returned."));
    }
    let returned = repo::returned_by_purchase_item(&tx, &input.purchase_id)?;
    tx.pragma_update(None, "defer_foreign_keys", "ON")?;
    let id = new_id();
    let return_no = sequences::next(&tx, "purchase_return")?;
    let mut total: Money = 0;
    let mut seen = HashSet::new();
    for line in lines {
        if !seen.insert(line.purchase_item_id.as_str()) {
            return Err(AppError::validation("Each purchased line can only appear once in a return."));
        }
        let item = repo::purchase_item(&tx, &input.purchase_id, &line.purchase_item_id)?;
        let p = products::get(&tx, &item.product_id)?;
        v::quantity(&p.display_name, line.qty, p.allow_decimal)?;
        let (done_qty, done_value) = returned.get(&line.purchase_item_id).copied().unwrap_or((0, 0));
        let remaining = item.qty - done_qty;
        if line.qty > remaining {
            return Err(AppError::validation(format!(
                "{}: only {} can still be returned from this purchase.",
                p.display_name,
                money::format_qty(remaining)
            )));
        }
        let value = if line.qty == remaining {
            item.net_cost - done_value
        } else {
            money::prorate(item.net_cost, line.qty, item.qty)
        };
        let refs = Refs {
            ref_type: "purchase_return",
            ref_id: &id,
            ref_no: &return_no,
            ref_line_id: Some(&line.purchase_item_id),
            reason: reason.as_deref(),
            note: None,
        };
        stock::issue_at_value(
            &tx,
            ctx,
            &item.product_id,
            line.qty,
            value,
            "purchase_return",
            &refs,
            item.batch_id.as_deref(),
        )?;
        repo::insert_purchase_return_item(
            &tx,
            &id,
            &line.purchase_item_id,
            &item.product_id,
            item.batch_id.as_deref(),
            line.qty,
            value,
        )?;
        total += value;
    }
    repo::insert_purchase_return(
        &tx,
        &PurchaseReturnRow {
            id: &id,
            return_no: &return_no,
            client_txn_id: &txn,
            supplier_id: &supplier_id,
            purchase_id: &input.purchase_id,
            total,
            reason: reason.as_deref(),
            user_id: &uid,
            business_date: &ctx.date(),
            created_at: &ctx.ts(),
        },
    )?;
    audit::record(
        &tx,
        ctx,
        "purchase.returned",
        "purchase_return",
        Some(&id),
        format!("Return {return_no} to supplier against {purchase_no}: {}", format_amount(total, 2)),
        Some(json!({ "purchaseId": input.purchase_id, "total": total })),
    )?;
    tx.commit()?;
    repo::get_purchase_return(conn, &id)
}

pub fn get_purchase_return(conn: &Connection, ctx: &Ctx, id: &str) -> AppResult<PurchaseReturn> {
    ctx.require(Permission::ManagePurchasing)?;
    repo::get_purchase_return(conn, id)
}

pub fn list_purchase_returns(conn: &Connection, ctx: &Ctx, f: &DocFilter) -> AppResult<Page<PurchaseReturn>> {
    ctx.require(Permission::ManagePurchasing)?;
    repo::list_purchase_returns(conn, f)
}
