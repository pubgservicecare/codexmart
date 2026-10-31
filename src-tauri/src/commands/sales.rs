use codexmart_core::repo::customers::{Customer, CustomerFilter, CustomerPayment, LedgerEntry, PaymentFilter};
use codexmart_core::repo::sales::{
    ReturnFilter, SaleDetail, SaleFilter, SaleReturnDetail, SaleReturnSummary, SaleSummary,
};
use codexmart_core::repo::Page;
use codexmart_core::services::cash::{self, CashClosing, CashDay, CloseInput};
use codexmart_core::services::customers::{self, CustomerInput, ReceivePaymentInput};
use codexmart_core::services::returns::{self, ReturnInput, ReturnableLine};
use codexmart_core::services::sales::{self, Cart, CheckoutInput, Quote};
use codexmart_core::services::settings;
use codexmart_core::{escpos, AppError, Permission};
use tauri::State;

use super::CmdResult;
use crate::printing;
use crate::state::AppState;

#[tauri::command]
pub async fn sales_quote(state: State<'_, AppState>, cart: Cart) -> CmdResult<Quote> {
    state.run(|c, ctx| sales::quote(c, ctx, &cart))
}

#[tauri::command]
pub async fn sales_checkout(state: State<'_, AppState>, input: CheckoutInput) -> CmdResult<SaleDetail> {
    state.run(|c, ctx| sales::checkout(c, ctx, input))
}

#[tauri::command]
pub async fn sales_list(state: State<'_, AppState>, filter: SaleFilter) -> CmdResult<Page<SaleSummary>> {
    state.run(|c, ctx| sales::list(c, ctx, &filter))
}

#[tauri::command]
pub async fn sales_get(state: State<'_, AppState>, id: String) -> CmdResult<SaleDetail> {
    state.run(|c, ctx| sales::get(c, ctx, &id))
}

#[tauri::command]
pub async fn sales_void(state: State<'_, AppState>, id: String, reason: String) -> CmdResult<SaleDetail> {
    state.run(|c, ctx| sales::void_sale(c, ctx, &id, &reason))
}

#[tauri::command]
pub async fn sales_returnable(state: State<'_, AppState>, sale_id: String) -> CmdResult<Vec<ReturnableLine>> {
    state.run(|c, ctx| returns::returnable(c, ctx, &sale_id))
}

#[tauri::command]
pub async fn sales_return(state: State<'_, AppState>, input: ReturnInput) -> CmdResult<SaleReturnDetail> {
    state.run(|c, ctx| returns::create_return(c, ctx, input))
}

#[tauri::command]
pub async fn sales_returns_list(
    state: State<'_, AppState>,
    filter: ReturnFilter,
) -> CmdResult<Page<SaleReturnSummary>> {
    state.run(|c, ctx| returns::list_returns(c, ctx, &filter))
}

#[tauri::command]
pub async fn sales_return_get(state: State<'_, AppState>, id: String) -> CmdResult<SaleReturnDetail> {
    state.run(|c, ctx| returns::get_return(c, ctx, &id))
}

#[tauri::command]
pub async fn customers_list(state: State<'_, AppState>, filter: CustomerFilter) -> CmdResult<Page<Customer>> {
    state.run(|c, ctx| customers::list(c, ctx, &filter))
}

#[tauri::command]
pub async fn customers_get(state: State<'_, AppState>, id: String) -> CmdResult<Customer> {
    state.run(|c, ctx| customers::get(c, ctx, &id))
}

#[tauri::command]
pub async fn customers_create(state: State<'_, AppState>, input: CustomerInput) -> CmdResult<Customer> {
    state.run(|c, ctx| customers::create(c, ctx, input))
}

#[tauri::command]
pub async fn customers_update(state: State<'_, AppState>, id: String, input: CustomerInput) -> CmdResult<Customer> {
    state.run(|c, ctx| customers::update(c, ctx, &id, input))
}

#[tauri::command]
pub async fn customers_ledger(state: State<'_, AppState>, id: String) -> CmdResult<Vec<LedgerEntry>> {
    state.run(|c, ctx| customers::ledger(c, ctx, &id))
}

#[tauri::command]
pub async fn customers_receive_payment(
    state: State<'_, AppState>,
    input: ReceivePaymentInput,
) -> CmdResult<CustomerPayment> {
    state.run(|c, ctx| customers::receive_payment(c, ctx, input))
}

#[tauri::command]
pub async fn customer_payments_list(
    state: State<'_, AppState>,
    filter: PaymentFilter,
) -> CmdResult<Page<CustomerPayment>> {
    state.run(|c, ctx| customers::list_payments(c, ctx, &filter))
}

#[tauri::command]
pub async fn cash_preview(state: State<'_, AppState>, date: Option<String>) -> CmdResult<CashDay> {
    state.run(|c, ctx| cash::preview(c, ctx, date.as_deref()))
}

#[tauri::command]
pub async fn cash_close(state: State<'_, AppState>, input: CloseInput) -> CmdResult<CashClosing> {
    state.run(|c, ctx| cash::close(c, ctx, input))
}

#[tauri::command]
pub async fn cash_closings(state: State<'_, AppState>, from: String, to: String) -> CmdResult<Vec<CashClosing>> {
    state.run(|c, ctx| cash::list_closings(c, ctx, &from, &to))
}

// ------------------------------------------------------------- printing

#[tauri::command]
pub async fn printers_list(state: State<'_, AppState>) -> CmdResult<Vec<printing::PrinterInfo>> {
    state.session().ok_or(AppError::Unauthenticated)?;
    printing::list().map_err(AppError::Internal)
}

/// Prints a sale receipt directly to the configured thermal printer.
#[tauri::command]
pub async fn print_receipt_direct(state: State<'_, AppState>, sale_id: String, reprint: bool) -> CmdResult<()> {
    let (bytes, printer, copies, invoice) = state.run(|c, ctx| {
        let sale = sales::get(c, ctx, &sale_id)?;
        let s = settings::load(c)?;
        if s.receipt_printer.trim().is_empty() {
            return Err(AppError::validation("Choose a receipt printer in Settings → Receipt & printer."));
        }
        Ok((
            escpos::sale_receipt(&sale, &s, reprint),
            s.receipt_printer.clone(),
            s.receipt_copies,
            sale.summary.invoice_no,
        ))
    })?;
    for _ in 0..copies.max(1) {
        printing::print_raw(&printer, &format!("CODE X MART {invoice}"), &bytes).map_err(AppError::Internal)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn print_test_direct(state: State<'_, AppState>, printer: String, paper: String) -> CmdResult<()> {
    let bytes = state.run(|c, ctx| {
        ctx.require(Permission::ManageSettings)?;
        let mut s = settings::load(c)?;
        s.receipt_paper = paper;
        Ok(escpos::test_page(&s))
    })?;
    printing::print_raw(&printer, "CODE X MART test page", &bytes).map_err(AppError::Internal)
}
