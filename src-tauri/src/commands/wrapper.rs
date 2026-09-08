use crate::dto::AppErrorDto;
use crate::error::{new_correlation_id, AppError};

/// Execute one Tauri command inside a correlated tracing span and map every
/// failure to a stable `AppErrorDto` carrying the same correlation id the
/// server logged under.
pub async fn run_command<T>(
    name: &str,
    fut: impl std::future::Future<Output = Result<T, AppError>>,
) -> Result<T, AppErrorDto> {
    let correlation_id = new_correlation_id();
    let span = tracing::info_span!("command", command = %name, correlation_id = %correlation_id);
    let _enter = span.enter();

    match fut.await {
        Ok(value) => {
            tracing::info!("command ok");
            Ok(value)
        }
        Err(e) => {
            tracing::error!(code = e.code(), message = %e, "command failed");
            Err(AppErrorDto::from_error(&e, &correlation_id))
        }
    }
}
