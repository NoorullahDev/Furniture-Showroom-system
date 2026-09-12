use crate::application;
use crate::commands::authed;
use crate::commands::wrapper::run_command;
use crate::dto::AppErrorDto;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn seed_demo_data(
    state: State<'_, AppState>,
    session: String,
) -> Result<application::seed_demo::SeedResult, AppErrorDto> {
    run_command("seed_demo_data", async move {
        #[cfg(not(debug_assertions))]
        {
            return Err(crate::error::AppError::Validation(
                "seed_demo_data is not available in production builds".into(),
            ));
        }
        #[cfg(debug_assertions)]
        {
            let _ = authed(&state, &session, "settings.manage").await?;
            application::seed_demo::seed_demo_data(&state).await
        }
    })
    .await
}
