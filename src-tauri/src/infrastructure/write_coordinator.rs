use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use sqlx::sqlite::SqlitePool;
use sqlx::Transaction;

use crate::error::AppError;

/// Serializes database write transactions across the application so that a
/// concurrent read can never observe an intermediate state and concurrent
/// writers cannot deadlock each other with SQLite's reserved locks.
#[derive(Clone, Debug, Default)]
pub struct WriteCoordinator {
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl WriteCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a transaction, run `op`, then commit (or roll back on error).
    /// Writes are globally serialized; reads stay concurrent.
    pub async fn execute<T>(
        &self,
        pool: &SqlitePool,
        op: impl for<'a> FnOnce(
            &'a mut Transaction<'_, sqlx::Sqlite>,
        )
            -> Pin<Box<dyn Future<Output = Result<T, AppError>> + Send + 'a>>,
    ) -> Result<T, AppError> {
        let _guard = self.lock.lock().await;
        let mut tx = pool.begin().await?;
        match op(&mut tx).await {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(e) => {
                tx.rollback().await?;
                Err(e)
            }
        }
    }
}
