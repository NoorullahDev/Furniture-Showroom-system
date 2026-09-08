use crate::application;
use crate::commands::authed;
use crate::commands::wrapper::run_command;
use crate::dto::AppErrorDto;
use crate::error::new_correlation_id;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn settings_get(
    state: State<'_, AppState>,
    session: String,
    key: String,
) -> Result<Option<String>, AppErrorDto> {
    run_command("settings_get", async move {
        // Reading shop settings is safe for any authenticated user; the shell
        // needs them for every role (currency, shop name, issue policy).
        let _ = crate::commands::authenticated(&state, &session).await?;
        application::settings::get(&state, &key).await
    })
    .await
}

/// Authorized settings write: permission + audit, never accepts secrets.
#[tauri::command]
pub async fn settings_set(
    state: State<'_, AppState>,
    session: String,
    key: String,
    value: serde_json::Value,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("settings_set", async move {
        let principal = authed(&state, &session, "settings.manage").await?;
        let json = value.to_string();
        application::settings::set_authorized(&state, &principal, &key, &json, &correlation_id)
            .await
    })
    .await
}
