use codexmart_core::repo::customers::Customer;
use codexmart_core::repo::products::Product;
use codexmart_core::repo::stock::Batch;
use codexmart_core::repo::suppliers::Supplier;
use codexmart_core::reports::{
    self, CategorySalesRow, Dashboard, DateRange, ExpenseReport, InventoryValuation, NamedAmount, ProductSalesRow,
    ProfitLoss, PurchaseReport, ReturnsReport, SalesSummary,
};
use tauri::State;

use super::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn dashboard_get(state: State<'_, AppState>) -> CmdResult<Dashboard> {
    state.run(|c, ctx| reports::dashboard(c, ctx))
}

#[tauri::command]
pub async fn reports_sales_summary(
    state: State<'_, AppState>,
    range: DateRange,
    group_by: String,
) -> CmdResult<SalesSummary> {
    state.run(|c, ctx| reports::sales_summary(c, ctx, &range, &group_by))
}

#[tauri::command]
pub async fn reports_profit_loss(state: State<'_, AppState>, range: DateRange) -> CmdResult<ProfitLoss> {
    state.run(|c, ctx| reports::profit_loss(c, ctx, &range))
}

#[tauri::command]
pub async fn reports_product_sales(state: State<'_, AppState>, range: DateRange) -> CmdResult<Vec<ProductSalesRow>> {
    state.run(|c, ctx| reports::product_sales(c, ctx, &range))
}

#[tauri::command]
pub async fn reports_category_sales(state: State<'_, AppState>, range: DateRange) -> CmdResult<Vec<CategorySalesRow>> {
    state.run(|c, ctx| reports::category_sales(c, ctx, &range))
}

#[tauri::command]
pub async fn reports_purchases(state: State<'_, AppState>, range: DateRange) -> CmdResult<PurchaseReport> {
    state.run(|c, ctx| reports::purchases_report(c, ctx, &range))
}

#[tauri::command]
pub async fn reports_expenses(state: State<'_, AppState>, range: DateRange) -> CmdResult<ExpenseReport> {
    state.run(|c, ctx| reports::expenses_report(c, ctx, &range))
}

#[tauri::command]
pub async fn reports_returns(state: State<'_, AppState>, range: DateRange) -> CmdResult<ReturnsReport> {
    state.run(|c, ctx| reports::returns_report(c, ctx, &range))
}

#[tauri::command]
pub async fn reports_payment_methods(state: State<'_, AppState>, range: DateRange) -> CmdResult<Vec<NamedAmount>> {
    state.run(|c, ctx| reports::payment_methods(c, ctx, &range))
}

#[tauri::command]
pub async fn reports_supplier_balances(state: State<'_, AppState>) -> CmdResult<Vec<Supplier>> {
    state.run(|c, ctx| reports::supplier_balances(c, ctx))
}

#[tauri::command]
pub async fn reports_customer_balances(state: State<'_, AppState>) -> CmdResult<Vec<Customer>> {
    state.run(|c, ctx| reports::customer_balances(c, ctx))
}

#[tauri::command]
pub async fn reports_inventory_valuation(state: State<'_, AppState>) -> CmdResult<InventoryValuation> {
    state.run(|c, ctx| reports::inventory_valuation(c, ctx))
}

#[tauri::command]
pub async fn reports_low_stock(state: State<'_, AppState>) -> CmdResult<Vec<Product>> {
    state.run(|c, ctx| reports::low_stock(c, ctx))
}

#[tauri::command]
pub async fn reports_expiring(state: State<'_, AppState>, days: i64) -> CmdResult<Vec<Batch>> {
    state.run(|c, ctx| reports::expiring(c, ctx, days))
}
