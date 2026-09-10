use tauri::State;

use crate::commands::authenticated;
use crate::commands::wrapper::run_command;
use crate::dto::search::SearchResultsDto;
use crate::dto::AppErrorDto;
use crate::state::AppState;

/// Global search across products, customers, suppliers and business documents.
/// Results are ranked (exact first) and every kind is permission-restricted.
#[tauri::command]
pub async fn global_search(
    state: State<'_, AppState>,
    session: String,
    query: String,
) -> Result<SearchResultsDto, AppErrorDto> {
    run_command("global_search", async move {
        let principal = authenticated(&state, &session).await?;
        crate::application::search::global_search(&state, &principal, &query).await
    })
    .await
}
