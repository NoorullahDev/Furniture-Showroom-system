use serde::Serialize;

use crate::error::AppError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub code: String,
    pub message: String,
    pub correlation_id: String,
}

impl AppErrorDto {
    pub fn from_error(err: &AppError, correlation_id: &str) -> Self {
        AppErrorDto {
            code: err.code().to_string(),
            message: err.to_string(),
            correlation_id: correlation_id.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub app_name: String,
    pub version: String,
    pub data_dir: String,
    pub db_path: String,
    pub db_version: i64,
    pub pending_migrations: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfResultDto {
    pub report_path: String,
    pub pages: usize,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageImportResultDto {
    pub original_path: String,
    pub stored_name: String,
    pub width: u32,
    pub height: u32,
    pub thumbnail_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResultDto {
    pub backup_path: String,
    pub sha256: String,
    pub verified: bool,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntryDto {
    pub name: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub created_at: String,
}
