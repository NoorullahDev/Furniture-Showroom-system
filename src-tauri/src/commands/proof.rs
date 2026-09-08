use tauri::State;

use crate::application;
use crate::commands::wrapper::run_command;
use crate::dto::{AppInfoDto, BackupEntryDto, BackupResultDto, ImageImportResultDto, PdfResultDto};
use crate::state::AppState;

#[tauri::command]
pub async fn proof_app_info(
    state: State<'_, AppState>,
) -> Result<AppInfoDto, crate::dto::AppErrorDto> {
    run_command("proof_app_info", async move {
        application::proof::app_info(&state).await
    })
    .await
}

#[tauri::command]
pub async fn proof_generate_pdf(
    state: State<'_, AppState>,
) -> Result<PdfResultDto, crate::dto::AppErrorDto> {
    run_command("proof_generate_pdf", async move {
        application::proof::generate_pdf(&state).await
    })
    .await
}

#[tauri::command]
pub async fn proof_import_image(
    state: State<'_, AppState>,
    path: String,
) -> Result<ImageImportResultDto, crate::dto::AppErrorDto> {
    run_command("proof_import_image", async move {
        application::proof::import_image(&state, path).await
    })
    .await
}

#[tauri::command]
pub async fn proof_create_backup(
    state: State<'_, AppState>,
) -> Result<BackupResultDto, crate::dto::AppErrorDto> {
    run_command("proof_create_backup", async move {
        application::proof::create_backup(&state).await
    })
    .await
}

#[tauri::command]
pub async fn proof_list_backups(
    state: State<'_, AppState>,
) -> Result<Vec<BackupEntryDto>, crate::dto::AppErrorDto> {
    run_command("proof_list_backups", async move {
        application::proof::list_backups(&state).await
    })
    .await
}

#[tauri::command]
pub async fn proof_open_path(
    state: State<'_, AppState>,
    path: String,
) -> Result<(), crate::dto::AppErrorDto> {
    run_command("proof_open_path", async move {
        application::proof::open_path(&state, path).await
    })
    .await
}
