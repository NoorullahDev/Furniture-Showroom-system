use crate::application;
use crate::commands::authed;
use crate::commands::wrapper::run_command;
use crate::dto::{AppErrorDto, PrinterDto};
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

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn settings_update_general(
    state: State<'_, AppState>,
    session: String,
    shop_name: String,
    owner_name: String,
    address: String,
    phone: String,
    currency: String,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("settings_update_general", async move {
        let principal = authed(&state, &session, "settings.manage").await?;
        application::settings::update_general(
            &state,
            &principal,
            application::settings::GeneralSettingsInput {
                shop_name,
                owner_name,
                address,
                phone,
                currency,
            },
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn settings_update_print(
    state: State<'_, AppState>,
    session: String,
    paper_size: String,
    orientation: String,
    margin_mm: i64,
    font_size: String,
    copies: i64,
    show_logo: bool,
    show_address: bool,
    show_phone: bool,
    show_payment_details: bool,
    footer_text: String,
    printer_destination: String,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("settings_update_print", async move {
        let principal = authed(&state, &session, "settings.manage").await?;
        application::settings::update_print(
            &state,
            &principal,
            application::settings::PrintSettingsInput {
                paper_size,
                orientation,
                margin_mm,
                font_size,
                copies,
                show_logo,
                show_address,
                show_phone,
                show_payment_details,
                footer_text,
                printer_destination,
            },
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn shop_logo_get(
    state: State<'_, AppState>,
    session: String,
) -> Result<Option<String>, AppErrorDto> {
    run_command("shop_logo_get", async move {
        let _ = crate::commands::authenticated(&state, &session).await?;
        application::settings::logo_data(&state).await
    })
    .await
}

#[tauri::command]
pub async fn shop_logo_replace(
    state: State<'_, AppState>,
    session: String,
    path: String,
) -> Result<String, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("shop_logo_replace", async move {
        let principal = authed(&state, &session, "settings.manage").await?;
        application::settings::replace_logo(&state, &principal, &path, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn shop_logo_remove(
    state: State<'_, AppState>,
    session: String,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("shop_logo_remove", async move {
        let principal = authed(&state, &session, "settings.manage").await?;
        application::settings::remove_logo(&state, &principal, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn printer_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<PrinterDto>, AppErrorDto> {
    run_command("printer_list", async move {
        let _ = crate::commands::authenticated(&state, &session).await?;
        crate::infrastructure::printers::list()
    })
    .await
}
