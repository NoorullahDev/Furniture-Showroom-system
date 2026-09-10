use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceStatusDto {
    pub app_version: String,
    pub db_size_bytes: u64,
    pub images_size_bytes: u64,
    pub backups_size_bytes: u64,
    pub free_disk_bytes: Option<u64>,
    pub schema_version: i64,
    pub pending_migrations: usize,
    pub last_backup_name: Option<String>,
    pub last_backup_at: Option<String>,
    pub last_integrity_at: Option<String>,
    pub last_integrity_ok: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupListItemDto {
    pub name: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub created_at: String,
    pub kind: String,
    pub created_by: Option<String>,
    pub verified: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResultDto {
    pub restart_required: bool,
    pub safety_backup_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityResultDto {
    pub page_integrity_ok: bool,
    pub foreign_key_violations: i64,
    pub checked_at: String,
}
