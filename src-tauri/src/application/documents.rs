use sqlx::sqlite::SqliteConnection;

use crate::error::AppError;

/// Allowed table names for `replay_guard` to prevent SQL injection via table
/// interpolation. Every call site must use one of these values.
const ALLOWED_TABLES: &[&str] = &[
    "sales",
    "purchases",
    "supplier_returns",
    "sales_returns",
    "credit_notes",
    "damage_records",
];

/// Consume one number from a `document_sequences` row. The UPDATE happens inside
/// the caller's transaction, so a failed posting never burns a document number.
pub(crate) async fn next_document_number(
    tx: &mut SqliteConnection,
    document_type: &str,
) -> Result<String, AppError> {
    let row: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT next_value, prefix, suffix FROM document_sequences WHERE document_type = ?",
    )
    .bind(document_type)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((value, prefix, suffix)) = row else {
        return Err(AppError::Internal(format!(
            "no document sequence configured for '{document_type}'"
        )));
    };
    let number = format!("{prefix}{value:06}{suffix}");
    sqlx::query("UPDATE document_sequences SET next_value = ? WHERE document_type = ?")
        .bind(value + 1)
        .bind(document_type)
        .execute(&mut *tx)
        .await?;
    Ok(number)
}

/// Replay guard for idempotent posting commands: a key already bound to this same
/// document short-circuits to the existing record; a key bound to a different
/// document is a conflict.
pub(crate) async fn replay_guard(
    tx: &mut SqliteConnection,
    table: &str,
    document_id: i64,
    idempotency_key: &Option<String>,
) -> Result<bool, AppError> {
    if !ALLOWED_TABLES.contains(&table) {
        return Err(AppError::Internal(format!(
            "replay_guard: invalid table name '{table}'"
        )));
    }
    let Some(key) = idempotency_key.as_deref().filter(|k| !k.trim().is_empty()) else {
        return Ok(false);
    };
    let existing: Option<i64> =
        sqlx::query_scalar(&format!("SELECT id FROM {table} WHERE idempotency_key = ?"))
            .bind(key)
            .fetch_optional(&mut *tx)
            .await?;
    match existing {
        Some(id) if id == document_id => Ok(true),
        Some(_) => Err(AppError::Conflict(format!(
            "idempotency key '{key}' was already used for a different {table}"
        ))),
        None => Ok(false),
    }
}
