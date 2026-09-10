use tauri::State;

use crate::commands::authed;
use crate::commands::wrapper::run_command;
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
        let _principal = authed(&state, &session, "reports.view").await?;
        crate::application::reports::export_report(&state, &report_type, &filter, format).await
    })
    .await
}

#[tauri::command]
pub async fn open_file(path: String) -> Result<(), AppErrorDto> {
    run_command("open_file", async {
        opener::open(&path)
            .map_err(|e| crate::error::AppError::Io(std::io::Error::other(e.to_string())))
    })
    .await
}
