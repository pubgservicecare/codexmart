//! CODE X MART desktop shell.

mod backup_jobs;
mod commands;
mod printing;
mod state;

use tauri::{Manager, RunEvent, WindowEvent};

use crate::state::{AppPaths, AppState};

pub fn run() {
    let app = tauri::Builder::default()
        // Must be first: a second launch just focuses the running window, so
        // two copies can never write to the database at the same time.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                // Shop time, so log entries match receipts and reports.
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let paths = AppPaths::resolve(app.handle())?;
            log::info!("CODE X MART {} starting; data folder: {}", codexmart_core::APP_VERSION, paths.root.display());
            std::panic::set_hook(Box::new(|info| log::error!("Unexpected error: {info}")));
            app.manage(AppState::initialize(paths));
            backup_jobs::start_scheduler(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    backup_jobs::backup_on_exit(&window.state::<AppState>());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_status,
            commands::app::app_retry_open,
            commands::app::app_setup,
            commands::app::auth_login,
            commands::app::auth_logout,
            commands::app::auth_change_password,
            commands::app::auth_recover,
            commands::app::auth_regenerate_recovery_key,
            commands::app::users_list,
            commands::app::users_create,
            commands::app::users_update,
            commands::app::users_reset_password,
            commands::app::settings_get,
            commands::app::settings_save,
            commands::app::audit_list,
            commands::app::app_health_check,
            commands::app::app_open_folder,
            commands::app::export_text_file,
            commands::app::log_client_error,
            commands::catalog::lookups_list,
            commands::catalog::lookups_create,
            commands::catalog::lookups_update,
            commands::catalog::units_list,
            commands::catalog::units_create,
            commands::catalog::units_update,
            commands::catalog::products_list,
            commands::catalog::products_get,
            commands::catalog::products_search,
            commands::catalog::products_find_by_code,
            commands::catalog::products_create,
            commands::catalog::products_update,
            commands::catalog::products_set_active,
            commands::catalog::products_generate_barcode,
            commands::catalog::inventory_adjust,
            commands::catalog::inventory_adjustments,
            commands::catalog::inventory_movements,
            commands::catalog::inventory_batches,
            commands::catalog::inventory_verify,
            commands::sales::sales_quote,
            commands::sales::sales_checkout,
            commands::sales::sales_list,
            commands::sales::sales_get,
            commands::sales::sales_void,
            commands::sales::sales_returnable,
            commands::sales::sales_return,
            commands::sales::sales_returns_list,
            commands::sales::sales_return_get,
            commands::sales::customers_list,
            commands::sales::customers_get,
            commands::sales::customers_create,
            commands::sales::customers_update,
            commands::sales::customers_ledger,
            commands::sales::customers_receive_payment,
            commands::sales::customer_payments_list,
            commands::sales::cash_preview,
            commands::sales::cash_close,
            commands::sales::cash_closings,
            commands::sales::printers_list,
            commands::sales::print_receipt_direct,
            commands::sales::print_test_direct,
            commands::purchasing::suppliers_list,
            commands::purchasing::suppliers_get,
            commands::purchasing::suppliers_create,
            commands::purchasing::suppliers_update,
            commands::purchasing::suppliers_ledger,
            commands::purchasing::purchases_list,
            commands::purchasing::purchases_get,
            commands::purchasing::purchases_create,
            commands::purchasing::po_list,
            commands::purchasing::po_get,
            commands::purchasing::po_create,
            commands::purchasing::po_update,
            commands::purchasing::po_set_status,
            commands::purchasing::supplier_payments_list,
            commands::purchasing::supplier_payments_create,
            commands::purchasing::purchase_returns_list,
            commands::purchasing::purchase_returns_create,
            commands::purchasing::expenses_list,
            commands::purchasing::expenses_create,
            commands::purchasing::expenses_void,
            commands::reports::dashboard_get,
            commands::reports::reports_sales_summary,
            commands::reports::reports_profit_loss,
            commands::reports::reports_product_sales,
            commands::reports::reports_category_sales,
            commands::reports::reports_purchases,
            commands::reports::reports_expenses,
            commands::reports::reports_returns,
            commands::reports::reports_payment_methods,
            commands::reports::reports_supplier_balances,
            commands::reports::reports_customer_balances,
            commands::reports::reports_inventory_valuation,
            commands::reports::reports_low_stock,
            commands::reports::reports_expiring,
            commands::backup::backup_status,
            commands::backup::backup_create,
            commands::backup::backup_list_folder,
            commands::backup::backup_validate,
            commands::backup::backup_restore,
        ])
        .build(tauri::generate_context!())
        .expect("CODE X MART could not start");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            if let Some(state) = handle.try_state::<AppState>() {
                state.shutdown();
            }
            log::info!("CODE X MART closed");
        }
    });
}
