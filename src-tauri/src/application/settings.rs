use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::state::AppState;

/// Read one settings value as raw JSON text (Phase 1 example read).
pub async fn get(state: &AppState, key: &str) -> Result<Option<String>, AppError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value_json FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(&state.pool)
        .await?;
    Ok(row.map(|(value,)| value))
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
        .execute(&state.pool, move |tx| {
            let key_owned = key_owned.clone();
            let value_owned = value_owned.clone();
            let now = now.clone();
            let updated_by = updated_by;
            Box::pin(async move {
                sqlx::query(
                    "INSERT INTO settings (key, value_json, updated_by, updated_at)
                     VALUES (?, ?, ?, ?)
                     ON CONFLICT(key) DO UPDATE SET
                       value_json = excluded.value_json,
                       updated_by = excluded.updated_by,
                       updated_at = excluded.updated_at",
                )
                .bind(key_owned)
                .bind(value_owned)
                .bind(updated_by)
                .bind(now)
                .execute(&mut **tx)
                .await?;
                Ok(())
            })
        })
        .await
}
