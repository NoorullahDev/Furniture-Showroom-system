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
    #[serde(default)]
    pub staged_path: Option<String>,
    #[serde(default)]
    pub legacy_database_only: bool,
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

    if let Some(staged_path) = marker.staged_path.as_deref() {
        apply_staged_restore(
            paths,
            Path::new(staged_path),
            &marker.backup_name,
            marker.legacy_database_only,
        )?;
        clear_restore_marker(&paths.data_dir)?;
        return Ok(Some(marker.backup_name));
    }

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

fn apply_staged_restore(
    paths: &FilePaths,
    stage: &Path,
    backup_name: &str,
    legacy_database_only: bool,
) -> Result<(), AppError> {
    let canonical_data = fs::canonicalize(&paths.data_dir)?;
    let canonical_stage = fs::canonicalize(stage)
        .map_err(|e| AppError::Backup(format!("restore staging area is unavailable: {e}")))?;
    if !canonical_stage.starts_with(&canonical_data) || canonical_stage == canonical_data {
        return Err(AppError::Validation(
            "restore staging path is outside application data".into(),
        ));
    }
    let staged_db = canonical_stage.join("database/furniture_shop.db");
    if !staged_db.exists() || !crate::infrastructure::backup::verify_backup_file(&staged_db)? {
        return Err(AppError::Integrity(format!(
            "staged restore `{backup_name}` failed final database verification"
        )));
    }

    let rollback = paths
        .data_dir
        .join(format!(".restore-rollback-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&rollback)?;
    let targets: &[&str] = if legacy_database_only {
        &[]
    } else {
        &["images", "branding", "fonts", "attachments", "templates"]
    };

    for suffix in ["-wal", "-shm"] {
        let sidecar = paths.db_path.with_extension(format!("db{suffix}"));
        if sidecar.exists() {
            remove_file_with_retry(&sidecar)?;
        }
    }

    let mut old_database_moved = false;
    let mut new_database_moved = false;
    let mut old_directories_moved: Vec<String> = Vec::new();
    let mut new_directories_moved: Vec<String> = Vec::new();
    let swap_result = (|| -> Result<(), AppError> {
        if paths.db_path.exists() {
            rename_with_retry(&paths.db_path, &rollback.join("furniture_shop.db"))?;
            old_database_moved = true;
        }
        for name in targets {
            let current = paths.data_dir.join(name);
            if current.exists() {
                rename_with_retry(&current, &rollback.join(name))?;
                old_directories_moved.push((*name).to_string());
            }
            rename_with_retry(&canonical_stage.join(name), &current)?;
            new_directories_moved.push((*name).to_string());
        }
        rename_with_retry(&staged_db, &paths.db_path)?;
        new_database_moved = true;
        Ok(())
    })();

    if let Err(error) = swap_result {
        if new_database_moved && paths.db_path.exists() {
            let _ = fs::remove_file(&paths.db_path);
        }
        if old_database_moved && rollback.join("furniture_shop.db").exists() {
            let _ = fs::rename(rollback.join("furniture_shop.db"), &paths.db_path);
        }
        for name in new_directories_moved.iter().rev() {
            let current = paths.data_dir.join(name);
            if current.exists() {
                let _ = fs::remove_dir_all(&current);
            }
        }
        for name in old_directories_moved.iter().rev() {
            let current = paths.data_dir.join(name);
            let old = rollback.join(name);
            if old.exists() {
                let _ = fs::rename(old, current);
            }
        }
        return Err(AppError::Backup(format!(
            "restore switch failed; current data was rolled back: {error}"
        )));
    }

    let _ = fs::remove_dir_all(&canonical_stage);
    let _ = fs::remove_dir_all(&rollback);
    Ok(())
}

fn rename_with_retry(source: &Path, destination: &Path) -> Result<(), AppError> {
    let mut last_error = None;
    for attempt in 0..40 {
        match fs::rename(source, destination) {
            Ok(()) => return Ok(()),
            Err(error)
                if error.kind() == std::io::ErrorKind::PermissionDenied
                    || error.raw_os_error() == Some(32) =>
            {
                last_error = Some(error);
                if attempt < 39 {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(AppError::Backup(format!(
        "could not move {} to {}: {}",
        source.display(),
        destination.display(),
        last_error.expect("rename retry records an error")
    )))
}

fn remove_file_with_retry(path: &Path) -> Result<(), AppError> {
    let mut last_error = None;
    for attempt in 0..40 {
        match fs::remove_file(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error)
                if error.kind() == std::io::ErrorKind::PermissionDenied
                    || error.raw_os_error() == Some(32) =>
            {
                last_error = Some(error);
                if attempt < 39 {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(AppError::Backup(format!(
        "could not remove database sidecar {}: {}",
        path.display(),
        last_error.expect("remove retry records an error")
    )))
}
