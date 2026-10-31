//! Billing: quoting a cart, completing a sale, voiding and history.
//!
//! The UI never calculates totals itself; it asks for a [`quote`] whenever
//! the cart changes and the same code prices the cart again at checkout.

use std::collections::HashMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::settings::{self, Settings};
use super::stock::{self, BatchTarget, Refs};
use super::{audit, begin, user_id};
use crate::context::{Ctx, Permission, Role};
use crate::domain::payments::{self, Tender};
use crate::domain::pricing::{self, CartTotals, Discount, LineInput, PricedLine};
use crate::domain::validation as v;
use crate::error::{AppError, AppResult};
use crate::ids::new_id;
use crate::money::{self, format_amount, Money, Qty};
use crate::repo::customers;
use crate::repo::products::{self, Product};
use crate::repo::sales::{self as repo, SaleDetail, SaleFilter, SaleHeaderRow, SaleItemRow, SaleSummary};
use crate::repo::stock as stock_repo;
use crate::repo::{sequences, Page};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PriceTier {
    #[default]
    Retail,
    Wholesale,
}

impl PriceTier {
    fn as_str(self) -> &'static str {
        match self {
            PriceTier::Retail => "retail",
            PriceTier::Wholesale => "wholesale",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CartLine {
    pub product_id: String,
    pub qty: Qty,
    #[serde(default)]
    pub discount: Option<Discount>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cart {
    pub lines: Vec<CartLine>,
    #[serde(default)]
    pub bill_discount: Option<Discount>,
    #[serde(default)]
    pub price_tier: PriceTier,
    #[serde(default)]
    pub customer_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteLine {
    pub product_id: String,
    pub name: String,
    pub sku: String,
    pub barcode: Option<String>,
    pub unit_short: String,
    pub allow_decimal: bool,
    pub qty: Qty,
    pub unit_price: Money,
    #[serde(flatten)]
    pub priced: PricedLine,
    pub stock_qty: Qty,
    /// False when this cart would take stock below zero.
    pub stock_ok: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerBrief {
    pub id: String,
    pub name: String,
    pub phone: Option<String>,
    pub balance: Money,
    pub credit_limit: Option<Money>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub lines: Vec<QuoteLine>,
    pub totals: CartTotals,
    pub item_count: i64,
    pub total_qty: Qty,
    pub customer: Option<CustomerBrief>,
    pub warnings: Vec<String>,
}

struct Prepared {
    products: Vec<Product>,
    unit_prices: Vec<Money>,
    priced: Vec<PricedLine>,
    totals: CartTotals,
}

fn prepare(conn: &Connection, cart: &Cart, settings: &Settings) -> AppResult<Prepared> {
    if cart.lines.is_empty() {
        return Err(AppError::validation("The cart is empty."));
    }
    if cart.lines.len() > 500 {
        return Err(AppError::validation("A bill can have at most 500 lines."));
    }
    let wholesale = cart.price_tier == PriceTier::Wholesale && settings.enable_wholesale;
    let mut products_out = Vec::with_capacity(cart.lines.len());
    let mut unit_prices = Vec::with_capacity(cart.lines.len());
    let mut inputs = Vec::with_capacity(cart.lines.len());
    for (i, line) in cart.lines.iter().enumerate() {
        let p = products::get(conn, &line.product_id)?;
        if !p.is_active {
            return Err(AppError::validation(format!("{} is no longer for sale.", p.display_name)));
        }
        v::quantity(&format!("Line {} ({})", i + 1, p.display_name), line.qty, p.allow_decimal)?;
        let price = if wholesale { p.wholesale_price.unwrap_or(p.sale_price) } else { p.sale_price };
        inputs.push(LineInput { unit_price: price, qty: line.qty, discount: line.discount });
        unit_prices.push(price);
        products_out.push(p);
    }
    let (priced, totals) = pricing::price_cart(&inputs, cart.bill_discount)?;
    Ok(Prepared { products: products_out, unit_prices, priced, totals })
}

fn customer_brief(conn: &Connection, id: &str) -> AppResult<CustomerBrief> {
    let c = customers::get(conn, id)?;
    Ok(CustomerBrief { id: c.id, name: c.name, phone: c.phone, balance: c.balance, credit_limit: c.credit_limit })
}

fn check_discount_limit(ctx: &Ctx, settings: &Settings, totals: &CartTotals) -> AppResult<()> {
    if ctx.actor.role == Role::Cashier && settings.max_cashier_discount_bp < 10_000 {
        let limit = money::percent(totals.subtotal, settings.max_cashier_discount_bp);
        if totals.discount_total > limit {
            return Err(AppError::Forbidden(format!(
                "Discounts above {}% need a manager.",
                settings.max_cashier_discount_bp as f64 / 100.0
            )));
        }
    }
    Ok(())
}

pub fn quote(conn: &Connection, ctx: &Ctx, cart: &Cart) -> AppResult<Quote> {
    ctx.require(Permission::Sell)?;
    let settings = settings::load(conn)?;
    let prep = prepare(conn, cart, &settings)?;
    let mut needed: HashMap<&str, Qty> = HashMap::new();
    for (line, p) in cart.lines.iter().zip(&prep.products) {
        *needed.entry(p.id.as_str()).or_default() += line.qty;
    }
    let mut warnings = Vec::new();
    let lines: Vec<QuoteLine> = cart
        .lines
        .iter()
        .zip(prep.products.iter())
        .zip(prep.priced.iter().zip(prep.unit_prices.iter()))
        .map(|((line, p), (priced, price))| {
            let stock_ok = p.stock_qty >= needed[p.id.as_str()];
            QuoteLine {
                product_id: p.id.clone(),
                name: p.display_name.clone(),
                sku: p.sku.clone(),
                barcode: p.barcode.clone(),
                unit_short: p.unit_short.clone(),
                allow_decimal: p.allow_decimal,
                qty: line.qty,
                unit_price: *price,
                priced: *priced,
                stock_qty: p.stock_qty,
                stock_ok,
            }
        })
        .collect();
    for l in lines.iter().filter(|l| !l.stock_ok) {
        let w = format!("Only {} {} of {} in stock.", money::format_qty(l.stock_qty), l.unit_short, l.name);
        if !warnings.contains(&w) {
            warnings.push(w);
        }
    }
    if let Err(e) = check_discount_limit(ctx, &settings, &prep.totals) {
        warnings.push(e.to_string());
    }
    let customer = match cart.customer_id.as_deref().filter(|c| !c.is_empty()) {
        Some(id) => Some(customer_brief(conn, id)?),
        None => None,
    };
    Ok(Quote {
        item_count: lines.len() as i64,
        total_qty: cart.lines.iter().map(|l| l.qty).sum(),
        lines,
        totals: prep.totals,
        customer,
        warnings,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutInput {
    /// Generated by the till for each checkout attempt; retrying with the
    /// same id returns the original sale instead of charging twice.
    pub client_txn_id: String,
    pub cart: Cart,
    pub payments: Vec<Tender>,
    #[serde(default)]
    pub note: Option<String>,
}

fn redact(ctx: &Ctx, mut d: SaleDetail) -> SaleDetail {
    if !ctx.can(Permission::ViewCosts) {
        d.redact_costs();
    }
    d
}

pub fn checkout(conn: &mut Connection, ctx: &Ctx, input: CheckoutInput) -> AppResult<SaleDetail> {
    ctx.require(Permission::Sell)?;
    let txn = v::client_txn_id(&input.client_txn_id)?;
    let note = v::optional("Note", input.note.as_deref(), 300)?;
    let cashier = user_id(ctx)?.to_string();
    let tx = begin(conn)?;
    if let Some(existing) = repo::id_by_txn(&tx, &txn)? {
        let detail = repo::get(&tx, &existing)?;
        return Ok(redact(ctx, detail));
    }

    let settings = settings::load(&tx)?;
    let prep = prepare(&tx, &input.cart, &settings)?;
    check_discount_limit(ctx, &settings, &prep.totals)?;
    let total = prep.totals.total;

    let customer_id = input.cart.customer_id.as_deref().map(str::trim).filter(|c| !c.is_empty());
    let customer = match customer_id {
        Some(id) => {
            let c = customers::get(&tx, id)?;
            if !c.is_active {
                return Err(AppError::validation("This customer account is disabled."));
            }
            Some(c)
        }
        None => None,
    };

    let settlement = payments::settle(total, &input.payments)?;
    if settlement.credit > 0 {
        let c = customer.as_ref().ok_or_else(|| {
            AppError::validation(
                "The payment is less than the total. Collect the full amount or select a customer for credit.",
            )
        })?;
        if !settings.enable_credit_sales {
            return Err(AppError::validation("Credit sales are turned off in Settings."));
        }
        if let Some(limit) = c.credit_limit {
            if c.balance + settlement.credit > limit {
                return Err(AppError::validation(format!(
                    "This would take {} over their credit limit ({}). Current balance: {}.",
                    c.name,
                    format_amount(limit, settings.currency_decimals),
                    format_amount(c.balance, settings.currency_decimals)
                )));
            }
        }
    }

    // Lines are posted before the header (their cost is only known once the
    // stock is issued), so foreign keys are checked at commit instead.
    tx.pragma_update(None, "defer_foreign_keys", "ON")?;
    let sale_id = new_id();
    let invoice_no = sequences::next(&tx, "sale")?;
    let mut cogs: Money = 0;
    for (i, ((line, p), (priced, price))) in
        input.cart.lines.iter().zip(&prep.products).zip(prep.priced.iter().zip(&prep.unit_prices)).enumerate()
    {
        let item_id = new_id();
        let refs = Refs {
            ref_type: "sale",
            ref_id: &sale_id,
            ref_no: &invoice_no,
            ref_line_id: Some(&item_id),
            reason: None,
            note: None,
        };
        let issued = stock::issue(&tx, ctx, &p.id, line.qty, "sale", &refs, settings.allow_negative_stock, None)?;
        cogs += issued.cost;
        repo::insert_item(
            &tx,
            &SaleItemRow {
                id: &item_id,
                sale_id: &sale_id,
                line_no: i as i64 + 1,
                product_id: &p.id,
                product_name: &p.display_name,
                sku: &p.sku,
                barcode: p.barcode.as_deref(),
                unit_name: &p.unit_short,
                qty: line.qty,
                unit_price: *price,
                gross_amount: priced.gross,
                line_discount: priced.line_discount,
                bill_discount_share: priced.bill_discount_share,
                net_amount: priced.net,
                cost_amount: issued.cost,
            },
        )?;
    }
    repo::insert_header(
        &tx,
        &SaleHeaderRow {
            id: &sale_id,
            invoice_no: &invoice_no,
            client_txn_id: &txn,
            customer_id: customer.as_ref().map(|c| c.id.as_str()),
            price_tier: input.cart.price_tier.as_str(),
            item_count: input.cart.lines.len() as i64,
            total_qty: input.cart.lines.iter().map(|l| l.qty).sum(),
            subtotal: prep.totals.subtotal,
            line_discount_total: prep.totals.line_discount_total,
            bill_discount: prep.totals.bill_discount,
            total,
            paid_amount: settlement.paid,
            tendered_cash: settlement.tendered_cash,
            change_amount: settlement.change,
            credit_amount: settlement.credit,
            cogs_total: cogs,
            note: note.as_deref(),
            cashier_id: &cashier,
            business_date: &ctx.date(),
            created_at: &ctx.ts(),
        },
    )?;
    for p in &settlement.applied {
        repo::insert_payment(&tx, &sale_id, p.method.as_str(), p.amount, p.reference.as_deref(), &ctx.ts())?;
    }
    audit::record(
        &tx,
        ctx,
        "sale.completed",
        "sale",
        Some(&sale_id),
        format!("Sale {invoice_no}: {} ({} line(s))", format_amount(total, 2), input.cart.lines.len()),
        Some(
            json!({ "total": total, "paid": settlement.paid, "credit": settlement.credit, "discount": prep.totals.discount_total }),
        ),
    )?;
    tx.commit()?;
    log::info!("Sale {invoice_no} completed");
    Ok(redact(ctx, repo::get(conn, &sale_id)?))
}

pub fn get(conn: &Connection, ctx: &Ctx, id: &str) -> AppResult<SaleDetail> {
    ctx.require(Permission::ViewSales)?;
    Ok(redact(ctx, repo::get(conn, id)?))
}

pub fn list(conn: &Connection, ctx: &Ctx, filter: &SaleFilter) -> AppResult<Page<SaleSummary>> {
    ctx.require(Permission::ViewSales)?;
    repo::list(conn, filter)
}

/// Cancels a sale completely, as if it never happened: stock goes back at
/// its original cost and the sale drops out of all reports. Only allowed on
/// the day of the sale and before any return; later corrections use returns.
pub fn void_sale(conn: &mut Connection, ctx: &Ctx, id: &str, reason: &str) -> AppResult<SaleDetail> {
    ctx.require(Permission::VoidSale)?;
    let reason = v::required("Reason", reason, 200)?;
    let tx = begin(conn)?;
    let sale = repo::get(&tx, id)?;
    if sale.summary.status != "completed" {
        return Err(AppError::Conflict("This sale has already been voided.".into()));
    }
    if sale.summary.business_date != ctx.date() {
        return Err(AppError::validation("Only today's sales can be voided. Use a return for older sales."));
    }
    if !sale.returns.is_empty() {
        return Err(AppError::validation("This sale already has returns, so it cannot be voided."));
    }
    for item in &sale.items {
        let refs = Refs {
            ref_type: "sale_void",
            ref_id: id,
            ref_no: &sale.summary.invoice_no,
            ref_line_id: Some(&item.id),
            reason: Some(&reason),
            note: None,
        };
        let batch = stock_repo::first_batch_for_line(&tx, &item.id)?;
        let target = match batch.as_deref() {
            Some(b) => BatchTarget::Existing(b),
            None => BatchTarget::None,
        };
        stock::receive(
            &tx,
            ctx,
            &item.product_id,
            item.qty,
            item.cost_amount.unwrap_or(0),
            "sale_void",
            &refs,
            target,
        )?;
    }
    repo::mark_void(&tx, id, ctx.user_id(), &reason, &ctx.ts())?;
    audit::record(
        &tx,
        ctx,
        "sale.voided",
        "sale",
        Some(id),
        format!("Sale {} voided: {reason}", sale.summary.invoice_no),
        Some(json!({ "total": sale.summary.total })),
    )?;
    tx.commit()?;
    Ok(redact(ctx, repo::get(conn, id)?))
}
