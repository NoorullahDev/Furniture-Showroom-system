pub mod application;
pub mod commands;
pub mod domain;
pub mod dto;
pub mod error;
pub mod infrastructure;
pub mod repositories;
pub mod state;

use std::sync::atomic::Ordering;

use tauri::{Emitter, Manager};

use crate::state::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("failed to resolve app-data directory: {e}"))?;
            std::fs::create_dir_all(&data_dir)?;

            let paths = infrastructure::FilePaths::init(&data_dir)?;

            let verbose = std::env::var_os("FURNITURE_SHOP_LOG_VERBOSE").is_some();
            infrastructure::install_logging(&paths.logs_dir, verbose)
                .map_err(|e| format!("failed to initialize logging: {e}"))?;

            let restored = infrastructure::restore::perform_pending_restore(&paths)?;
            if let Some(ref name) = restored {
                tracing::info!(backup = %name, "pending restore applied");
            }

            let (pool, db_info) = tauri::async_runtime::block_on(infrastructure::db::open(&paths))?;
            tracing::info!(
                db_path = %paths.db_path.display(),
                schema_version = db_info.version,
                pending_migrations = db_info.pending_migrations,
                "database ready"
            );

            let integrity =
                tauri::async_runtime::block_on(infrastructure::db::integrity_check(&pool))?;
            if !integrity.page_integrity_ok || integrity.foreign_key_violations != 0 {
                return Err(format!(
                    "database integrity check failed: page_ok={}, fk_violations={}",
                    integrity.page_integrity_ok, integrity.foreign_key_violations
                )
                .into());
            }
            tracing::debug!("startup integrity check passed");

            let state = AppState::new(pool, paths);
            let administrator_created = tauri::async_runtime::block_on(
                application::first_run::ensure_initial_administrator(&state),
            )?;
            if administrator_created {
                tracing::info!(username = "admin", "initial administrator created");
            }
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::settings::settings_update_general,
            commands::settings::settings_update_print,
            commands::settings::shop_logo_get,
            commands::settings::shop_logo_replace,
            commands::settings::shop_logo_remove,
            commands::settings::printer_list,
            commands::first_run::first_run_status,
            commands::first_run::first_run_complete,
            commands::licensing::license_status,
            commands::auth::auth_login,
            commands::auth::auth_logout,
            commands::auth::auth_current,
            commands::auth::auth_lock,
            commands::auth::auth_unlock,
            commands::auth::auth_change_password,
            commands::auth::auth_update_login_details,
            commands::users::user_create,
            commands::users::user_list,
            commands::users::user_update,
            commands::users::user_deactivate,
            commands::users::user_reset_password,
            commands::roles::role_list,
            commands::roles::role_permissions_set,
            commands::audit::audit_query,
            commands::catalogue::category_list,
            commands::catalogue::category_create,
            commands::catalogue::category_update,
            commands::catalogue::category_archive,
            commands::catalogue::product_type_list,
            commands::catalogue::product_type_create,
            commands::catalogue::product_type_update,
            commands::catalogue::product_type_archive,
            commands::catalogue::unit_list,
            commands::products::product_create,
            commands::products::product_update,
            commands::products::product_archive,
            commands::products::product_unarchive,
            commands::products::product_duplicate,
            commands::products::product_get,
            commands::products::product_list,
            commands::products::product_image_data,
            commands::products::product_image_add,
            commands::products::product_image_remove,
            commands::products::product_image_set_primary,
            commands::products::product_image_reorder,
            commands::inventory::location_list,
            commands::inventory::stock_balance_list,
            commands::inventory::stock_movement_list,
            commands::inventory::stock_valuation,
            commands::inventory::stock_opening,
            commands::inventory::stock_transfer,
            commands::inventory::stock_adjust,
            commands::inventory::stock_damage,
            commands::inventory::stock_repair,
            commands::inventory::stock_reserve,
            commands::inventory::stock_release,
            commands::inventory::stock_reverse,
            commands::inventory::stock_low_list,
            commands::inventory::stock_count_start,
            commands::inventory::stock_count_line_update,
            commands::inventory::stock_count_lines,
            commands::inventory::stock_count_post,
            commands::inventory::stock_count_list,
            commands::inventory::stock_opening_batch,
            commands::suppliers::supplier_create,
            commands::suppliers::supplier_update,
            commands::suppliers::supplier_list,
            commands::suppliers::supplier_get,
            commands::suppliers::supplier_ledger,
            commands::suppliers::payment_method_list,
            commands::suppliers::cash_account_list,
            commands::suppliers::cash_account_create,
            commands::suppliers::cash_entry_list,
            commands::purchases::purchase_create,
            commands::purchases::purchase_post,
            commands::purchases::purchase_list,
            commands::purchases::purchase_get,
            commands::purchases::payable_aging,
            commands::purchases::supplier_payment_create,
            commands::purchases::supplier_payment_void,
            commands::purchases::supplier_payment_list,
            commands::purchases::supplier_return_create,
            commands::purchases::supplier_return_post,
            commands::purchases::supplier_return_list,
            commands::sales::customer_create,
            commands::sales::customer_update,
            commands::sales::customer_list,
            commands::sales::customer_get,
            commands::sales::customer_ledger,
            commands::sales::customer_receipt_create,
            commands::sales::customer_receipt_void,
            commands::sales::customer_receipt_list,
            commands::sales::bundle_create,
            commands::sales::bundle_update,
            commands::sales::bundle_list,
            commands::sales::bundle_get,
            commands::sales::bundle_availability,
            commands::sales::sale_create,
            commands::sales::sale_confirm,
            commands::sales::sale_cancel,
            commands::sales::sale_list,
            commands::sales::sale_get,
            commands::sales::sale_invoice_pdf,
            commands::sales::customer_statement,
            commands::sales::customer_receipt_preview,
            commands::sales::customer_receipt_pdf,
            commands::sales::receivables,
            commands::fulfilment::delivery_create,
            commands::fulfilment::delivery_transition,
            commands::fulfilment::delivery_reschedule,
            commands::fulfilment::delivery_list,
            commands::fulfilment::delivery_get,
            commands::fulfilment::delivery_note_pdf,
            commands::fulfilment::sale_return_post,
            commands::fulfilment::sale_return_void,
            commands::fulfilment::sale_return_list,
            commands::fulfilment::sale_return_get,
            commands::fulfilment::credit_note_list,
            commands::fulfilment::credit_note_pdf,
            commands::fulfilment::damage_record,
            commands::fulfilment::damage_decide,
            commands::fulfilment::damage_list,
            commands::fulfilment::damage_get,
            commands::expenses::expense_category_list,
            commands::expenses::expense_category_create,
            commands::expenses::expense_category_update,
            commands::expenses::expense_list,
            commands::expenses::expense_page,
            commands::expenses::expense_post,
            commands::expenses::expense_reverse,
            commands::expenses::owner_transaction_post,
            commands::expenses::owner_transaction_list,
            commands::expenses::profit_summary,
            commands::dashboard::dashboard_summary,
            commands::search::global_search,
            commands::reports::report_export,
            commands::reports::open_file,
            commands::maintenance::maintenance_status,
            commands::maintenance::backup_create,
            commands::maintenance::backup_list,
            commands::maintenance::backup_delete,
            commands::maintenance::backup_restore,
            commands::maintenance::backup_preferences_get,
            commands::maintenance::backup_preferences_save,
            commands::maintenance::backup_inspect,
            commands::maintenance::backup_restore_import,
            commands::maintenance::backup_restart,
            commands::maintenance::backup_close_retry,
            commands::maintenance::backup_close_cancel,
            commands::maintenance::backup_close_without,
            commands::maintenance::maintenance_integrity,
            commands::seed_demo::seed_demo_data,
        ])
        .on_window_event(|window, event| {
            use tauri::WindowEvent;
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                let auto_enabled = match infrastructure::backup_preferences::load(&state.paths.data_dir) {
                    Ok(settings) => settings.auto_backup_on_close,
                    Err(error) => {
                        api.prevent_close();
                        state.backup_close_state.store(2, Ordering::SeqCst);
                        let _ = window.emit(
                            "backup-close-failed",
                            serde_json::json!({ "message": error.to_string() }),
                        );
                        return;
                    }
                };
                if !auto_enabled { return; }
                api.prevent_close();
                if state.backup_close_state.compare_exchange(
                    0, 1, Ordering::SeqCst, Ordering::SeqCst,
                ).is_err() {
                    return;
                }
                let app = window.app_handle().clone();
                let _ = app.emit("backup-close-progress", ());
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    match application::backup_workflow::create_automatic(&state).await {
                        Ok(result) => {
                            tracing::info!(backup = %result.name, "automatic close backup completed");
                            let _ = app.emit("backup-close-complete", ());
                            app.exit(0);
                        }
                        Err(error) => {
                            tracing::error!(error = %error, "automatic close backup failed");
                            state.backup_close_state.store(2, Ordering::SeqCst);
                            let _ = app.emit(
                                "backup-close-failed",
                                serde_json::json!({ "message": error.to_string() }),
                            );
                        }
                    }
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
