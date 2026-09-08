use sqlx::SqlitePool;

use crate::domain::Session;
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::infrastructure::id::IdGenerator;

#[derive(Clone)]
pub struct SessionManager {
    pool: SqlitePool,
    clock: std::sync::Arc<dyn Clock>,
    ids: std::sync::Arc<dyn IdGenerator>,
}

impl SessionManager {
    pub fn new(
        pool: SqlitePool,
        clock: std::sync::Arc<dyn Clock>,
        ids: std::sync::Arc<dyn IdGenerator>,
    ) -> Self {
        Self { pool, clock, ids }
    }

    pub async fn create(&self, user_id: i64) -> Result<Session, AppError> {
        let id = self.ids.new_id();
        let now = self.clock.now_iso();
        sqlx::query(
            "INSERT INTO sessions (id, user_id, created_at, last_used_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(user_id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(Session {
            id,
            user_id,
            active: true,
            locked_at: None,
            created_at: now.clone(),
            last_used_at: now,
        })
    }

    pub async fn get(&self, session_id: &str) -> Result<Option<Session>, AppError> {
        let row: Option<(String, i64, i64, Option<String>, String, String)> = sqlx::query_as(
            "SELECT id, user_id, active, locked_at, created_at, last_used_at
                 FROM sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(id, user_id, active, locked_at, created_at, last_used_at)| Session {
                id,
                user_id,
                active: active != 0,
                locked_at,
                created_at,
                last_used_at,
            },
        ))
    }

    pub async fn is_active(&self, session_id: &str) -> Result<bool, AppError> {
        let found: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM sessions WHERE id = ? AND active = 1")
                .bind(session_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(found.is_some())
    }

    pub async fn is_locked(&self, session_id: &str) -> Result<bool, AppError> {
        let found: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sessions WHERE id = ? AND active = 1 AND locked_at IS NOT NULL",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(found.is_some())
    }

    pub async fn lock(&self, session_id: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE sessions SET locked_at = ? WHERE id = ? AND active = 1")
            .bind(self.clock.now_iso())
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn unlock(&self, session_id: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE sessions SET locked_at = NULL WHERE id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn touch(&self, session_id: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE sessions SET last_used_at = ? WHERE id = ? AND active = 1")
            .bind(self.clock.now_iso())
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn deactivate(&self, session_id: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE sessions SET active = 0 WHERE id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn revoke_for_user(&self, user_id: i64) -> Result<u64, AppError> {
        let result = sqlx::query("UPDATE sessions SET active = 0 WHERE user_id = ? AND active = 1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}
