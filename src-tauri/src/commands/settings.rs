use crate::application;
use crate::commands::wrapper::run_command;
use crate::dto::AppErrorDto;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn settings_get(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, AppErrorDto> {
    run_command("settings_get", async move {
        application::settings::get(&state, &key).await
    })
    .await
}

#[tauri::command]
pub async fn settings_set(
    state: State<'_, AppState>,
    key: String,
    value: serde_json::Value,
) -> Result<(), AppErrorDto> {
    run_command("settings_set", async move {
        let json = value.to_string();
        application::settings::set(&state, &key, &json, None).await
    })
    .await
}
