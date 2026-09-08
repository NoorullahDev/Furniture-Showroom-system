use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteConnection;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::infrastructure::redact;

type ChainRow = (
    i64,
    Option<i64>,
    String,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
);

#[derive(Debug, Clone, Default)]
pub struct AuditInput {
    pub user_id: Option<i64>,
    pub session_id: Option<String>,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub reason: Option<String>,
    /// Sensitive values must already be redacted by the caller; the service
    /// re-applies `redact` as a final safety pass before storage.
    pub before_json: Option<String>,
    pub after_json: Option<String>,
    pub approval_user_id: Option<i64>,
    pub correlation_id: Option<String>,
}

#[derive(Clone)]
pub struct AuditService {
    pool: SqlitePool,
    clock: std::sync::Arc<dyn Clock>,
    app_version: String,
}

impl AuditService {
    pub fn new(pool: SqlitePool, clock: std::sync::Arc<dyn Clock>, app_version: String) -> Self {
        Self {
            pool,
            clock,
            app_version,
        }
    }

    /// Persist one event on the caller's connection so it commits atomically
    /// with the operation it documents.
    pub async fn record(
        &self,
        conn: &mut SqliteConnection,
        input: AuditInput,
    ) -> Result<(), AppError> {
        let prev_hash = self.prev_hash(conn).await?;
        let now = self.clock.now_iso();

        sqlx::query(
            "INSERT INTO audit_logs
               (user_id, action, entity_type, entity_id, reason, before_json,
                after_json, approval_user_id, session_id, app_version,
                correlation_id, prev_hash, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(input.user_id)
        .bind(input.action)
        .bind(input.entity_type)
        .bind(input.entity_id)
        .bind(input.reason)
        .bind(input.before_json.map(|v| redact(&v)))
        .bind(input.after_json.map(|v| redact(&v)))
        .bind(input.approval_user_id)
        .bind(input.session_id)
        .bind(&self.app_version)
        .bind(input.correlation_id)
        .bind(prev_hash)
        .bind(&now)
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Convenience for events that are themselves the transaction (e.g.
    /// failed login) where no surrounding mutation exists.
    pub async fn record_pool(&self, input: AuditInput) -> Result<(), AppError> {
        let mut conn = self.pool.acquire().await?;
        self.record(&mut conn, input).await
    }

    async fn prev_hash(&self, conn: &mut SqliteConnection) -> Result<Option<String>, AppError> {
        // Because every audit insert funnels through the write coordinator, ids
        // are strictly increasing and the immediate predecessor is the newest row.
        let row: Option<ChainRow> = sqlx::query_as(
            "SELECT id, user_id, action, entity_type, entity_id, created_at, prev_hash
                 FROM audit_logs ORDER BY id DESC LIMIT 1",
        )
        .fetch_optional(conn)
        .await?;

        Ok(row.map(
            |(id, user_id, action, entity_type, entity_id, created_at, prev)| {
                chain_hash(
                    id,
                    user_id,
                    &action,
                    entity_type.as_deref(),
                    entity_id.as_deref(),
                    &created_at,
                    prev.as_deref(),
                )
            },
        ))
    }

    /// Recompute the hash chain and report the id of the first broken link, if any.
    pub async fn verify_chain(&self) -> Result<Option<i64>, AppError> {
        let rows: Vec<ChainRow> = sqlx::query_as(
            "SELECT id, user_id, action, entity_type, entity_id, created_at, prev_hash
                 FROM audit_logs ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        for pair in rows.windows(2) {
            let head = &pair[0];
            let tail = &pair[1];
            let want = chain_hash(
                head.0,
                head.1,
                &head.2,
                head.3.as_deref(),
                head.4.as_deref(),
                &head.5,
                head.6.as_deref(),
            );
            if tail.6.as_deref() != Some(want.as_str()) {
                return Ok(Some(tail.0));
            }
        }
        Ok(None)
    }
}

fn canonical(
    id: i64,
    user_id: Option<i64>,
    action: &str,
    entity_type: Option<&str>,
    entity_id: Option<&str>,
    created_at: &str,
    prev_hash: Option<&str>,
) -> String {
    format!(
        "{id}|{}|{action}|{}|{}|{created_at}|{}",
        user_id.map(|v| v.to_string()).unwrap_or_default(),
        entity_type.unwrap_or_default(),
        entity_id.unwrap_or_default(),
        prev_hash.unwrap_or_default(),
    )
}

fn chain_hash(
    id: i64,
    user_id: Option<i64>,
    action: &str,
    entity_type: Option<&str>,
    entity_id: Option<&str>,
    created_at: &str,
    prev_hash: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        canonical(
            id,
            user_id,
            action,
            entity_type,
            entity_id,
            created_at,
            prev_hash,
        )
        .as_bytes(),
    );
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_hashes_are_stable_hex() {
        let h = chain_hash(
            1,
            Some(2),
            "user.login",
            None,
            None,
            "2026-09-08T00:00:00Z",
            None,
        );
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
