use crate::application;
use crate::commands::authed;
use crate::commands::wrapper::run_command;
use crate::dto::{AppErrorDto, AuditPageDto};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn audit_query(
    state: State<'_, AppState>,
    session: String,
    action: Option<String>,
    entity_type: Option<String>,
    entity_id: Option<String>,
    user_id: Option<i64>,
    from: Option<String>,
    to: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<AuditPageDto, AppErrorDto> {
    run_command("audit_query", async move {
        let principal = authed(&state, &session, "audit.view").await?;
        application::audit::query_audit(
            &state,
            &principal,
            &application::audit::AuditFilter {
                action,
                entity_type,
                entity_id,
                user_id,
                from,
                to,
                limit,
                offset,
            },
        )
        .await
    })
    .await
}
