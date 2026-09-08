use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application;
use furniture_shop_lib::error::AppError;
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::infrastructure::Clock;
use furniture_shop_lib::infrastructure::IdGenerator;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-phase1-{label}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

async fn open_state(dir: &std::path::Path) -> AppState {
    let paths = infra::FilePaths::init(dir).unwrap();
    let (pool, _info) = infra::db::open(&paths).await.unwrap();
    AppState::new(pool, paths)
}

#[tokio::test]
async fn every_pooled_connection_gets_pragmas_applied() {
    let dir = temp_dir("pragmas");
    let state = open_state(&dir).await;

    for _ in 0..8 {
        let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(fk, 1, "foreign_keys must be ON on every connection");

        let busy: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(busy, 10000);
    }

    let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(journal.to_ascii_lowercase(), "wal");

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn startup_integrity_check_passes_on_clean_db() {
    let dir = temp_dir("integrity");
    let state = open_state(&dir).await;

    let info = infra::db::integrity_check(&state.pool).await.unwrap();
    assert!(info.page_integrity_ok);
    assert_eq!(info.foreign_key_violations, 0);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn settings_service_round_trips_through_write_coordinator() {
    let dir = temp_dir("settings");
    let state = open_state(&dir).await;

    application::settings::set(&state, "shop.name", "\"Proof Shop\"", None)
        .await
        .unwrap();
    let value = application::settings::get(&state, "shop.name")
        .await
        .unwrap();
    assert_eq!(value.as_deref(), Some("\"Proof Shop\""));

    // Upsert updates in place.
    application::settings::set(&state, "shop.name", "\"Renamed Shop\"", Some(1))
        .await
        .unwrap();
    let value = application::settings::get(&state, "shop.name")
        .await
        .unwrap();
    assert_eq!(value.as_deref(), Some("\"Renamed Shop\""));

    let updated_by: Option<i64> =
        sqlx::query_scalar("SELECT (updated_by IS NOT NULL) FROM settings WHERE key = 'shop.name'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(updated_by, Some(1));

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn write_transaction_rolls_back_on_error() {
    let dir = temp_dir("rollback");
    let state = open_state(&dir).await;

    let before = application::settings::get(&state, "shop.name")
        .await
        .unwrap();

    let result = state
        .write_coordinator
        .execute(
            &state.pool,
            |tx| -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), AppError>> + Send + '_>,
            > {
                Box::pin(async move {
                    sqlx::query(
                        "INSERT INTO settings (key, value_json) VALUES ('shop.name', '\"X\"')",
                    )
                    .execute(tx)
                    .await?;
                    // Force a failure after the insert; nothing may persist.
                    Err(AppError::Validation("simulated failure".into()))
                })
            },
        )
        .await;

    assert!(result.is_err());
    let after = application::settings::get(&state, "shop.name")
        .await
        .unwrap();
    assert_eq!(before, after, "rollback must undo the insert");

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sessions_create_touch_and_deactivate() {
    let dir = temp_dir("sessions");
    let state = open_state(&dir).await;

    // Seed one user (password hash is a dummy; auth logic arrives in Phase 2).
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, password_hash, full_name) VALUES (?, ?, ?)")
            .bind("owner")
            .bind("x")
            .bind("Owner")
            .execute(&state.pool)
            .await
            .unwrap()
            .last_insert_rowid();

    let session = state
        .sessions
        .create(user_id)
        .await
        .expect("create session");
    assert!(state.sessions.is_active(&session.id).await.unwrap());
    let row: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE user_id = ? AND active = 1")
            .bind(user_id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(row, 1);

    state.sessions.touch(&session.id).await.unwrap();
    state.sessions.deactivate(&session.id).await.unwrap();
    assert!(!state.sessions.is_active(&session.id).await.unwrap());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn logging_redacts_secret_material() {
    let samples = [
        ("password=sup3rs3cret", "password=[REDACTED]"),
        ("token=abc123, stay=1", "token=[REDACTED], stay=1"),
        (
            "Authorization: Bearer x.y.z",
            "Authorization: Bearer [REDACTED]",
        ),
    ];
    for (input, expected) in samples {
        assert_eq!(infra::redact(input), expected);
    }
}

#[test]
fn capabilities_deny_shell_fs_and_network() {
    let manifest = include_str!("../capabilities/default.json");
    let caps: serde_json::Value = serde_json::from_str(manifest).unwrap();
    let permissions = caps["permissions"]
        .as_array()
        .expect("capability must list permissions");

    for blocked in ["shell", "fs", "http", "process", "window-state", "updater"] {
        let blocked_prefix = format!("{blocked}:");
        for permission in permissions {
            let text = permission.as_str().unwrap();
            assert!(
                !(text == blocked || text.starts_with(&blocked_prefix)),
                "permission `{text}` is not least-privilege for Phase 1"
            );
        }
    }

    assert!(
        permissions
            .iter()
            .any(|p| p.as_str() == Some("core:default")),
        "core:default capability must be present"
    );
    assert!(
        permissions
            .iter()
            .any(|p| p.as_str() == Some("dialog:default")),
        "dialog capability must be present"
    );
}

#[test]
fn id_and_clock_foundations_are_available() {
    let gen = infra::UuidIdGenerator;
    let id = gen.new_id();
    assert_eq!(id.len(), 36);
    let now = infra::SystemClock.now_iso();
    assert!(now.contains('T') && now.contains('Z'));
}
