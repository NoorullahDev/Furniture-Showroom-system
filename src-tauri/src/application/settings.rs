use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::repositories::SettingsRepository;
use crate::state::AppState;

/// Read one settings value as raw JSON text (Phase 1 example read service).
pub async fn get(state: &AppState, key: &str) -> Result<Option<String>, AppError> {
    SettingsRepository::find(&state.pool, key).await
}

/// Upsert one settings key inside a single serialized write transaction
/// (Phase 1 example of a transactional service through the write coordinator).
pub async fn set(
    state: &AppState,
    key: &str,
    value_json: &str,
    updated_by: Option<i64>,
) -> Result<(), AppError> {
    let key_owned = key.to_owned();
    let value_owned = value_json.to_owned();
    let now = state.clock.now_iso();

    state
        .write_coordinator
        .execute(&state.pool, |tx| {
            Box::pin(async move {
                SettingsRepository::upsert(tx, &key_owned, &value_owned, updated_by, &now).await?;
                Ok(())
            })
        })
        .await
}
