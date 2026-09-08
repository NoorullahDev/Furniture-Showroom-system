use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;

pub struct FilePaths {
    pub data_dir: PathBuf,
    pub images_dir: PathBuf,
    pub reports_dir: PathBuf,
    pub backups_dir: PathBuf,
    pub fonts_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub db_path: PathBuf,
}

impl FilePaths {
    pub fn init(data_dir: &Path) -> Result<Self, AppError> {
        let images_dir = data_dir.join("images");
        let reports_dir = data_dir.join("reports");
        let backups_dir = data_dir.join("backups");
        let fonts_dir = data_dir.join("fonts");
        let logs_dir = data_dir.join("logs");
        let db_path = data_dir.join("furniture_shop.db");

        for dir in [
            &images_dir,
            &reports_dir,
            &backups_dir,
            &fonts_dir,
            &logs_dir,
        ] {
            fs::create_dir_all(dir)?;
        }

        Ok(Self {
            data_dir: data_dir.to_path_buf(),
            images_dir,
            reports_dir,
            backups_dir,
            fonts_dir,
            logs_dir,
            db_path,
        })
    }

    pub fn ensure_member(&self, base: &Path, candidate: &Path) -> Result<(), AppError> {
        let base_canonical = fs::canonicalize(base)
            .map_err(|_| AppError::Internal("base directory missing".into()))?;
        let canonical = fs::canonicalize(candidate)
            .map_err(|_| AppError::NotFound("path does not exist".into()))?;
        if canonical.starts_with(&base_canonical) {
            Ok(())
        } else {
            Err(AppError::Validation(
                "path escapes the approved application directory".into(),
            ))
        }
    }
}
