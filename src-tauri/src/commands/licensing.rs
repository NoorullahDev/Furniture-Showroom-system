use crate::application;
use crate::commands::wrapper::run_command;
use crate::dto::{AppErrorDto, LicenseStatusDto};
use crate::state::AppState;
use tauri::State;

/// Public status probe used by the logged-out screen. The response contains
/// only the verified state and never exposes a license payload or secret.
#[tauri::command]
pub async fn license_status(state: State<'_, AppState>) -> Result<LicenseStatusDto, AppErrorDto> {
    run_command("license_status", async move {
        application::licensing::status(&state)
    })
    .await
}

/// Public activation/renewal boundary. The signed token is verified and bound
/// to this machine before it replaces the current protected license.
#[tauri::command]
pub async fn license_activate(
    state: State<'_, AppState>,
    license_key: String,
) -> Result<LicenseStatusDto, AppErrorDto> {
    run_command("license_activate", async move {
        application::licensing::activate(&state, &license_key)
    })
    .await
}
