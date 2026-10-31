use codexmart_core::repo::expenses::{Expense, ExpenseFilter};
use codexmart_core::repo::purchases::{
    DocFilter, PurchaseDetail, PurchaseOrder, PurchaseReturn, PurchaseSummary, SupplierPayment,
};
use codexmart_core::repo::suppliers::{Supplier, SupplierFilter, SupplierLedgerEntry};
use codexmart_core::repo::Page;
use codexmart_core::services::expenses::{self, ExpenseInput};
use codexmart_core::services::purchasing::{self, PoInput, PurchaseInput, PurchaseReturnInput, SupplierPaymentInput};
use codexmart_core::services::suppliers::{self, SupplierInput};
use tauri::State;

use super::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn suppliers_list(state: State<'_, AppState>, filter: SupplierFilter) -> CmdResult<Page<Supplier>> {
    state.run(|c, ctx| suppliers::list(c, ctx, &filter))
}

#[tauri::command]
pub async fn suppliers_get(state: State<'_, AppState>, id: String) -> CmdResult<Supplier> {
    state.run(|c, ctx| suppliers::get(c, ctx, &id))
}

#[tauri::command]
pub async fn suppliers_create(state: State<'_, AppState>, input: SupplierInput) -> CmdResult<Supplier> {
    state.run(|c, ctx| suppliers::create(c, ctx, input))
}

#[tauri::command]
pub async fn suppliers_update(state: State<'_, AppState>, id: String, input: SupplierInput) -> CmdResult<Supplier> {
    state.run(|c, ctx| suppliers::update(c, ctx, &id, input))
}

#[tauri::command]
pub async fn suppliers_ledger(state: State<'_, AppState>, id: String) -> CmdResult<Vec<SupplierLedgerEntry>> {
    state.run(|c, ctx| suppliers::ledger(c, ctx, &id))
}

#[tauri::command]
pub async fn purchases_list(state: State<'_, AppState>, filter: DocFilter) -> CmdResult<Page<PurchaseSummary>> {
    state.run(|c, ctx| purchasing::list_purchases(c, ctx, &filter))
}

#[tauri::command]
pub async fn purchases_get(state: State<'_, AppState>, id: String) -> CmdResult<PurchaseDetail> {
    state.run(|c, ctx| purchasing::get_purchase(c, ctx, &id))
}

#[tauri::command]
pub async fn purchases_create(state: State<'_, AppState>, input: PurchaseInput) -> CmdResult<PurchaseDetail> {
    state.run(|c, ctx| purchasing::create_purchase(c, ctx, input))
}

#[tauri::command]
pub async fn po_list(state: State<'_, AppState>, filter: DocFilter) -> CmdResult<Page<PurchaseOrder>> {
    state.run(|c, ctx| purchasing::list_pos(c, ctx, &filter))
}

#[tauri::command]
pub async fn po_get(state: State<'_, AppState>, id: String) -> CmdResult<PurchaseOrder> {
    state.run(|c, ctx| purchasing::get_po(c, ctx, &id))
}

#[tauri::command]
pub async fn po_create(state: State<'_, AppState>, input: PoInput) -> CmdResult<PurchaseOrder> {
    state.run(|c, ctx| purchasing::create_po(c, ctx, input))
}

#[tauri::command]
pub async fn po_update(state: State<'_, AppState>, id: String, input: PoInput) -> CmdResult<PurchaseOrder> {
    state.run(|c, ctx| purchasing::update_po(c, ctx, &id, input))
}

#[tauri::command]
pub async fn po_set_status(state: State<'_, AppState>, id: String, action: String) -> CmdResult<PurchaseOrder> {
    state.run(|c, ctx| purchasing::set_po_status(c, ctx, &id, &action))
}

#[tauri::command]
pub async fn supplier_payments_list(state: State<'_, AppState>, filter: DocFilter) -> CmdResult<Page<SupplierPayment>> {
    state.run(|c, ctx| purchasing::list_supplier_payments(c, ctx, &filter))
}

#[tauri::command]
pub async fn supplier_payments_create(
    state: State<'_, AppState>,
    input: SupplierPaymentInput,
) -> CmdResult<SupplierPayment> {
    state.run(|c, ctx| purchasing::pay_supplier(c, ctx, input))
}

#[tauri::command]
pub async fn purchase_returns_list(state: State<'_, AppState>, filter: DocFilter) -> CmdResult<Page<PurchaseReturn>> {
    state.run(|c, ctx| purchasing::list_purchase_returns(c, ctx, &filter))
}

#[tauri::command]
pub async fn purchase_returns_create(
    state: State<'_, AppState>,
    input: PurchaseReturnInput,
) -> CmdResult<PurchaseReturn> {
    state.run(|c, ctx| purchasing::return_to_supplier(c, ctx, input))
}

#[tauri::command]
pub async fn expenses_list(state: State<'_, AppState>, filter: ExpenseFilter) -> CmdResult<Page<Expense>> {
    state.run(|c, ctx| expenses::list(c, ctx, &filter))
}

#[tauri::command]
pub async fn expenses_create(state: State<'_, AppState>, input: ExpenseInput) -> CmdResult<Expense> {
    state.run(|c, ctx| expenses::create(c, ctx, input))
}

#[tauri::command]
pub async fn expenses_void(state: State<'_, AppState>, id: String, reason: String) -> CmdResult<Expense> {
    state.run(|c, ctx| expenses::void(c, ctx, &id, &reason))
}
