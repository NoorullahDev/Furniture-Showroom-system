use std::time::Duration;

use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::SqlitePool;

use crate::error::AppError;
use super::paths::FilePaths;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone)]
pub struct DbInfo {
    pub version: i64,
    pub pending_migrations: usize,
}

pub async fn open(paths: &FilePaths) -> Result<(SqlitePool, DbInfo), AppError> {
    let options = SqliteConnectOptions::new()
        .filename(&paths.db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(10));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    MIGRATOR.run(&pool).await?;

    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await?;

    let total = MIGRATOR.iter().count() as i64;
    let latest = MIGRATOR.iter().map(|m| m.version).max().unwrap_or(0);
    let pending_migrations = (total - applied).max(0) as usize;

    Ok((
        pool,
        DbInfo {
            version: latest,
            pending_migrations,
        },
    ))
}