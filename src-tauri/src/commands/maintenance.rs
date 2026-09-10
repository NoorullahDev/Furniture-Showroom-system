use tauri::State;

use crate::commands::authed;
use crate::commands::wrapper::run_command;
use crate::dto::maintenance::{
    BackupListItemDto, IntegrityResultDto, MaintenanceStatusDto, RestoreResultDto,
};
use crate::dto::AppErrorDto;
use crate::dto::BackupResultDto;
use crate::error::new_correlation_id;
use crate::state::AppState;

#[tauri::command]
pub async fn maintenance_status(
    state: State<'_, AppState>,
    session: String,
) -> Result<MaintenanceStatusDto, AppErrorDto> {
    run_command("maintenance_status", async move {
        let principal = authed(&state, &session, "backup.create").await?;
        crate::application::maintenance::status(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn backup_create(
    state: State<'_, AppState>,
    session: String,
) -> Result<BackupResultDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("backup_create", async move {
        let principal = authed(&state, &session, "backup.create").await?;
        crate::application::maintenance::create_backup(&state, &principal, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn backup_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<BackupListItemDto>, AppErrorDto> {
    run_command("backup_list", async move {
        let principal = authed(&state, &session, "backup.create").await?;
        crate::application::maintenance::list_backups(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn backup_delete(
    state: State<'_, AppState>,
    session: String,
    name: String,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("backup_delete", async move {
        let principal = authed(&state, &session, "backup.restore").await?;
        crate::application::maintenance::delete_backup(&state, &principal, &name, &correlation_id)
            .await
    })
    .await
}

#[tauri::command]
pub async fn backup_restore(
    state: State<'_, AppState>,
    session: String,
    name: String,
) -> Result<RestoreResultDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("backup_restore", async move {
        let principal = authed(&state, &session, "backup.restore").await?;
        crate::application::maintenance::restore_backup(&state, &principal, &name, &correlation_id)
            .await
    })
    .await
}

#[tauri::command]
pub async fn maintenance_integrity(
    state: State<'_, AppState>,
    session: String,
) -> Result<IntegrityResultDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("maintenance_integrity", async move {
        let principal = authed(&state, &session, "backup.create").await?;
        crate::application::maintenance::run_integrity_check(&state, &principal, &correlation_id)
            .await
    })
    .await
}
