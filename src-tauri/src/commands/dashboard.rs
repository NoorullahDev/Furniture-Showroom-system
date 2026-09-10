use tauri::State;

use crate::commands::authenticated;
use crate::commands::wrapper::run_command;
use crate::dto::dashboard::DashboardSummaryDto;
use crate::dto::AppErrorDto;
use crate::state::AppState;

/// Permission-filtered landing summary. Any authenticated user may open the
/// dashboard; each metric is dropped or zeroed according to their permissions.
#[tauri::command]
pub async fn dashboard_summary(
    state: State<'_, AppState>,
    session: String,
) -> Result<DashboardSummaryDto, AppErrorDto> {
    run_command("dashboard_summary", async move {
        let principal = authenticated(&state, &session).await?;
        crate::application::dashboard::dashboard_summary(&state, &principal).await
    })
    .await
}
