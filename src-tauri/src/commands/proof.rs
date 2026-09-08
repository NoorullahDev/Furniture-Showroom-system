use tauri::State;

use crate::application;
use crate::dto::{
    AppErrorDto, AppInfoDto, BackupEntryDto, BackupResultDto, ImageImportResultDto, PdfResultDto,
};
use crate::error::AppError;
use crate::state::AppState;

fn err(e: AppError) -> AppErrorDto {
    AppErrorDto::from(e)
}

#[tauri::command]
pub async fn proof_app_info(state: State<'_, AppState>) -> Result<AppInfoDto, AppErrorDto> {
    application::proof::app_info(&state).await.map_err(err)
}

#[tauri::command]
pub async fn proof_generate_pdf(state: State<'_, AppState>) -> Result<PdfResultDto, AppErrorDto> {
    application::proof::generate_pdf(&state).await.map_err(err)
}

#[tauri::command]
pub async fn proof_import_image(
    state: State<'_, AppState>,
    path: String,
) -> Result<ImageImportResultDto, AppErrorDto> {
    application::proof::import_image(&state, path).await.map_err(err)
}

#[tauri::command]
pub async fn proof_create_backup(
    state: State<'_, AppState>,
) -> Result<BackupResultDto, AppErrorDto> {
    application::proof::create_backup(&state).await.map_err(err)
}

#[tauri::command]
pub async fn proof_list_backups(
    state: State<'_, AppState>,
) -> Result<Vec<BackupEntryDto>, AppErrorDto> {
    application::proof::list_backups(&state).await.map_err(err)
}

#[tauri::command]
pub async fn proof_open_path(state: State<'_, AppState>, path: String) -> Result<(), AppErrorDto> {
    application::proof::open_path(&state, path).await.map_err(err)
}