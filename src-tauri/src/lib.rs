pub mod application;
pub mod commands;
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
            let (pool, db_info) = tauri::async_runtime::block_on(
                infrastructure::db::open(&paths),
            )?;

            eprintln!(
                "[furniture-shop] database ready at {:?} (schema version {}, pending {})",
                paths.db_path, db_info.version, db_info.pending_migrations
            );

            app.manage(AppState { pool, paths });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::proof::proof_app_info,
            commands::proof::proof_generate_pdf,
            commands::proof::proof_import_image,
            commands::proof::proof_create_backup,
            commands::proof::proof_list_backups,
            commands::proof::proof_open_path,
        ])
        .on_window_event(|window, event| {
            use tauri::WindowEvent;
            if let WindowEvent::CloseRequested { .. } = event {
                let state = window.state::<AppState>();
                // Opportunity to checkpoint/backup on close (Phase 7).
                eprintln!("[furniture-shop] window close requested");
                let _ = state;
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}