use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

const FILE_NAME: &str = "backup-preferences.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPreferences {
    pub directory: Option<String>,
    pub auto_backup_on_close: bool,
}

fn preference_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_NAME)
}

pub fn load(data_dir: &Path) -> Result<BackupPreferences, AppError> {
    let path = preference_path(data_dir);
    if !path.exists() {
        return Ok(BackupPreferences::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| {
        AppError::Backup(format!(
            "cannot read backup settings at {}: {e}",
            path.display()
        ))
    })?;
    serde_json::from_str(&raw).map_err(|e| {
        AppError::Backup(format!(
            "backup settings at {} are invalid: {e}",
            path.display()
        ))
    })
}

pub fn save(data_dir: &Path, preferences: &BackupPreferences) -> Result<(), AppError> {
    let directory = preferences
        .directory
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Validation("select a backup folder first".into()))?;
    let canonical = validate_directory(Path::new(directory))?;
    let saved = BackupPreferences {
        directory: Some(canonical.to_string_lossy().into_owned()),
        auto_backup_on_close: preferences.auto_backup_on_close,
    };
    let raw = serde_json::to_vec_pretty(&saved)
        .map_err(|e| AppError::Internal(format!("encode backup settings: {e}")))?;
    let target = preference_path(data_dir);
    let temporary = data_dir.join(format!(".{FILE_NAME}.{}.tmp", uuid::Uuid::new_v4()));
    {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(&raw)?;
        file.sync_all()?;
    }
    if target.exists() {
        fs::remove_file(&target)?;
    }
    fs::rename(&temporary, &target).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        AppError::Io(e)
    })?;
    Ok(())
}

pub fn configured_directory(data_dir: &Path) -> Result<PathBuf, AppError> {
    let preferences = load(data_dir)?;
    let value = preferences
        .directory
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Validation("select and save a backup folder first".into()))?;
    validate_directory(Path::new(value))
}

/// Resolve the exact selected folder and prove that it is currently writable.
/// No alternate destination is ever selected by this function.
pub fn validate_directory(path: &Path) -> Result<PathBuf, AppError> {
    if !path.exists() {
        return Err(AppError::Backup(format!(
            "backup folder is unavailable: {}",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(AppError::Validation(format!(
            "backup destination is not a folder: {}",
            path.display()
        )));
    }
    let canonical = fs::canonicalize(path).map_err(|e| {
        AppError::Backup(format!(
            "cannot access backup folder {}: {e}",
            path.display()
        ))
    })?;
    let probe = canonical.join(format!(
        ".furniture-backup-write-test-{}.tmp",
        uuid::Uuid::new_v4()
    ));
    let result = (|| -> Result<(), std::io::Error> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)?;
        file.write_all(b"write test")?;
        file.sync_all()?;
        fs::remove_file(&probe)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&probe);
        return Err(AppError::Backup(format!(
            "backup folder is not writable ({}): {error}",
            canonical.display()
        )));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_persist_and_unavailable_folder_is_rejected() {
        let root = std::env::temp_dir().join(format!("backup-prefs-{}", uuid::Uuid::new_v4()));
        let destination = root.join("selected");
        fs::create_dir_all(&destination).unwrap();
        save(
            &root,
            &BackupPreferences {
                directory: Some(destination.to_string_lossy().into_owned()),
                auto_backup_on_close: true,
            },
        )
        .unwrap();
        let loaded = load(&root).unwrap();
        assert!(loaded.auto_backup_on_close);
        assert_eq!(
            Path::new(loaded.directory.as_ref().unwrap()),
            fs::canonicalize(&destination).unwrap()
        );

        fs::remove_dir_all(&destination).unwrap();
        assert!(configured_directory(&root).is_err());
        let _ = fs::remove_dir_all(root);
    }
}
