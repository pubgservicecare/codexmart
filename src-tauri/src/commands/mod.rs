//! Tauri commands: thin adapters from the UI to `codexmart_core` services.
//! No business rules live here; each command only passes the signed-in
//! user's context and the request to one service function.

pub mod app;
pub mod backup;
pub mod catalog;
pub mod license;
pub mod purchasing;
pub mod reports;
pub mod sales;

pub type CmdResult<T> = Result<T, codexmart_core::AppError>;
