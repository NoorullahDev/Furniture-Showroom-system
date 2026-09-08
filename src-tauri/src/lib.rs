pub mod application;
pub mod commands;
pub mod domain;
pub mod dto;
pub mod error;
pub mod infrastructure;
pub mod state;

use tauri::Manager;

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
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::proof::proof_app_info,
            commands::proof::proof_generate_pdf,
            commands::proof::proof_import_image,
            commands::proof::proof_create_backup,
            commands::proof::proof_list_backups,
            commands::proof::proof_open_path,
            commands::settings::settings_get,
            commands::settings::settings_set,
        ])
        .on_window_event(|window, event| {
            use tauri::WindowEvent;
            if let WindowEvent::CloseRequested { .. } = event {
                let state = window.state::<AppState>();
                // Opportunity to checkpoint/backup on close (Phase 7).
                tracing::debug!("window close requested");
                let _ = state;
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
