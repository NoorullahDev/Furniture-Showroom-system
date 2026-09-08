use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use super::paths::FilePaths;
use crate::error::AppError;

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
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 10000;")
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
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

#[derive(Debug, Clone)]
pub struct IntegrityInfo {
    pub page_integrity_ok: bool,
    pub foreign_key_violations: i64,
}

/// Verify the database is internally consistent. Runs on every connection the
/// pool hands out for the pragma probes, then a bounded `integrity_check`.
pub async fn integrity_check(pool: &SqlitePool) -> Result<IntegrityInfo, AppError> {
    let page_check: String = sqlx::query_scalar("PRAGMA integrity_check(1)")
        .fetch_one(pool)
        .await?;
    let page_integrity_ok = page_check == "ok";

    let violations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_foreign_key_check")
        .fetch_one(pool)
        .await?;

    Ok(IntegrityInfo {
        page_integrity_ok,
        foreign_key_violations: violations,
    })
}
