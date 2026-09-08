use crate::application;
use crate::commands::authed;
use crate::commands::wrapper::run_command;
use crate::dto::{AppErrorDto, RoleDto};
use crate::error::new_correlation_id;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn role_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<RoleDto>, AppErrorDto> {
    run_command("role_list", async move {
        let principal = authed(&state, &session, "user.manage").await?;
        application::roles::list_roles(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn role_permissions_set(
    state: State<'_, AppState>,
    session: String,
    role_id: i64,
    permissions: Vec<String>,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command("role_permissions_set", async move {
        let principal = authed(&state, &session, "role.manage").await?;
        application::roles::set_role_permissions(
            &state,
            &principal,
            role_id,
            permissions,
            &correlation_id,
        )
        .await
        .map(|_| ())
    })
    .await
}
