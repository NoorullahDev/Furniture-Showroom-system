use crate::application;
use crate::commands::authed;
use crate::commands::wrapper::run_command;
use crate::dto::{AppErrorDto, UserDto};
use crate::error::new_correlation_id;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn user_create(
    state: State<'_, AppState>,
    session: String,
    username: String,
    full_name: String,
    password: String,
    roles: Vec<String>,
) -> Result<UserDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("user_create", async move {
        let principal = authed(&state, &session, "user.create").await?;
        application::users::create_user(
            &state,
            &principal,
            &username,
            &full_name,
            &password,
            &roles,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn user_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<UserDto>, AppErrorDto> {
    run_command("user_list", async move {
        let principal = authed(&state, &session, "user.manage").await?;
        application::users::list_users(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn user_update(
    state: State<'_, AppState>,
    session: String,
    user_id: i64,
    full_name: Option<String>,
    is_active: Option<bool>,
    roles: Option<Vec<String>>,
    new_password: Option<String>,
) -> Result<UserDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("user_update", async move {
        let principal = authed(&state, &session, "user.edit").await?;
        application::users::update_user(
            &state,
            &principal,
            user_id,
            full_name,
            is_active,
            roles,
            new_password,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn user_deactivate(
    state: State<'_, AppState>,
    session: String,
    user_id: i64,
    reason: String,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("user_deactivate", async move {
        let principal = authed(&state, &session, "user.deactivate").await?;
        application::users::deactivate_user(&state, &principal, user_id, &reason, &correlation_id)
            .await
    })
    .await
}

#[tauri::command]
pub async fn user_reset_password(
    state: State<'_, AppState>,
    session: String,
    user_id: i64,
    new_password: String,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("user_reset_password", async move {
        let principal = authed(&state, &session, "user.reset_password").await?;
        application::users::reset_password(
            &state,
            &principal,
            user_id,
            &new_password,
            &correlation_id,
        )
        .await
    })
    .await
}
