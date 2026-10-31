//! Application services: one function per business operation.
//!
//! Every service checks permissions, validates input, and performs all of
//! its writes inside a single IMMEDIATE transaction together with the audit
//! record. Operations that create documents are idempotent: they carry a
//! client transaction id, and repeating a request returns the original
//! document instead of creating a second one.

pub mod audit;
pub mod auth;
pub mod cash;
pub mod catalog;
pub mod customers;
pub mod expenses;
pub mod inventory;
pub mod purchasing;
pub mod returns;
pub mod sales;
pub mod settings;
pub mod stock;
pub mod suppliers;

use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::context::Ctx;
use crate::error::{AppError, AppResult};

/// Starts a write transaction that takes the database write lock up front,
/// so a long operation can never fail half-way on a lock conflict.
pub(crate) fn begin(conn: &mut Connection) -> AppResult<Transaction<'_>> {
    Ok(conn.transaction_with_behavior(TransactionBehavior::Immediate)?)
}

/// Documents must be attributed to a real signed-in user.
pub(crate) fn user_id(ctx: &Ctx) -> AppResult<&str> {
    ctx.user_id().ok_or(AppError::Unauthenticated)
}
