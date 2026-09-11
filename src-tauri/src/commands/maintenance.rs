use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::authed;
use crate::commands::wrapper::run_command;
use crate::dto::maintenance::{
    BackupInspectionDto, BackupListItemDto, BackupPreferencesDto, IntegrityResultDto,
    MaintenanceStatusDto, RestoreResultDto,
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
    backup_name: String,
) -> Result<BackupResultDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("backup_create", async move {
        let principal = authed(&state, &session, "backup.create").await?;
        crate::application::backup_workflow::create_manual(
            &state,
            &principal,
            &backup_name,
            &correlation_id,
        )
        .await
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
        crate::application::backup_workflow::list(&state, &principal).await
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
        crate::application::backup_workflow::delete(&state, &principal, &name, &correlation_id)
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
        crate::application::backup_workflow::schedule_restore_from_list(
            &state,
            &principal,
            &name,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn backup_preferences_get(
    state: State<'_, AppState>,
    session: String,
) -> Result<BackupPreferencesDto, AppErrorDto> {
    run_command("backup_preferences_get", async move {
        let principal = authed(&state, &session, "backup.create").await?;
        crate::application::backup_workflow::get_preferences(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn backup_preferences_save(
    state: State<'_, AppState>,
    session: String,
    directory: String,
    auto_backup_on_close: bool,
) -> Result<BackupPreferencesDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("backup_preferences_save", async move {
        let principal = authed(&state, &session, "backup.create").await?;
        crate::application::backup_workflow::save_preferences(
            &state,
            &principal,
            directory,
            auto_backup_on_close,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn backup_inspect(
    state: State<'_, AppState>,
    session: String,
    path: String,
) -> Result<BackupInspectionDto, AppErrorDto> {
    run_command("backup_inspect", async move {
        let principal = authed(&state, &session, "backup.restore").await?;
        crate::application::backup_workflow::inspect(&state, &principal, &path).await
    })
    .await
}

#[tauri::command]
pub async fn backup_restore_import(
    state: State<'_, AppState>,
    session: String,
    path: String,
) -> Result<RestoreResultDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("backup_restore_import", async move {
        let principal = authed(&state, &session, "backup.restore").await?;
        crate::application::backup_workflow::schedule_restore(
            &state,
            &principal,
            &path,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn backup_restart(
    app: AppHandle,
    state: State<'_, AppState>,
    session: String,
) -> Result<(), AppErrorDto> {
    run_command("backup_restart", async move {
        let _ = authed(&state, &session, "backup.restore").await?;
        app.restart();
        #[allow(unreachable_code)]
        Ok(())
    })
    .await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CloseBackupError {
    message: String,
}

#[tauri::command]
pub async fn backup_close_retry(app: AppHandle) -> Result<(), AppErrorDto> {
    run_command("backup_close_retry", async move {
        let state = app.state::<AppState>();
        if state
            .backup_close_state
            .compare_exchange(2, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(crate::error::AppError::Conflict(
                "a close backup is already running".into(),
            ));
        }
        let _ = app.emit("backup-close-progress", ());
        match crate::application::backup_workflow::create_automatic(&state).await {
            Ok(_) => {
                let _ = app.emit("backup-close-complete", ());
                app.exit(0);
                Ok(())
            }
            Err(error) => {
                state.backup_close_state.store(2, Ordering::SeqCst);
                let _ = app.emit(
                    "backup-close-failed",
                    CloseBackupError {
                        message: error.to_string(),
                    },
                );
                Err(error)
            }
        }
    })
    .await
}

#[tauri::command]
pub fn backup_close_cancel(state: State<'_, AppState>) {
    state.backup_close_state.store(0, Ordering::SeqCst);
}

#[tauri::command]
pub fn backup_close_without(app: AppHandle, state: State<'_, AppState>) {
    state.backup_close_state.store(0, Ordering::SeqCst);
    app.exit(0);
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
