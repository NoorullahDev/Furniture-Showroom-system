use serde::Serialize;
use thiserror::Error;

use crate::dto::AppErrorDto;

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
    #[error("insufficient stock: {0}")]
    InsufficientStock(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(String),
    #[error("pdf error: {0}")]
    Pdf(String),
    #[error("backup error: {0}")]
    Backup(String),
    #[error("database integrity check failed: {0}")]
    Integrity(String),
    #[error("authentication required: {0}")]
    Unauthorized(String),
    #[error("invalid username or password")]
    InvalidCredentials,
    #[error("session is locked: {0}")]
    SessionLocked(String),
    #[error("too many failed attempts, try again in {0}s")]
    RateLimited(i64),
    #[error("operation not allowed: {0}")]
    Conflict(String),
    #[error("license required: {0}")]
    License(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Validation(_) => "VALIDATION",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Database(_) | AppError::Migrate(_) => "DATABASE",
            AppError::InsufficientStock(_) => "INSUFFICIENT_STOCK",
            AppError::Io(_) => "IO_ERROR",
            AppError::Image(_) => "IMAGE_ERROR",
            AppError::Pdf(_) => "PDF_ERROR",
            AppError::Backup(_) => "BACKUP_ERROR",
            AppError::Integrity(_) => "INTEGRITY_ERROR",
            AppError::Unauthorized(_) => "UNAUTHORIZED",
            AppError::InvalidCredentials => "INVALID_CREDENTIALS",
            AppError::SessionLocked(_) => "SESSION_LOCKED",
            AppError::RateLimited(_) => "RATE_LIMITED",
            AppError::Conflict(_) => "CONFLICT",
            AppError::License(_) => "LICENSE_REQUIRED",
            AppError::Internal(_) => "INTERNAL",
        }
    }

    /// Convert into a DTO carrying the correlation id the server logged under.
    pub fn to_dto(&self, correlation_id: &str) -> AppErrorDto {
        AppErrorDto {
            code: self.code().to_string(),
            message: self.to_string(),
            correlation_id: correlation_id.to_string(),
            retry_after_secs: match self {
                AppError::RateLimited(secs) => Some(*secs as u64),
                _ => None,
            },
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
