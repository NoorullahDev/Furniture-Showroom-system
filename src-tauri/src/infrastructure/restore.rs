use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::infrastructure::paths::FilePaths;

/// Marker describing a pending restore that runs on the next app start, before
/// the database is opened. Keeping the swap out of the hot path makes it atomic
/// and WAL-safe (no `-wal`/`-shm` sidecars exist yet).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreMarker {
    pub backup_name: String,
    pub safety_backup: Option<String>,
}

const MARKER_FILE: &str = "restore_pending.json";

fn marker_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join(MARKER_FILE)
}

pub fn read_restore_marker(data_dir: &Path) -> Result<Option<RestoreMarker>, AppError> {
    let path = marker_path(data_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|e| {
        AppError::Internal(format!(
            "failed to read restore marker {}: {e}",
            path.display()
        ))
    })?;
    let marker = serde_json::from_str(&raw).map_err(|e| {
        AppError::Internal(format!("corrupt restore marker {}: {e}", path.display()))
    })?;
    Ok(Some(marker))
}

pub fn write_restore_marker(data_dir: &Path, marker: &RestoreMarker) -> Result<(), AppError> {
    let raw = serde_json::to_string_pretty(marker)
        .map_err(|e| AppError::Internal(format!("failed to encode restore marker: {e}")))?;
    fs::write(marker_path(data_dir), raw)?;
    Ok(())
}

pub fn clear_restore_marker(data_dir: &Path) -> Result<(), AppError> {
    let path = marker_path(data_dir);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// Applies a pending restore, if any. Must be called BEFORE the database pool
/// is opened. The source backup is verified, copied to a temp file on the same
/// volume, then atomically renamed over the live database; stale WAL/SHM
/// sidecars are removed. On validation failure the live database is left
/// untouched and the marker is preserved so the failure can be surfaced.
pub fn perform_pending_restore(paths: &FilePaths) -> Result<Option<String>, AppError> {
    let Some(marker) = read_restore_marker(&paths.data_dir)? else {
        return Ok(None);
    };

    let source = paths.backups_dir.join(&marker.backup_name);
    if !source.exists() {
        return Err(AppError::NotFound(format!(
            "pending restore source `{}` is missing",
            marker.backup_name
        )));
    }
    paths.ensure_member(&paths.backups_dir, &source)?;

    let verified = crate::infrastructure::backup::verify_backup_file(&source)?;
    if !verified {
        return Err(AppError::Integrity(format!(
            "pending restore source `{}` failed integrity verification",
            marker.backup_name
        )));
    }

    let tmp = paths
        .data_dir
        .join(format!("restore-{}.db.tmp", std::process::id()));
    fs::copy(&source, &tmp)?;

    if paths.db_path.exists() {
        fs::remove_file(&paths.db_path)?;
    }
    fs::rename(&tmp, &paths.db_path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        AppError::Io(e)
    })?;

    for suffix in ["-wal", "-shm"] {
        let sidecar = paths.db_path.with_extension(format!("db{suffix}"));
        if sidecar.exists() {
            fs::remove_file(&sidecar)?;
        }
    }

    clear_restore_marker(&paths.data_dir)?;
    Ok(Some(marker.backup_name))
}
