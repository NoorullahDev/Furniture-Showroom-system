use crate::application;
use crate::commands::wrapper::{run_command, run_command_with_correlation};
use crate::dto::{AppErrorDto, FirstRunStatusDto, LoginResultDto};
use crate::error::new_correlation_id;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn first_run_status(
    state: State<'_, AppState>,
) -> Result<FirstRunStatusDto, AppErrorDto> {
    run_command("first_run_status", async move {
        application::first_run::status(&state).await
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn first_run_complete(
    state: State<'_, AppState>,
    shop_name: String,
    shop_address: String,
    shop_phone: String,
    shop_email: String,
    currency: String,
    timezone: String,
    invoice_prefix: Option<String>,
    backup_location: Option<String>,
    owner_username: String,
    owner_full_name: String,
    owner_password: String,
) -> Result<LoginResultDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("first_run_complete", correlation_id.clone(), async move {
        application::first_run::complete(
            &state,
            &application::first_run::FirstRunInput {
                shop_name,
                shop_address,
                shop_phone,
                shop_email,
                currency,
                timezone,
                invoice_prefix,
                backup_location,
                owner_username,
                owner_full_name,
                owner_password,
            },
            &correlation_id,
        )
        .await
    })
    .await
}
