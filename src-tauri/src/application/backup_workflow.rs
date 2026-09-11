use std::path::PathBuf;

use crate::application::auth::Principal;
use crate::dto::maintenance::{
    BackupInspectionDto, BackupListItemDto, BackupPreferencesDto, RestoreResultDto,
};
use crate::dto::BackupResultDto;
use crate::error::AppError;
use crate::infrastructure::backup_package::{CreatedPackage, PackageInspection};
use crate::infrastructure::backup_preferences::BackupPreferences;
use crate::infrastructure::clock::Clock;
use crate::infrastructure::AuditInput;
use crate::repositories::SettingsRepository;
use crate::state::AppState;

fn current_schema_version() -> i64 {
    crate::infrastructure::db::MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0)
}

pub async fn get_preferences(
    state: &AppState,
    principal: &Principal,
) -> Result<BackupPreferencesDto, AppError> {
    principal.require("backup.create")?;
    let data_dir = state.paths.data_dir.clone();
    let mut preferences = tokio::task::spawn_blocking(move || {
        crate::infrastructure::backup_preferences::load(&data_dir)
    })
    .await
    .map_err(background_error)??;

    // One-time compatibility with the folder captured by the first-run flow.
    if preferences.directory.is_none() {
        if let Some(raw) = SettingsRepository::find(&state.pool, "backup.location").await? {
            if let Ok(value) = serde_json::from_str::<String>(&raw) {
                if !value.trim().is_empty() {
                    preferences.directory = Some(value);
                }
            }
        }
    }
    Ok(BackupPreferencesDto {
        directory: preferences.directory,
        auto_backup_on_close: preferences.auto_backup_on_close,
    })
}

pub async fn save_preferences(
    state: &AppState,
    principal: &Principal,
    directory: String,
    auto_backup_on_close: bool,
    correlation_id: &str,
) -> Result<BackupPreferencesDto, AppError> {
    principal.require("backup.create")?;
    let data_dir = state.paths.data_dir.clone();
    let preferences = BackupPreferences {
        directory: Some(directory),
        auto_backup_on_close,
    };
    let to_save = preferences.clone();
    tokio::task::spawn_blocking(move || {
        crate::infrastructure::backup_preferences::save(&data_dir, &to_save)
    })
    .await
    .map_err(background_error)??;
    let saved = crate::infrastructure::backup_preferences::load(&state.paths.data_dir)?;
    let directory_json = serde_json::json!(saved.directory).to_string();
    let auto_json = serde_json::json!(saved.auto_backup_on_close).to_string();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let now = state.clock.now_iso();
    let correlation = correlation_id.to_owned();
    let audits = state.audits.clone();
    let saved_dir = saved.directory.clone();
    let saved_auto = saved.auto_backup_on_close;
    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                SettingsRepository::upsert(
                    tx,
                    "backup.location",
                    &directory_json,
                    Some(actor_id),
                    &now,
                )
                .await?;
                SettingsRepository::upsert(
                    tx,
                    "backup.auto_on_close",
                    &auto_json,
                    Some(actor_id),
                    &now,
                )
                .await?;
                audits
                    .record(
                        tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "backup.settings.update".into(),
                            entity_type: Some("backup_settings".into()),
                            after_json: Some(
                                serde_json::json!({
                                    "directory": saved_dir,
                                    "auto_backup_on_close": saved_auto,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await?;
    Ok(BackupPreferencesDto {
        directory: saved.directory,
        auto_backup_on_close: saved.auto_backup_on_close,
    })
}

pub async fn create_manual(
    state: &AppState,
    principal: &Principal,
    requested_name: &str,
    correlation_id: &str,
) -> Result<BackupResultDto, AppError> {
    principal.require("backup.create")?;
    let package = create_complete(state, requested_name, "manual").await?;
    record_created(state, &package, Some(principal), correlation_id).await?;
    Ok(result_dto(package))
}

pub async fn create_automatic(state: &AppState) -> Result<BackupResultDto, AppError> {
    let preferences = crate::infrastructure::backup_preferences::load(&state.paths.data_dir)?;
    if !preferences.auto_backup_on_close {
        return Err(AppError::Conflict(
            "automatic backup on close is disabled".into(),
        ));
    }
    let package = create_complete(state, "Automatic", "automatic").await?;
    if let Err(error) = record_created(state, &package, None, "automatic-close").await {
        // The package is already durably saved and validated. Do not create a
        // duplicate on close merely because optional history recording failed.
        tracing::warn!(error = %error, backup = %package.name, "automatic backup history was not recorded");
    }
    Ok(result_dto(package))
}

async fn create_complete(
    state: &AppState,
    name: &str,
    kind: &str,
) -> Result<CreatedPackage, AppError> {
    let destination =
        crate::infrastructure::backup_preferences::configured_directory(&state.paths.data_dir)?;
    let _write_gate = state.write_coordinator.acquire().await;
    let paths = state.paths.clone();
    let requested = name.to_owned();
    let kind = kind.to_owned();
    tokio::task::spawn_blocking(move || {
        crate::infrastructure::backup_package::create_package(
            &paths,
            &destination,
            &requested,
            &kind,
            current_schema_version(),
        )
    })
    .await
    .map_err(background_error)?
}

async fn record_created(
    state: &AppState,
    package: &CreatedPackage,
    principal: Option<&Principal>,
    correlation_id: &str,
) -> Result<(), AppError> {
    let name = package.name.clone();
    let sha = package.sha256.clone();
    let bytes = package.bytes.min(i64::MAX as u64) as i64;
    let kind = package.kind.clone();
    let actor_id = principal.map(|p| p.user_id);
    let actor_session = principal.map(|p| p.session_id.clone());
    let correlation = correlation_id.to_owned();
    let audits = state.audits.clone();
    state.write_coordinator.execute(&state.pool, move |tx| Box::pin(async move {
        sqlx::query(
            "INSERT OR REPLACE INTO backup_history (name, size_bytes, sha256, verified, kind, created_by, created_at)
             VALUES (?, ?, ?, 1, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        ).bind(&name).bind(bytes).bind(&sha).bind(&kind).bind(actor_id).execute(&mut *tx).await?;
        audits.record(tx, AuditInput {
            user_id: actor_id, session_id: actor_session, action: "backup.create".into(),
            entity_type: Some("backup".into()), entity_id: Some(name),
            after_json: Some(serde_json::json!({ "sha256": sha, "size_bytes": bytes, "kind": kind }).to_string()),
            correlation_id: Some(correlation), ..Default::default()
        }).await?;
        Ok(())
    })).await
}

pub async fn list(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<BackupListItemDto>, AppError> {
    principal.require("backup.create")?;
    let destination =
        crate::infrastructure::backup_preferences::configured_directory(&state.paths.data_dir)?;
    let scratch = state.paths.data_dir.clone();
    let items = tokio::task::spawn_blocking(move || {
        crate::infrastructure::backup_package::list_backup_files(
            &destination,
            current_schema_version(),
            &scratch,
        )
    })
    .await
    .map_err(background_error)??;
    Ok(items.into_iter().map(list_item).collect())
}

pub async fn inspect(
    state: &AppState,
    principal: &Principal,
    source: &str,
) -> Result<BackupInspectionDto, AppError> {
    principal.require("backup.restore")?;
    let source = PathBuf::from(source);
    let scratch = state.paths.data_dir.clone();
    let item = tokio::task::spawn_blocking(move || {
        crate::infrastructure::backup_package::inspect_backup(
            &source,
            current_schema_version(),
            &scratch,
        )
    })
    .await
    .map_err(background_error)??;
    Ok(inspection_dto(item))
}

pub async fn delete(
    state: &AppState,
    principal: &Principal,
    name: &str,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("backup.restore")?;
    let destination =
        crate::infrastructure::backup_preferences::configured_directory(&state.paths.data_dir)?;
    let owned = name.to_owned();
    tokio::task::spawn_blocking(move || {
        crate::infrastructure::backup_package::delete_backup_file(&destination, &owned)
    })
    .await
    .map_err(background_error)??;
    let name = name.to_owned();
    let audit_name = name.clone();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_owned();
    let audits = state.audits.clone();
    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                sqlx::query("DELETE FROM backup_history WHERE name = ?")
                    .bind(&name)
                    .execute(&mut *tx)
                    .await?;
                audits
                    .record(
                        tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "backup.delete".into(),
                            entity_type: Some("backup".into()),
                            entity_id: Some(audit_name),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await
}

pub async fn schedule_restore(
    state: &AppState,
    principal: &Principal,
    source_path: &str,
    correlation_id: &str,
) -> Result<RestoreResultDto, AppError> {
    principal.require("backup.restore")?;
    let source = PathBuf::from(source_path);
    let mut machine_settings = sqlx::query_as::<_, (String, String)>(
        "SELECT key, value_json FROM settings
          WHERE key IN ('backup.location', 'backup.auto_on_close') OR key LIKE 'license.%'",
    )
    .fetch_all(&state.pool)
    .await?;
    let preferences = crate::infrastructure::backup_preferences::load(&state.paths.data_dir)?;
    machine_settings.retain(|(key, _)| key != "backup.location" && key != "backup.auto_on_close");
    machine_settings.push((
        "backup.location".into(),
        serde_json::json!(preferences.directory).to_string(),
    ));
    machine_settings.push((
        "backup.auto_on_close".into(),
        serde_json::json!(preferences.auto_backup_on_close).to_string(),
    ));
    let destination =
        crate::infrastructure::backup_preferences::configured_directory(&state.paths.data_dir)?;
    let stage = state
        .paths
        .data_dir
        .join(format!(".restore-stage-{}", uuid::Uuid::new_v4()));
    let _write_gate = state.write_coordinator.acquire().await;
    let paths = state.paths.clone();
    let stage_for_task = stage.clone();
    let source_for_task = source.clone();
    let safety = tokio::task::spawn_blocking(move || -> Result<CreatedPackage, AppError> {
        // Validate and fully stage the selected package before creating a marker.
        crate::infrastructure::backup_package::stage_backup(
            &source_for_task,
            &stage_for_task,
            current_schema_version(),
            &paths.data_dir,
        )?;
        crate::infrastructure::backup_package::preserve_machine_settings(
            &stage_for_task.join("database/furniture_shop.db"),
            &machine_settings,
        )?;
        match crate::infrastructure::backup_package::create_package(
            &paths,
            &destination,
            "Before-Restore",
            "restore-safety",
            current_schema_version(),
        ) {
            Ok(package) => Ok(package),
            Err(error) => {
                let _ = std::fs::remove_dir_all(&stage_for_task);
                Err(error)
            }
        }
    })
    .await
    .map_err(background_error)??;

    let marker = crate::infrastructure::restore::RestoreMarker {
        backup_name: source
            .file_name()
            .map(|v| v.to_string_lossy().into_owned())
            .unwrap_or_else(|| "imported backup".into()),
        safety_backup: Some(safety.name.clone()),
        staged_path: Some(stage.to_string_lossy().into_owned()),
        legacy_database_only: source
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("db"))
            .unwrap_or(false),
    };
    drop(_write_gate);

    if let Err(error) = record_created(state, &safety, Some(principal), correlation_id).await {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(error);
    }
    let source_name = marker.backup_name.clone();
    let safety_name = safety.name.clone();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_owned();
    let audits = state.audits.clone();
    if let Err(error) = state.write_coordinator.execute(&state.pool, move |tx| Box::pin(async move {
        audits.record(tx, AuditInput { user_id: Some(actor_id), session_id: Some(actor_session),
            action: "backup.restore.schedule".into(), entity_type: Some("backup".into()), entity_id: Some(source_name),
            after_json: Some(serde_json::json!({"safety_backup": safety_name, "restart_required": true}).to_string()),
            correlation_id: Some(correlation), ..Default::default() }).await?;
        Ok(())
    })).await {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(error);
    }
    if let Err(error) =
        crate::infrastructure::restore::write_restore_marker(&state.paths.data_dir, &marker)
    {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(error);
    }
    Ok(RestoreResultDto {
        restart_required: true,
        safety_backup_name: Some(safety.name),
    })
}

pub async fn schedule_restore_from_list(
    state: &AppState,
    principal: &Principal,
    name: &str,
    correlation_id: &str,
) -> Result<RestoreResultDto, AppError> {
    if name.is_empty() || name.contains(['/', '\\']) {
        return Err(AppError::Validation("invalid backup name".into()));
    }
    let destination =
        crate::infrastructure::backup_preferences::configured_directory(&state.paths.data_dir)?;
    schedule_restore(
        state,
        principal,
        &destination.join(name).to_string_lossy(),
        correlation_id,
    )
    .await
}

fn result_dto(package: CreatedPackage) -> BackupResultDto {
    BackupResultDto {
        name: package.name,
        backup_path: package.path.to_string_lossy().into_owned(),
        sha256: package.sha256,
        verified: true,
        bytes: package.bytes,
        created_at: package.created_at,
        kind: package.kind,
    }
}

fn list_item(item: PackageInspection) -> BackupListItemDto {
    let verified = item.file_count > 0;
    BackupListItemDto {
        name: item.name,
        full_path: item.path.to_string_lossy().into_owned(),
        size_bytes: item.size_bytes,
        sha256: item.sha256,
        created_at: item.created_at,
        kind: item.kind,
        created_by: None,
        verified,
        app_version: item.app_version,
        schema_version: item.schema_version,
        file_count: item.file_count,
        legacy_database_only: item.legacy_database_only,
    }
}

fn inspection_dto(item: PackageInspection) -> BackupInspectionDto {
    BackupInspectionDto {
        name: item.name,
        full_path: item.path.to_string_lossy().into_owned(),
        size_bytes: item.size_bytes,
        sha256: item.sha256,
        created_at: item.created_at,
        kind: item.kind,
        app_version: item.app_version,
        schema_version: item.schema_version,
        file_count: item.file_count,
        legacy_database_only: item.legacy_database_only,
    }
}

fn background_error(error: tokio::task::JoinError) -> AppError {
    AppError::Internal(format!("background task failed: {error}"))
}
