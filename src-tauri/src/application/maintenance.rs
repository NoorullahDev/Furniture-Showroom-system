use crate::application::auth::Principal;
use crate::dto::maintenance::{
    BackupListItemDto, IntegrityResultDto, MaintenanceStatusDto, RestoreResultDto,
};
use crate::dto::BackupResultDto;
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::infrastructure::AuditInput;
use crate::repositories::SettingsRepository;
use crate::state::AppState;

async fn insert_history(
    conn: &mut sqlx::sqlite::SqliteConnection,
    name: &str,
    size_bytes: i64,
    sha256: &str,
    kind: &str,
    created_by: Option<i64>,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT OR IGNORE INTO backup_history (name, size_bytes, sha256, verified, kind, created_by)
         VALUES (?, ?, ?, 1, ?, ?)",
    )
    .bind(name)
    .bind(size_bytes)
    .bind(sha256)
    .bind(kind)
    .bind(created_by)
    .execute(conn)
    .await?;
    Ok(())
}

async fn remove_history(
    conn: &mut sqlx::sqlite::SqliteConnection,
    name: &str,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM backup_history WHERE name = ?")
        .bind(name)
        .execute(conn)
        .await?;
    Ok(())
}

async fn list_history_rows(
    state: &AppState,
) -> Result<Vec<(String, String, i64, String, bool)>, AppError> {
    let rows = sqlx::query_as::<_, (String, String, i64, String, bool)>(
        "SELECT bh.name, bh.sha256, bh.size_bytes, bh.kind, bh.verified > 0
           FROM backup_history bh",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows)
}

/// Authorized manual backup. Requires `backup.create`. Writes a verified
/// snapshot, records history, and audits the event.
pub async fn create_backup(
    state: &AppState,
    principal: &Principal,
    correlation_id: &str,
) -> Result<BackupResultDto, AppError> {
    principal.require("backup.create")?;

    let db_path = state.paths.db_path.clone();
    let backups_dir = state.paths.backups_dir.clone();
    let backup = tokio::task::spawn_blocking(move || {
        crate::infrastructure::create_backup(&db_path, &backups_dir)
    })
    .await
    .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    let name = std::path::Path::new(&backup.backup_path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name_copy = name.clone();
    let sha_copy = backup.sha256.clone();
    let bytes = backup.bytes as i64;
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                insert_history(tx, &name_copy, bytes, &sha_copy, "manual", Some(actor_id)).await?;
                audits
                    .record(
                        tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "backup.create".into(),
                            entity_type: Some("backup".into()),
                            entity_id: Some(name_copy),
                            after_json: Some(
                                serde_json::json!({
                                    "sha256": sha_copy,
                                    "size_bytes": bytes,
                                    "verified": true,
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

    Ok(BackupResultDto {
        backup_path: backup.backup_path,
        sha256: backup.sha256,
        verified: backup.verified,
        bytes: backup.bytes,
    })
}

/// List backups for the maintenance page. Requires `backup.create`. Enriches
/// the filesystem listing with history metadata (kind, creator, verified).
pub async fn list_backups(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<BackupListItemDto>, AppError> {
    principal.require("backup.create")?;
    let backups_dir = state.paths.backups_dir.clone();
    let fs_entries =
        tokio::task::spawn_blocking(move || crate::infrastructure::list_backups(&backups_dir))
            .await
            .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    let history = list_history_rows(state).await?;
    let mut by_name: std::collections::HashMap<&str, (&str, i64, bool)> =
        std::collections::HashMap::new();
    for (name, _sha, size, kind, verified) in &history {
        by_name.insert(name.as_str(), (kind.as_str(), *size, *verified));
    }

    let mut created_by_names = sqlx::query_as::<_, (i64, String)>("SELECT id, username FROM users")
        .fetch_all(&state.pool)
        .await?;
    created_by_names.dedup_by_key(|(id, _)| *id);
    let name_lookup: std::collections::HashMap<i64, String> =
        created_by_names.into_iter().collect();

    let rows = sqlx::query_as::<_, (String, Option<i64>, String, String)>(
        "SELECT bh.name, bh.created_by, bh.kind, bh.created_at FROM backup_history bh",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut items: Vec<BackupListItemDto> = fs_entries
        .into_iter()
        .map(|entry| {
            let (kind, size_bytes, verified) = by_name
                .get(entry.name.as_str())
                .copied()
                .unwrap_or(("manual", entry.size_bytes as i64, true));
            BackupListItemDto {
                name: entry.name,
                size_bytes: size_bytes.max(0) as u64,
                sha256: entry.sha256,
                created_at: entry.created_at,
                kind: kind.to_string(),
                created_by: None,
                verified,
            }
        })
        .collect();

    let row_map: std::collections::HashMap<String, (Option<i64>, String, String)> = rows
        .into_iter()
        .map(|(name, created_by, kind, created_at)| (name, (created_by, kind, created_at)))
        .collect();

    for item in &mut items {
        if let Some((created_by, kind, created_at)) = row_map.get(&item.name) {
            item.created_by = created_by.and_then(|id| name_lookup.get(&id).cloned());
            item.kind = kind.clone();
            item.created_at = created_at.clone();
        }
    }

    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

/// Delete a backup. Requires `backup.restore` (destructive). Audits the event.
pub async fn delete_backup(
    state: &AppState,
    principal: &Principal,
    name: &str,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("backup.restore")?;
    if name.trim().is_empty() {
        return Err(AppError::Validation("backup name is required".into()));
    }

    let backups_dir = state.paths.backups_dir.clone();
    let name_owned = name.to_owned();
    let name_copy = name_owned.clone();
    tokio::task::spawn_blocking(move || {
        crate::infrastructure::delete_backup(&backups_dir, &name_owned)
    })
    .await
    .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let history_name = name_copy.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                remove_history(tx, &history_name).await?;
                audits
                    .record(
                        tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "backup.delete".into(),
                            entity_type: Some("backup".into()),
                            entity_id: Some(history_name),
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

/// Restore a backup. Requires `backup.restore`. Never touches the live DB
/// immediately: it creates a `restore-safety` snapshot of the current data,
/// writes a pending-restore marker, and audits the event. The swap happens on
/// next startup via `perform_pending_restore`.
pub async fn restore_backup(
    state: &AppState,
    principal: &Principal,
    name: &str,
    correlation_id: &str,
) -> Result<RestoreResultDto, AppError> {
    principal.require("backup.restore")?;
    if name.trim().is_empty() {
        return Err(AppError::Validation("backup name is required".into()));
    }

    let db_path = state.paths.db_path.clone();
    let backups_dir = state.paths.backups_dir.clone();
    let source_name = name.to_owned();
    let backups_dir_owned = backups_dir.clone();

    let verified = tokio::task::spawn_blocking(move || {
        let source_path = backups_dir_owned.join(&source_name);
        crate::infrastructure::verify_backup_file(&source_path)
    })
    .await
    .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    if !verified {
        return Err(AppError::Integrity(format!(
            "backup `{name}` failed integrity verification and was not restored"
        )));
    }

    let safety = tokio::task::spawn_blocking(move || {
        crate::infrastructure::create_backup(&db_path, &backups_dir)
    })
    .await
    .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;
    let safety_name = std::path::Path::new(&safety.backup_path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let safety_name_copy = safety_name.clone();

    let data_dir = state.paths.data_dir.clone();
    let marker_name = name.to_owned();
    let marker_name_copy = marker_name.clone();
    let marker_safety = safety_name.clone();
    tokio::task::spawn_blocking(move || {
        crate::infrastructure::restore::write_restore_marker(
            &data_dir,
            &crate::infrastructure::restore::RestoreMarker {
                backup_name: marker_name,
                safety_backup: Some(marker_safety),
            },
        )
    })
    .await
    .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    let safety_bytes = safety.bytes as i64;
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let history_safety_name = safety_name.clone();
    let history_safety_sha = safety.sha256.clone();
    let audit_safety_name = safety_name.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                insert_history(
                    tx,
                    &history_safety_name,
                    safety_bytes,
                    &history_safety_sha,
                    "restore-safety",
                    Some(actor_id),
                )
                .await?;
                audits
                    .record(
                        tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "backup.restore".into(),
                            entity_type: Some("backup".into()),
                            entity_id: Some(marker_name_copy),
                            after_json: Some(
                                serde_json::json!({
                                    "safety_backup": audit_safety_name,
                                    "restart_required": true,
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

    Ok(RestoreResultDto {
        restart_required: true,
        safety_backup_name: Some(safety_name_copy),
    })
}

fn settings_bool(value: Option<String>) -> Option<bool> {
    value.as_deref().map(|v| v.trim() == "true")
}

/// Maintenance status summary. Requires `backup.create`.
pub async fn status(
    state: &AppState,
    principal: &Principal,
) -> Result<MaintenanceStatusDto, AppError> {
    principal.require("backup.create")?;

    let db_path = state.paths.db_path.clone();
    let db_size_bytes = tokio::task::spawn_blocking(move || {
        std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0)
    })
    .await
    .map_err(|e| AppError::Internal(format!("background task failed: {e}")))?;

    let images_dir = state.paths.images_dir.clone();
    let backups_dir = state.paths.backups_dir.clone();
    let (images_size_bytes, backups_size_bytes) = tokio::task::spawn_blocking(move || {
        let img = crate::infrastructure::disk_size::dir_size(&images_dir).unwrap_or(0);
        let bck = crate::infrastructure::disk_size::dir_size(&backups_dir).unwrap_or(0);
        (img, bck)
    })
    .await
    .map_err(|e| AppError::Internal(format!("background task failed: {e}")))?;

    let data_dir = state.paths.data_dir.clone();
    let free_disk_bytes = tokio::task::spawn_blocking(move || {
        crate::infrastructure::disk_size::free_disk_bytes(&data_dir).unwrap_or(None)
    })
    .await
    .map_err(|e| AppError::Internal(format!("background task failed: {e}")))?;

    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&state.pool)
        .await?;
    let total = crate::infrastructure::db::MIGRATOR.iter().count() as i64;
    let latest = crate::infrastructure::db::MIGRATOR
        .iter()
        .map(|m| m.version)
        .max()
        .unwrap_or(0);
    let pending_migrations = (total - applied).max(0) as usize;

    let history = list_history_rows(state).await?;
    let last_backup = history
        .iter()
        .max_by_key(|(_, _, _, _, verified)| if *verified { 1 } else { 0 })
        .map(|(name, _, _, _, _)| name.clone());
    let last_backup_at =
        sqlx::query_scalar::<_, Option<String>>("SELECT MAX(created_at) FROM backup_history")
            .fetch_one(&state.pool)
            .await?;

    let last_integrity_at =
        crate::application::settings::get(state, "maintenance.last_integrity_at").await?;
    let last_integrity_ok = settings_bool(
        crate::application::settings::get(state, "maintenance.last_integrity_ok").await?,
    );

    Ok(MaintenanceStatusDto {
        app_version: env!("CARGO_PKG_VERSION").into(),
        db_size_bytes,
        images_size_bytes,
        backups_size_bytes,
        free_disk_bytes,
        schema_version: latest,
        pending_migrations,
        last_backup_name: last_backup,
        last_backup_at,
        last_integrity_at,
        last_integrity_ok,
    })
}

/// On-demand integrity check. Requires `backup.create`. Persists the result in
/// settings and audits the event.
pub async fn run_integrity_check(
    state: &AppState,
    principal: &Principal,
    correlation_id: &str,
) -> Result<IntegrityResultDto, AppError> {
    principal.require("backup.create")?;
    let info = crate::infrastructure::db::integrity_check(&state.pool).await?;
    let page_ok = info.page_integrity_ok;
    let fk_violations = info.foreign_key_violations;
    let succeeded = page_ok && fk_violations == 0;
    let now = state.clock.now_iso();

    let now_copy = now.clone();
    let ok_val = succeeded.to_string();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                SettingsRepository::upsert(
                    tx,
                    "maintenance.last_integrity_at",
                    &now,
                    Some(actor_id),
                    &now,
                )
                .await?;
                SettingsRepository::upsert(
                    tx,
                    "maintenance.last_integrity_ok",
                    &ok_val,
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
                            action: "maintenance.integrity".into(),
                            entity_type: Some("database".into()),
                            after_json: Some(
                                serde_json::json!({
                                    "page_integrity_ok": page_ok,
                                    "foreign_key_violations": fk_violations,
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

    Ok(IntegrityResultDto {
        page_integrity_ok: page_ok,
        foreign_key_violations: fk_violations,
        checked_at: now_copy,
    })
}
