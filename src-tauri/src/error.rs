use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(String),
    #[error("pdf error: {0}")]
    Pdf(String),
    #[error("backup error: {0}")]
    Backup(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Validation(_) => "VALIDATION",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Database(_) => "DATABASE",
            AppError::Migrate(_) => "DATABASE",
            AppError::Io(_) => "IO_ERROR",
            AppError::Image(_) => "IMAGE_ERROR",
            AppError::Pdf(_) => "PDF_ERROR",
            AppError::Backup(_) => "BACKUP_ERROR",
            AppError::Internal(_) => "INTERNAL",
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ErrorCorrelation {
    pub id: String,
}

pub fn new_correlation_id() -> String {
    uuid::Uuid::now_v7().to_string()
}