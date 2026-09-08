use std::fs;
use std::path::Path;
use std::time::Duration;

use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};

use crate::dto::BackupEntryDto;
use crate::error::AppError;

pub struct CreatedBackup {
    pub backup_path: String,
    pub sha256: String,
    pub verified: bool,
    pub bytes: u64,
}

/// Creates a consistent SQLite snapshot using the online backup API. This
/// works correctly while WAL mode is active and never copies a live file
/// blindly. The snapshot is then checksummed and integrity-checked.
pub fn create_backup(db_path: &Path, backups_dir: &Path) -> Result<CreatedBackup, AppError> {
    fs::create_dir_all(backups_dir)?;

    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let name = format!("furniture_shop-{timestamp}.db");
    let destination = backups_dir.join(&name);

    // The backup API reads a consistent snapshot through the source connection,
    // so it is safe to run while the app has the database open in WAL mode.
    let src = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| AppError::Backup(format!("open source: {e}")))?;

    let mut dst = Connection::open(&destination)
        .map_err(|e| AppError::Backup(format!("open destination: {e}")))?;

    {
        let backup = Backup::new(&src, &mut dst)
            .map_err(|e| AppError::Backup(format!("init backup: {e}")))?;
        backup
            .run_to_completion(64, Duration::from_millis(10), None)
            .map_err(|e| AppError::Backup(format!("run backup: {e}")))?;
    }
    drop(src);
    drop(dst);

    let sha256 = sha256_file(&destination)?;
    let bytes = fs::metadata(&destination)?.len();
    let verified = integrity_check(&destination)?;

    if !verified {
        let _ = fs::remove_file(&destination);
        return Err(AppError::Backup(
            "backup failed integrity verification".into(),
        ));
    }

    Ok(CreatedBackup {
        backup_path: destination.to_string_lossy().into_owned(),
        sha256,
        verified,
        bytes,
    })
}

pub fn list_backups(backups_dir: &Path) -> Result<Vec<BackupEntryDto>, AppError> {
    let mut entries = Vec::new();
    if !backups_dir.exists() {
        return Ok(entries);
    }
    for entry in fs::read_dir(backups_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("db") {
            continue;
        }
        let metadata = entry.metadata()?;
        entries.push(BackupEntryDto {
            name: entry.file_name().to_string_lossy().into_owned(),
            size_bytes: metadata.len(),
            sha256: sha256_file(&path)?,
            created_at: format_utc(&metadata),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn sha256_file(path: &Path) -> Result<String, AppError> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn integrity_check(path: &Path) -> Result<bool, AppError> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| AppError::Backup(format!("verify open: {e}")))?;
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| AppError::Backup(format!("integrity query: {e}")))?;
    Ok(result == "ok")
}

fn format_utc(metadata: &fs::Metadata) -> String {
    if let Ok(modified) = metadata.modified() {
        if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
            let secs = duration.as_secs() as i64;
            if let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) {
                return dt.to_rfc3339();
            }
        }
    }
    String::new()
}
