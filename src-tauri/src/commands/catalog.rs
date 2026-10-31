use codexmart_core::repo::catalog::{LookupItem, LookupKind, Unit};
use codexmart_core::repo::products::{Product, ProductFilter};
use codexmart_core::repo::stock::{
    Adjustment, AdjustmentFilter, Batch, BatchFilter, LedgerMismatch, Movement, MovementFilter,
};
use codexmart_core::repo::Page;
use codexmart_core::services::catalog::{self, ProductInput, UnitInput};
use codexmart_core::services::inventory::{self, AdjustmentInput};
use tauri::State;

use super::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn lookups_list(
    state: State<'_, AppState>,
    kind: LookupKind,
    include_inactive: bool,
) -> CmdResult<Vec<LookupItem>> {
    state.run(|c, ctx| catalog::list_lookups(c, ctx, kind, include_inactive))
}

#[tauri::command]
pub async fn lookups_create(state: State<'_, AppState>, kind: LookupKind, name: String) -> CmdResult<LookupItem> {
    state.run(|c, ctx| catalog::create_lookup(c, ctx, kind, &name))
}

#[tauri::command]
pub async fn lookups_update(
    state: State<'_, AppState>,
    kind: LookupKind,
    id: String,
    name: String,
    is_active: bool,
) -> CmdResult<LookupItem> {
    state.run(|c, ctx| catalog::update_lookup(c, ctx, kind, &id, &name, is_active))
}

#[tauri::command]
pub async fn units_list(state: State<'_, AppState>, include_inactive: bool) -> CmdResult<Vec<Unit>> {
    state.run(|c, ctx| catalog::list_units(c, ctx, include_inactive))
}

#[tauri::command]
pub async fn units_create(state: State<'_, AppState>, input: UnitInput) -> CmdResult<Unit> {
    state.run(|c, ctx| catalog::create_unit(c, ctx, input))
}

#[tauri::command]
pub async fn units_update(state: State<'_, AppState>, id: String, input: UnitInput) -> CmdResult<Unit> {
    state.run(|c, ctx| catalog::update_unit(c, ctx, &id, input))
}

#[tauri::command]
pub async fn products_list(state: State<'_, AppState>, filter: ProductFilter) -> CmdResult<Page<Product>> {
    state.run(|c, ctx| catalog::list_products(c, ctx, &filter))
}

#[tauri::command]
pub async fn products_get(state: State<'_, AppState>, id: String) -> CmdResult<Product> {
    state.run(|c, ctx| catalog::get_product(c, ctx, &id))
}

#[tauri::command]
pub async fn products_search(state: State<'_, AppState>, query: String, limit: Option<i64>) -> CmdResult<Vec<Product>> {
    state.run(|c, ctx| catalog::search_products(c, ctx, &query, limit.unwrap_or(30)))
}

#[tauri::command]
pub async fn products_find_by_code(state: State<'_, AppState>, code: String) -> CmdResult<Option<Product>> {
    state.run(|c, ctx| catalog::find_by_code(c, ctx, &code))
}

#[tauri::command]
pub async fn products_create(state: State<'_, AppState>, input: ProductInput) -> CmdResult<Product> {
    state.run(|c, ctx| catalog::create_product(c, ctx, input))
}

#[tauri::command]
pub async fn products_update(state: State<'_, AppState>, id: String, input: ProductInput) -> CmdResult<Product> {
    state.run(|c, ctx| catalog::update_product(c, ctx, &id, input))
}

#[tauri::command]
pub async fn products_set_active(state: State<'_, AppState>, id: String, active: bool) -> CmdResult<Product> {
    state.run(|c, ctx| catalog::set_product_active(c, ctx, &id, active))
}

#[tauri::command]
pub async fn products_generate_barcode(state: State<'_, AppState>) -> CmdResult<String> {
    state.run(catalog::generate_barcode)
}

#[tauri::command]
pub async fn inventory_adjust(state: State<'_, AppState>, input: AdjustmentInput) -> CmdResult<Adjustment> {
    state.run(|c, ctx| inventory::adjust(c, ctx, input))
}

#[tauri::command]
pub async fn inventory_adjustments(
    state: State<'_, AppState>,
    filter: AdjustmentFilter,
) -> CmdResult<Page<Adjustment>> {
    state.run(|c, ctx| inventory::list_adjustments(c, ctx, &filter))
}

#[tauri::command]
pub async fn inventory_movements(state: State<'_, AppState>, filter: MovementFilter) -> CmdResult<Page<Movement>> {
    state.run(|c, ctx| inventory::list_movements(c, ctx, &filter))
}

#[tauri::command]
pub async fn inventory_batches(state: State<'_, AppState>, filter: BatchFilter) -> CmdResult<Vec<Batch>> {
    state.run(|c, ctx| inventory::list_batches(c, ctx, &filter))
}

#[tauri::command]
pub async fn inventory_verify(state: State<'_, AppState>) -> CmdResult<Vec<LedgerMismatch>> {
    state.run(|c, ctx| inventory::verify_ledger(c, ctx))
}
