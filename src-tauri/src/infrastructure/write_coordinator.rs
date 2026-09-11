use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use sqlx::sqlite::{SqliteConnection, SqlitePool};
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

    /// Hold the application-wide write gate without opening a transaction.
    /// Backup uses this to wait for pending writes and keep database references
    /// and their asset files stable while a snapshot package is assembled.
    pub async fn acquire(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.lock.clone().lock_owned().await
    }

    /// Begin a transaction, run `op` on the underlying connection, then commit
    /// (or roll back on error). Writes are globally serialized; reads stay
    /// concurrent. The closure receives a plain `&mut SqliteConnection` so SQL
    /// and repositories work directly on the transaction's connection; this
    /// type is the only place where the transaction wrapper is dereferenced.
    pub async fn execute<T>(
        &self,
        pool: &SqlitePool,
        op: impl for<'a> FnOnce(
            &'a mut SqliteConnection,
        )
            -> Pin<Box<dyn Future<Output = Result<T, AppError>> + Send + 'a>>,
    ) -> Result<T, AppError> {
        let _guard = self.lock.lock().await;
        let mut tx: Transaction<'_, sqlx::Sqlite> = pool.begin().await?;
        let conn: &mut SqliteConnection = &mut tx;
        match op(conn).await {
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
