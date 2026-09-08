use sqlx::sqlite::SqlitePool;
use sqlx::SqliteConnection;

use crate::error::AppError;

/// Thin data-access layer for the `settings` table. Application services own
/// transactions; repositories only execute SQL.
pub struct SettingsRepository;

impl SettingsRepository {
    pub async fn find(pool: &SqlitePool, key: &str) -> Result<Option<String>, AppError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value_json FROM settings WHERE key = ?")
                .bind(key)
                .fetch_optional(pool)
                .await?;
        Ok(row.map(|(value,)| value))
    }

    /// Runs on the caller's connection so it can participate in a write
    /// transaction owned by the application service.
    pub async fn upsert(
        conn: &mut SqliteConnection,
        key: &str,
        value_json: &str,
        updated_by: Option<i64>,
        updated_at: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO settings (key, value_json, updated_by, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET
               value_json = excluded.value_json,
               updated_by = excluded.updated_by,
               updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value_json)
        .bind(updated_by)
        .bind(updated_at)
        .execute(conn)
        .await?;
        Ok(())
    }
}
