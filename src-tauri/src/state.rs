use sqlx::SqlitePool;

use crate::infrastructure::FilePaths;

pub struct AppState {
    pub pool: SqlitePool,
    pub paths: FilePaths,
}