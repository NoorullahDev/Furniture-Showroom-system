use std::path::PathBuf;

use crate::dto::{
    AppInfoDto, BackupEntryDto, BackupResultDto, ImageImportResultDto, PdfResultDto,
};
use crate::error::AppError;
use crate::infrastructure;
use crate::state::AppState;

pub async fn app_info(state: &AppState) -> Result<AppInfoDto, AppError> {
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&state.pool)
        .await?;

    let total = infrastructure::db::MIGRATOR.iter().count() as i64;
    let latest = infrastructure::db::MIGRATOR.iter().map(|m| m.version).max().unwrap_or(0);
    let pending_migrations = (total - applied).max(0) as usize;

    Ok(AppInfoDto {
        app_name: "Furniture Shop".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        data_dir: state.paths.data_dir.to_string_lossy().into_owned(),
        db_path: state.paths.db_path.to_string_lossy().into_owned(),
        db_version: latest,
        pending_migrations,
    })
}

pub async fn generate_pdf(state: &AppState) -> Result<PdfResultDto, AppError> {
    let reports_dir = state.paths.reports_dir.clone();
    let fonts_dir = state.paths.fonts_dir.clone();

    let pdf = tokio::task::spawn_blocking(move || {
        infrastructure::generate_proof_pdf(&reports_dir, &fonts_dir)
    })
    .await
    .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    Ok(PdfResultDto {
        report_path: pdf.path,
        pages: pdf.pages,
        bytes: pdf.bytes,
    })
}

pub async fn import_image(state: &AppState, path: String) -> Result<ImageImportResultDto, AppError> {
    if path.trim().is_empty() {
        return Err(AppError::Validation("no file selected".into()));
    }

    let source = PathBuf::from(&path);
    let images_dir = state.paths.images_dir.clone();

    let imported = tokio::task::spawn_blocking(move || {
        infrastructure::import_image(&source, &images_dir)
    })
    .await
    .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    Ok(ImageImportResultDto {
        original_path: imported.original_path,
        stored_name: imported.stored_name,
        width: imported.width,
        height: imported.height,
        thumbnail_bytes: imported.thumbnail_bytes,
    })
}

pub async fn create_backup(state: &AppState) -> Result<BackupResultDto, AppError> {
    let db_path = state.paths.db_path.clone();
    let backups_dir = state.paths.backups_dir.clone();

    let backup = tokio::task::spawn_blocking(move || {
        infrastructure::create_backup(&db_path, &backups_dir)
    })
    .await
    .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    Ok(BackupResultDto {
        backup_path: backup.backup_path,
        sha256: backup.sha256,
        verified: backup.verified,
        bytes: backup.bytes,
    })
}

pub async fn list_backups(state: &AppState) -> Result<Vec<BackupEntryDto>, AppError> {
    let backups_dir = state.paths.backups_dir.clone();
    tokio::task::spawn_blocking(move || infrastructure::list_backups(&backups_dir))
        .await
        .map_err(|e| AppError::Internal(format!("background task failed: {e}")))?
}

pub async fn open_path(state: &AppState, path: String) -> Result<(), AppError> {
    // Proof-only convenience. The path must live inside an approved app
    // directory (reports in this proof).
    let target = PathBuf::from(&path);
    state.paths.ensure_member(&state.paths.reports_dir, &target)?;
    opener::open(&target)
        .map_err(|e| AppError::Internal(format!("failed to open file: {e}")))
}