use serde::Serialize;

use crate::domain::{models::AuditEvent, RoleTemplate};
use crate::error::AppError;
use crate::infrastructure::redact;

pub mod catalogue;
pub mod inventory;
pub mod purchases;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub code: String,
    pub message: String,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_secs: Option<u64>,
}

impl AppErrorDto {
    pub fn from_error(err: &AppError, correlation_id: &str) -> Self {
        AppErrorDto {
            code: err.code().to_string(),
            message: err.to_string(),
            correlation_id: correlation_id.to_string(),
            retry_after_secs: match err {
                AppError::RateLimited(secs) => Some(*secs as u64),
                _ => None,
            },
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

/// Safe view of a user — never includes the password hash.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDto {
    pub id: i64,
    pub username: String,
    pub full_name: String,
    pub is_active: bool,
    pub roles: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDto {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionProfileDto {
    pub session_id: String,
    pub user_id: i64,
    pub username: String,
    pub full_name: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub locked_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstRunStatusDto {
    pub required: bool,
    pub complete: bool,
    pub has_users: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResultDto {
    pub session_id: String,
    pub profile: SessionProfileDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventDto {
    pub id: i64,
    pub user_id: Option<i64>,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub reason: Option<String>,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
    pub approval_user_id: Option<i64>,
    pub session_id: Option<String>,
    pub app_version: Option<String>,
    pub correlation_id: Option<String>,
    pub prev_hash: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditPageDto {
    pub total: i64,
    pub items: Vec<AuditEventDto>,
}

impl From<RoleTemplate> for RoleDto {
    fn from(role: RoleTemplate) -> Self {
        RoleDto {
            id: role.id,
            code: role.code,
            name: role.name,
            description: role.description,
            is_system: role.is_system,
            permissions: Vec::new(),
        }
    }
}

impl From<AuditEvent> for AuditEventDto {
    fn from(event: AuditEvent) -> Self {
        AuditEventDto {
            id: event.id,
            user_id: event.user_id,
            action: event.action,
            entity_type: event.entity_type,
            entity_id: event.entity_id,
            reason: event.reason,
            before_json: event.before_json,
            after_json: event.after_json,
            approval_user_id: event.approval_user_id,
            session_id: event.session_id,
            app_version: event.app_version,
            correlation_id: event.correlation_id,
            prev_hash: event.prev_hash,
            created_at: event.created_at,
        }
    }
}

/// Redaction is repeated at the DTO boundary as a defense-in-depth guarantee:
/// stored JSON already has secrets masked by the audit service, and any value
/// that slips through is still scrubbed before it reaches the webview.
pub fn redacted_json(value: &str) -> String {
    redact(value)
}
