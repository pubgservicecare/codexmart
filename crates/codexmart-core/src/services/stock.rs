//! The stock engine: the only code that changes stock levels.
//!
//! Every change updates the product's quantity/value AND appends a stock
//! movement in the same transaction, so `products.stock_qty` always equals
//! the sum of its movements (verified by `inventory::verify_ledger`).

use rusqlite::Connection;

use crate::context::Ctx;
use crate::domain::costing::{self, StockState};
use crate::error::{AppError, AppResult};
use crate::money::{self, format_qty, Money, Qty};
use crate::repo::products;
use crate::repo::stock::{self as repo, NewBatch, NewMovement};

/// What a movement belongs to.
#[derive(Debug, Clone, Copy)]
pub struct Refs<'a> {
    pub ref_type: &'a str,
    pub ref_id: &'a str,
    pub ref_no: &'a str,
    pub ref_line_id: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub note: Option<&'a str>,
}

pub enum BatchTarget<'a> {
    /// No batch information.
    None,
    /// Create a new batch (only if the product is batch tracked).
    New { batch_no: Option<&'a str>, expiry_date: Option<&'a str>, source_type: &'a str, source_id: &'a str },
    /// Put the quantity back into an existing batch.
    Existing(&'a str),
}

pub struct Issued {
    /// Cost of goods issued (what COGS records).
    pub cost: Money,
    /// Change in inventory value (negative).
    pub value_change: Money,
}

pub struct Received {
    pub batch_id: Option<String>,
    pub value_change: Money,
}

#[allow(clippy::too_many_arguments)]
fn post(
    conn: &Connection,
    ctx: &Ctx,
    product_id: &str,
    batch_id: Option<&str>,
    movement_type: &str,
    qty_change: Qty,
    value_change: Money,
    new_cost: Option<Money>,
    refs: &Refs,
) -> AppResult<()> {
    let (qty, value, cost, _) = products::stock_state(conn, product_id)?;
    let qty_after = qty + qty_change;
    // Value can never go below zero; rounding residue is absorbed here.
    let value_after = (value + value_change).max(0);
    let value_change = value_after - value;
    products::write_stock(conn, product_id, qty_after, value_after, new_cost.unwrap_or(cost), &ctx.ts())?;
    repo::insert_movement(
        conn,
        &NewMovement {
            product_id,
            batch_id,
            movement_type,
            qty_change,
            value_change,
            qty_after,
            value_after,
            ref_type: refs.ref_type,
            ref_id: refs.ref_id,
            ref_no: refs.ref_no,
            ref_line_id: refs.ref_line_id,
            reason: refs.reason,
            note: refs.note,
            user_id: ctx.user_id(),
            business_date: &ctx.date(),
            created_at: &ctx.ts(),
        },
    )?;
    Ok(())
}

fn insufficient(conn: &Connection, product_id: &str, available: Qty, requested: Qty) -> AppError {
    let name = products::get(conn, product_id).map(|p| p.display_name).unwrap_or_else(|_| "this product".into());
    AppError::InsufficientStock(format!(
        "Not enough stock for {name}: {} available, {} needed.",
        format_qty(available),
        format_qty(requested)
    ))
}

/// Removes stock at weighted-average cost. Batch-tracked products are
/// drawn first-expiry-first-out unless a specific batch is given.
#[allow(clippy::too_many_arguments)]
pub fn issue(
    conn: &Connection,
    ctx: &Ctx,
    product_id: &str,
    qty: Qty,
    movement_type: &str,
    refs: &Refs,
    allow_negative: bool,
    batch_id: Option<&str>,
) -> AppResult<Issued> {
    if qty <= 0 {
        return Err(AppError::validation("Quantity must be more than zero."));
    }
    let (sq, sv, sc, tracked) = products::stock_state(conn, product_id)?;
    if !allow_negative && sq < qty {
        return Err(insufficient(conn, product_id, sq, qty));
    }
    let (after, cost) = costing::issue(StockState { qty: sq, value: sv, unit_cost: sc }, qty);
    let value_change = after.value - sv;

    let mut allocations: Vec<(Option<String>, Qty)> = Vec::new();
    if tracked {
        let mut remaining = qty;
        if let Some(b) = batch_id {
            let available = repo::batch_remaining(conn, b, product_id)?;
            if available < qty {
                return Err(AppError::InsufficientStock(format!(
                    "The selected batch only has {} left.",
                    format_qty(available)
                )));
            }
            allocations.push((Some(b.to_string()), qty));
            remaining = 0;
        }
        for (bid, available) in repo::batches_fefo(conn, product_id)? {
            if remaining == 0 {
                break;
            }
            let take = available.min(remaining);
            allocations.push((Some(bid), take));
            remaining -= take;
        }
        if remaining > 0 {
            allocations.push((None, remaining));
        }
    } else {
        allocations.push((None, qty));
    }

    let weights: Vec<Qty> = allocations.iter().map(|a| a.1).collect();
    let values = money::allocate(value_change, &weights);
    for ((bid, q), v) in allocations.iter().zip(values) {
        if let Some(b) = bid {
            repo::change_batch_qty(conn, b, -q, &ctx.ts())?;
        }
        post(conn, ctx, product_id, bid.as_deref(), movement_type, -q, v, None, refs)?;
    }
    Ok(Issued { cost, value_change })
}

/// Adds stock with a known total cost and updates the average cost.
#[allow(clippy::too_many_arguments)]
pub fn receive(
    conn: &Connection,
    ctx: &Ctx,
    product_id: &str,
    qty: Qty,
    total_value: Money,
    movement_type: &str,
    refs: &Refs,
    batch: BatchTarget,
) -> AppResult<Received> {
    if qty <= 0 {
        return Err(AppError::validation("Quantity must be more than zero."));
    }
    if total_value < 0 {
        return Err(AppError::validation("Cost cannot be negative."));
    }
    let (sq, sv, sc, tracked) = products::stock_state(conn, product_id)?;
    let after = costing::receive(StockState { qty: sq, value: sv, unit_cost: sc }, qty, total_value);
    let batch_id = match batch {
        BatchTarget::None => None,
        BatchTarget::New { batch_no, expiry_date, source_type, source_id } if tracked => Some(repo::insert_batch(
            conn,
            &NewBatch {
                product_id,
                batch_no,
                expiry_date,
                qty,
                unit_cost: money::unit_price_of(total_value, qty),
                source_type,
                source_id: Some(source_id),
                received_date: &ctx.date(),
                now: &ctx.ts(),
            },
        )?),
        BatchTarget::New { .. } => None,
        BatchTarget::Existing(b) => {
            repo::change_batch_qty(conn, b, qty, &ctx.ts())?;
            Some(b.to_string())
        }
    };
    let value_change = after.value - sv;
    post(conn, ctx, product_id, batch_id.as_deref(), movement_type, qty, value_change, Some(after.unit_cost), refs)?;
    Ok(Received { batch_id, value_change })
}

/// Removes stock at a given value (returns to supplier). Never allows
/// negative stock: goods that are not on hand cannot be sent back.
#[allow(clippy::too_many_arguments)]
pub fn issue_at_value(
    conn: &Connection,
    ctx: &Ctx,
    product_id: &str,
    qty: Qty,
    value: Money,
    movement_type: &str,
    refs: &Refs,
    batch_id: Option<&str>,
) -> AppResult<Money> {
    if qty <= 0 {
        return Err(AppError::validation("Quantity must be more than zero."));
    }
    let (sq, sv, sc, tracked) = products::stock_state(conn, product_id)?;
    if sq < qty {
        return Err(insufficient(conn, product_id, sq, qty));
    }
    let after = costing::issue_at_value(StockState { qty: sq, value: sv, unit_cost: sc }, qty, value);
    let used_batch = match (tracked, batch_id) {
        (true, Some(b)) => {
            let available = repo::batch_remaining(conn, b, product_id)?;
            let take = available.min(qty);
            if take > 0 {
                repo::change_batch_qty(conn, b, -take, &ctx.ts())?;
                Some(b)
            } else {
                None
            }
        }
        _ => None,
    };
    let value_change = after.value - sv;
    post(conn, ctx, product_id, used_batch, movement_type, -qty, value_change, Some(after.unit_cost), refs)?;
    Ok(value_change)
}
