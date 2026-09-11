use crate::application;
use crate::commands::wrapper::{run_command, run_command_with_correlation};
use crate::dto::{AppErrorDto, LoginResultDto, SessionProfileDto};
use crate::error::new_correlation_id;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn auth_login(
    state: State<'_, AppState>,
    username: String,
    password: String,
) -> Result<LoginResultDto, AppErrorDto> {
    // Explicit relation id so the login audit event matches the log entry.
    let correlation_id = new_correlation_id();
    run_command_with_correlation("auth_login", correlation_id.clone(), async move {
        application::auth::login(&state, &username, &password, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn auth_logout(state: State<'_, AppState>, session: String) -> Result<(), AppErrorDto> {
    run_command("auth_logout", async move {
        application::auth::logout(&state, &session).await
    })
    .await
}

#[tauri::command]
pub async fn auth_current(
    state: State<'_, AppState>,
    session: String,
) -> Result<Option<SessionProfileDto>, AppErrorDto> {
    run_command("auth_current", async move {
        application::auth::current(&state, &session).await
    })
    .await
}

#[tauri::command]
pub async fn auth_lock(state: State<'_, AppState>, session: String) -> Result<(), AppErrorDto> {
    run_command("auth_lock", async move {
        application::auth::lock_session(&state, &session).await
    })
    .await
}

#[tauri::command]
pub async fn auth_unlock(
    state: State<'_, AppState>,
    session: String,
    password: String,
) -> Result<SessionProfileDto, AppErrorDto> {
    run_command("auth_unlock", async move {
        application::auth::unlock_session(&state, &session, &password).await?;
        application::auth::current(&state, &session).await?.ok_or(
            crate::error::AppError::Unauthorized("session is not active".into()),
        )
    })
    .await
}

#[tauri::command]
pub async fn auth_change_password(
    state: State<'_, AppState>,
    session: String,
    current_password: String,
    new_password: String,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("auth_change_password", correlation_id.clone(), async move {
        let principal = application::auth::resolve_session(&state, &session).await?;
        application::auth::change_password(
            &state,
            &principal,
            &current_password,
            &new_password,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn auth_update_login_details(
    state: State<'_, AppState>,
    session: String,
    current_password: String,
    new_username: Option<String>,
    new_password: Option<String>,
    confirm_password: Option<String>,
) -> Result<String, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation(
        "auth_update_login_details",
        correlation_id.clone(),
        async move {
            let principal = application::auth::resolve_session(&state, &session).await?;
            application::auth::update_login_details(
                &state,
                &principal,
                &current_password,
                new_username.as_deref(),
                new_password.as_deref(),
                confirm_password.as_deref(),
                &correlation_id,
            )
            .await
        },
    )
    .await
}
