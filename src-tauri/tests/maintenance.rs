use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::application::maintenance;
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-maintenance-{label}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

async fn open_state(dir: &std::path::Path) -> AppState {
    let paths = infra::FilePaths::init(dir).unwrap();
    let (pool, _info) = infra::db::open(&paths).await.unwrap();
    AppState::new(pool, paths)
}

async fn make_owner(state: &AppState, username: &str) -> Principal {
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, full_name) VALUES (?, 'x', ?) RETURNING id",
    )
    .bind(username)
    .bind(username)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    let role_id: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE code = 'owner'")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
        .bind(user_id)
        .bind(role_id)
        .execute(&state.pool)
        .await
        .unwrap();
    let permissions: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT p.code
           FROM role_permissions rp
           JOIN permissions p ON p.id = rp.permission_id
          WHERE rp.role_id = ?
          ORDER BY p.code",
    )
    .bind(role_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    Principal {
        session_id: format!("sess-{username}"),
        user_id,
        username: username.to_string(),
        full_name: username.to_string(),
        roles: vec!["owner".to_string()],
        permissions,
    }
}

async fn make_salesperson(state: &AppState, username: &str) -> Principal {
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, full_name) VALUES (?, 'x', ?) RETURNING id",
    )
    .bind(username)
    .bind(username)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    let role_id: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE code = 'salesperson'")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
        .bind(user_id)
        .bind(role_id)
        .execute(&state.pool)
        .await
        .unwrap();
    let permissions: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT p.code
           FROM role_permissions rp
           JOIN permissions p ON p.id = rp.permission_id
          WHERE rp.role_id = ?
          ORDER BY p.code",
    )
    .bind(role_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    Principal {
        session_id: format!("sess-{username}"),
        user_id,
        username: username.to_string(),
        full_name: username.to_string(),
        roles: vec!["salesperson".to_string()],
        permissions,
    }
}

#[tokio::test]
async fn create_backup_requires_permission() {
    let dir = temp_dir("create_requires_perm");
    let state = open_state(&dir).await;
    let sp = make_salesperson(&state, "sarah").await;

    let err = maintenance::create_backup(&state, &sp, "corr-1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "UNAUTHORIZED");
}

#[tokio::test]
async fn list_backups_requires_permission() {
    let dir = temp_dir("list_requires_perm");
    let state = open_state(&dir).await;
    let sp = make_salesperson(&state, "sarah").await;

    let err = maintenance::list_backups(&state, &sp).await.unwrap_err();
    assert_eq!(err.code(), "UNAUTHORIZED");
}

#[tokio::test]
async fn delete_backup_requires_permission() {
    let dir = temp_dir("delete_requires_perm");
    let state = open_state(&dir).await;
    let sp = make_salesperson(&state, "sarah").await;

    let err = maintenance::delete_backup(&state, &sp, "foo.db", "corr-1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "UNAUTHORIZED");
}

#[tokio::test]
async fn restore_backup_requires_permission() {
    let dir = temp_dir("restore_requires_perm");
    let state = open_state(&dir).await;
    let sp = make_salesperson(&state, "sarah").await;

    let err = maintenance::restore_backup(&state, &sp, "foo.db", "corr-1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "UNAUTHORIZED");
}

#[tokio::test]
async fn integrity_check_requires_permission() {
    let dir = temp_dir("integrity_requires_perm");
    let state = open_state(&dir).await;
    let sp = make_salesperson(&state, "sarah").await;

    let err = maintenance::run_integrity_check(&state, &sp, "corr-1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "UNAUTHORIZED");
}

#[tokio::test]
async fn create_list_delete_backup_roundtrip() {
    let dir = temp_dir("create_list_delete");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    // Create
    let result = maintenance::create_backup(&state, &owner, "corr-1")
        .await
        .unwrap();
    assert!(result.verified);
    assert!(result.bytes > 0);

    let filename = std::path::Path::new(&result.backup_path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // List
    let backups = maintenance::list_backups(&state, &owner).await.unwrap();
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].name, filename);
    assert_eq!(backups[0].kind, "manual");
    assert!(backups[0].verified);

    // Delete
    maintenance::delete_backup(&state, &owner, &filename, "corr-2")
        .await
        .unwrap();

    let backups = maintenance::list_backups(&state, &owner).await.unwrap();
    assert!(backups.is_empty());
}

#[tokio::test]
async fn restore_backup_creates_safety_and_marker() {
    let dir = temp_dir("restore_safety_marker");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let backup = maintenance::create_backup(&state, &owner, "corr-1")
        .await
        .unwrap();
    let filename = std::path::Path::new(&backup.backup_path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let result = maintenance::restore_backup(&state, &owner, &filename, "corr-2")
        .await
        .unwrap();
    assert!(result.restart_required);
    assert!(result.safety_backup_name.is_some());

    let marker = infra::restore::read_restore_marker(&state.paths.data_dir).unwrap();
    assert!(marker.is_some());
    let m = marker.unwrap();
    assert_eq!(m.backup_name, filename);

    // Safety backup should exist
    let safety_name = m.safety_backup.unwrap();
    assert!(state.paths.backups_dir.join(&safety_name).exists());
}

#[tokio::test]
async fn delete_backup_rejects_empty_name() {
    let dir = temp_dir("delete_empty");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let err = maintenance::delete_backup(&state, &owner, "", "corr-1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");
}

#[tokio::test]
async fn restore_rejects_nonexistent_backup() {
    let dir = temp_dir("restore_nonexist");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let err = maintenance::restore_backup(&state, &owner, "nonexistent.db", "corr-1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "BACKUP_ERROR");
}

#[tokio::test]
async fn status_returns_sensible_values() {
    let dir = temp_dir("status_sensible");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let s = maintenance::status(&state, &owner).await.unwrap();
    assert!(s.db_size_bytes > 0);
    assert_eq!(s.app_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(s.pending_migrations, 0);
    assert!(s.last_integrity_at.is_none());
    assert!(s.last_integrity_ok.is_none());
    assert!(s.last_backup_name.is_none());
}

#[tokio::test]
async fn integrity_check_runs_and_persists() {
    let dir = temp_dir("integrity_persists");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let result = maintenance::run_integrity_check(&state, &owner, "corr-1")
        .await
        .unwrap();
    assert!(result.page_integrity_ok);
    assert_eq!(result.foreign_key_violations, 0);

    let s = maintenance::status(&state, &owner).await.unwrap();
    assert!(s.last_integrity_at.is_some());
    assert_eq!(s.last_integrity_ok, Some(true));
}

#[tokio::test]
async fn restore_backup_cannot_restore_corrupt_file() {
    let dir = temp_dir("restore_corrupt");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    // Create a backup first, then overwrite the file with garbage
    let backup = maintenance::create_backup(&state, &owner, "corr-1")
        .await
        .unwrap();
    let filename = std::path::Path::new(&backup.backup_path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    fs::write(state.paths.backups_dir.join(&filename), b"not a database").unwrap();

    let err = maintenance::restore_backup(&state, &owner, &filename, "corr-2")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "BACKUP_ERROR");
}

#[tokio::test]
async fn restore_applies_and_swaps_on_pending_restore() {
    // perform_pending_restore runs at startup BEFORE the pool opens. Test it on a
    // fresh data directory with a marker and a backup.
    let source_dir = temp_dir("restore_src");
    let state = open_state(&source_dir).await;
    let owner = make_owner(&state, "alice").await;

    let backup = maintenance::create_backup(&state, &owner, "corr-1")
        .await
        .unwrap();
    let filename = std::path::Path::new(&backup.backup_path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Close the pool — we no longer need the original state.
    state.pool.close().await;
    drop(state);

    // Prepare a fresh target directory (simulating a brand-new app start).
    let target_dir = temp_dir("restore_target");
    let target_state = open_state(&target_dir).await;
    let _target_owner = make_owner(&target_state, "bob").await;

    let original_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&target_state.pool)
        .await
        .unwrap();
    assert_eq!(original_count, 1); // just bob

    target_state.pool.close().await;
    drop(target_state);

    // Copy the backup to target backups dir, write marker, then perform restore.
    let target_paths = infra::FilePaths::init(&target_dir).unwrap();
    fs::copy(
        source_dir.join("backups").join(&filename),
        target_paths.backups_dir.join(&filename),
    )
    .unwrap();

    infra::restore::write_restore_marker(
        &target_paths.data_dir,
        &infra::restore::RestoreMarker {
            backup_name: filename.clone(),
            safety_backup: None,
            staged_path: None,
            legacy_database_only: true,
        },
    )
    .unwrap();

    let restored = infra::restore::perform_pending_restore(&target_paths).unwrap();
    assert_eq!(restored.as_deref(), Some(filename.as_str()));

    // Re-open: the DB should be the source (alice only, no bob).
    let final_state = open_state(&target_dir).await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&final_state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let name: Option<String> = sqlx::query_scalar("SELECT username FROM users LIMIT 1")
        .fetch_one(&final_state.pool)
        .await
        .unwrap();
    assert_eq!(name.as_deref(), Some("alice"));
}
