use crate::dto::AppErrorDto;
use crate::error::{new_correlation_id, AppError};

/// Execute one Tauri command inside a correlated tracing span and map every
/// failure to a stable `AppErrorDto` carrying the same correlation id the
/// server logged under.
pub async fn run_command<T>(
    name: &str,
    fut: impl std::future::Future<Output = Result<T, AppError>>,
) -> Result<T, AppErrorDto> {
    run_command_with_correlation(name, new_correlation_id(), fut).await
}

/// Variant that lets the caller reuse one correlation id for both the log span
/// and any audit events written by the command (e.g. `auth_login`).
pub async fn run_command_with_correlation<T>(
    name: &str,
    correlation_id: String,
    fut: impl std::future::Future<Output = Result<T, AppError>>,
) -> Result<T, AppErrorDto> {
    if let Err(error) = crate::application::licensing::ensure_command_allowed(name) {
        return Err(error.to_dto(&correlation_id));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn normal_commands_fail_closed_before_license_runtime_initialization() {
        let result = run_command("auth_login", async { Ok::<_, AppError>(()) }).await;
        let error = result.expect_err("normal command must be blocked");
        assert_eq!(error.code, "LICENSE_REQUIRED");

        let public_probe = run_command("license_status", async { Ok::<_, AppError>(()) }).await;
        assert!(public_probe.is_ok());
    }
}
