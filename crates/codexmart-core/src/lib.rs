//! CODE X MART business core.
//!
//! Everything that does not depend on the desktop shell lives here: domain
//! rules (pricing, costing, validation), the SQLite schema and repositories,
//! application services, reports and backup/restore. The Tauri application in
//! `src-tauri` is a thin command layer over these services, which keeps the
//! rules testable and reusable by a future cloud API.
//!
//! Layering (each layer only calls the ones below it):
//!
//! ```text
//!   services / reports / backup   business workflows, permissions, audit
//!   repo                          SQL for one aggregate per module
//!   domain                        pure calculations and validation
//!   db                            connection setup and migrations
//! ```

pub mod backup;
pub mod context;
pub mod db;
pub mod domain;
pub mod error;
pub mod escpos;
pub mod ids;
pub mod money;
pub mod repo;
pub mod reports;
pub mod services;

pub use context::{Actor, Ctx, Permission, Role};
pub use error::{AppError, AppResult};

pub const APP_NAME: &str = "CODE X MART";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
