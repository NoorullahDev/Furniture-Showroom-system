use tauri::State;

use crate::commands::wrapper::run_command;
use crate::commands::{authed, authenticated};
use crate::dto::reports::{ExportFormat, ReportExportResult, ReportFilterInput};
use crate::dto::AppErrorDto;
use crate::state::AppState;

#[tauri::command]
pub async fn report_export(
    state: State<'_, AppState>,
    session: String,
    report_type: String,
    filter: ReportFilterInput,
    format: ExportFormat,
) -> Result<ReportExportResult, AppErrorDto> {
    run_command("report_export", async move {
        let _principal = authed(&state, &session, "report.export").await?;
        crate::application::reports::export_report(&state, &report_type, &filter, format).await
    })
    .await
}

#[tauri::command]
pub async fn open_file(
    state: State<'_, AppState>,
    session: String,
    path: String,
) -> Result<(), AppErrorDto> {
    run_command("open_file", async move {
        // Any active session may open a file that already lives inside the
        // approved reports directory (reports, invoices, receipts, delivery
        // notes). The containment check resolves symlinks and rejects escapes.
        let _principal = authenticated(&state, &session).await?;
        let target = std::path::PathBuf::from(&path);
        state
            .paths
            .ensure_member(&state.paths.reports_dir, &target)?;
        opener::open(&target)
            .map_err(|e| crate::error::AppError::Io(std::io::Error::other(e.to_string())))
    })
    .await
}
