use crate::application::auth;
use crate::dto::{FirstRunStatusDto, LoginResultDto};
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::infrastructure::{password, AuditInput};
use crate::state::AppState;

const FIRST_RUN_KEY: &str = "system.first_run_complete";
const INITIAL_ADMIN_USERNAME: &str = "admin";
const INITIAL_ADMIN_PASSWORD: &str = "admin123";

#[derive(Debug, Clone)]
pub struct FirstRunInput {
    pub shop_name: String,
    pub shop_address: String,
    pub shop_phone: String,
    pub shop_email: String,
    pub currency: String,
    pub timezone: String,
    pub invoice_prefix: Option<String>,
    pub backup_location: Option<String>,
    pub owner_username: String,
    pub owner_full_name: String,
    pub owner_password: String,
}

pub async fn status(state: &AppState) -> Result<FirstRunStatusDto, AppError> {
    let complete: String = sqlx::query_scalar("SELECT value_json FROM settings WHERE key = ?")
        .bind(FIRST_RUN_KEY)
        .fetch_optional(&state.pool)
        .await?
        .unwrap_or_else(|| "false".into());
    let has_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await?;
    let complete = complete.trim_matches('"') == "true";
    Ok(FirstRunStatusDto {
        required: !complete,
        complete,
        has_users: has_users > 0,
    })
}

/// Seed the requested initial administrator exactly once.
///
/// Any user already assigned the owner role counts as an administrator, even
/// if that account is inactive. This intentionally prevents startup from
/// bypassing an intentional deactivation. Likewise, an existing `admin`
/// username is never overwritten, promoted, or given a replacement password.
pub async fn ensure_initial_administrator(state: &AppState) -> Result<bool, AppError> {
    let owner_exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
           FROM user_roles ur
           JOIN roles r ON r.id = ur.role_id
          WHERE r.code = 'owner'",
    )
    .fetch_one(&state.pool)
    .await?;
    if owner_exists > 0 {
        return Ok(false);
    }

    let username_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username = ?")
        .bind(INITIAL_ADMIN_USERNAME)
        .fetch_one(&state.pool)
        .await?;
    if username_exists > 0 {
        tracing::warn!(
            username = INITIAL_ADMIN_USERNAME,
            "initial administrator not created because the username already exists"
        );
        return Ok(false);
    }

    password::validate_strength(INITIAL_ADMIN_PASSWORD)?;
    let hash = password::hash_password(INITIAL_ADMIN_PASSWORD)?;
    let now = state.clock.now_iso();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                // Repeat both guards inside the serialized write transaction.
                let owner_exists: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*)
                       FROM user_roles ur
                       JOIN roles r ON r.id = ur.role_id
                      WHERE r.code = 'owner'",
                )
                .fetch_one(&mut *tx)
                .await?;
                let username_exists: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username = ?")
                        .bind(INITIAL_ADMIN_USERNAME)
                        .fetch_one(&mut *tx)
                        .await?;
                if owner_exists > 0 || username_exists > 0 {
                    return Ok(false);
                }

                let existing_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
                    .fetch_one(&mut *tx)
                    .await?;
                let user_id = sqlx::query(
                    "INSERT INTO users (username, password_hash, full_name, created_at, updated_at)
                     VALUES (?, ?, 'Administrator', ?, ?)",
                )
                .bind(INITIAL_ADMIN_USERNAME)
                .bind(&hash)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                let assigned = sqlx::query(
                    "INSERT INTO user_roles (user_id, role_id)
                     SELECT ?, id FROM roles WHERE code = 'owner'",
                )
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
                if assigned.rows_affected() != 1 {
                    return Err(AppError::Internal("owner role is unavailable".into()));
                }

                // On a brand-new database this account is the initial setup.
                // Seed only the defaults the existing first-run flow would
                // otherwise require, without touching values on an established
                // installation that merely lost its administrator.
                if existing_users == 0 {
                    let defaults: &[(&str, &str)] = &[
                        ("shop.name", "\"Furniture Showroom\""),
                        ("shop.currency", "\"PKR\""),
                        ("shop.timezone", "\"Asia/Karachi\""),
                        ("invoice.prefix", "\"INV/\""),
                        ("inventory.issue_policy", "\"on_confirmation\""),
                        ("inventory.negative_stock", "\"block\""),
                        (FIRST_RUN_KEY, "true"),
                    ];
                    for (key, value) in defaults {
                        sqlx::query(
                            "INSERT OR IGNORE INTO settings (key, value_json, updated_by, updated_at)
                             VALUES (?, ?, ?, ?)",
                        )
                        .bind(key)
                        .bind(value)
                        .bind(user_id)
                        .bind(&now)
                        .execute(&mut *tx)
                            .await?;
                    }
                    sqlx::query(
                        "UPDATE locations
                            SET name = 'Furniture Showroom', type = 'showroom',
                                updated_at = ?
                          WHERE is_active = 1",
                    )
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(user_id),
                            action: "system.initial_administrator_created".into(),
                            entity_type: Some("user".into()),
                            entity_id: Some(user_id.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "username": INITIAL_ADMIN_USERNAME,
                                    "role": "owner"
                                })
                                .to_string(),
                            ),
                            ..Default::default()
                        },
                    )
                    .await?;

                Ok(true)
            })
        })
        .await
}

/// Create the owner, shop settings, canonical showroom, and the completion flag in
/// a single transaction. Idempotent: once any user exists the transaction
/// refuses to run, so a crash mid-setup can never create a second owner.
pub async fn complete(
    state: &AppState,
    input: &FirstRunInput,
    correlation_id: &str,
) -> Result<LoginResultDto, AppError> {
    let shop_name = input.shop_name.trim().to_string();
    if shop_name.is_empty() || shop_name.chars().count() > 100 {
        return Err(AppError::Validation(
            "shop name is required and must be 100 characters or fewer".into(),
        ));
    }
    let currency = input.currency.trim().to_uppercase();
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(AppError::Validation(
            "currency must be a 3-letter code (e.g. PKR)".into(),
        ));
    }
    let timezone = input.timezone.trim().to_string();
    if timezone.is_empty() || timezone.len() > 64 {
        return Err(AppError::Validation(
            "timezone is required and must be 64 characters or fewer".into(),
        ));
    }
    let invoice_prefix = input
        .invoice_prefix
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(prefix) = invoice_prefix {
        if !(1..=16).contains(&prefix.chars().count())
            || !prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'))
        {
            return Err(AppError::Validation(
                "invoice prefix must be 1-16 characters using letters, digits, '/', '-', '_' or '.'"
                    .into(),
            ));
        }
    }
    let backup_location = input
        .backup_location
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(dir) = backup_location {
        let path = std::path::Path::new(dir);
        if !path.is_absolute() {
            return Err(AppError::Validation(
                "backup location must be an absolute folder path".into(),
            ));
        }
        std::fs::create_dir_all(path)
            .map_err(|e| AppError::Validation(format!("backup location is not usable: {e}")))?;
    }
    // Owned copies so the write transaction never borrows the caller's input.
    let prefix_setting = invoice_prefix.map(str::to_string);
    let backup_setting = backup_location.map(str::to_string);
    let username = input.owner_username.trim().to_lowercase();
    if username.len() < 3
        || !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(AppError::Validation(
            "owner username must be 3+ characters using letters, digits, '.', '_' or '-'".into(),
        ));
    }
    let full_name = input.owner_full_name.trim().to_string();
    if full_name.is_empty() || full_name.chars().count() > 200 {
        return Err(AppError::Validation(
            "owner full name is required and must be 200 characters or fewer".into(),
        ));
    }
    password::validate_strength(&input.owner_password)?;
    let hash = password::hash_password(&input.owner_password)?;
    let now = state.clock.now_iso();

    let address = input.shop_address.trim().to_string();
    let phone = input.shop_phone.trim().to_string();
    let email = input.shop_email.trim().to_string();
    let audits = state.audits.clone();
    let correlation = correlation_id.to_string();

    let owner_id = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let shop_name = shop_name.clone();
            let username = username.clone();
            let full_name = full_name.clone();
            let currency = currency.clone();
            let timezone = timezone.clone();
            let hash = hash.clone();
            Box::pin(async move {
                let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
                    .fetch_one(&mut *tx)
                    .await?;
                if existing > 0 {
                    return Err(AppError::Conflict(
                        "first-run setup is already complete".into(),
                    ));
                }

                let owner_id = sqlx::query(
                    "INSERT INTO users (username, password_hash, full_name, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind(&username)
                .bind(&hash)
                .bind(&full_name)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                sqlx::query(
                    "INSERT INTO user_roles (user_id, role_id)
                     SELECT ?, id FROM roles WHERE code = 'owner'",
                )
                .bind(owner_id)
                .execute(&mut *tx)
                .await?;

                let settings: &[(&str, serde_json::Value)] = &[
                    ("shop.name", serde_json::json!(shop_name)),
                    ("shop.currency", serde_json::json!(currency)),
                    ("shop.timezone", serde_json::json!(timezone)),
                    (
                        "invoice.prefix",
                        serde_json::json!(prefix_setting.clone().unwrap_or_else(|| "INV/".into())),
                    ),
                    (
                        "inventory.issue_policy",
                        serde_json::json!("on_confirmation"),
                    ),
                    ("inventory.negative_stock", serde_json::json!("block")),
                    (FIRST_RUN_KEY, serde_json::json!(true)),
                ];
                for (key, value) in settings {
                    sqlx::query(
                        "INSERT INTO settings (key, value_json, updated_by, updated_at)
                         VALUES (?, ?, ?, ?)",
                    )
                    .bind(key)
                    .bind(value.to_string())
                    .bind(owner_id)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }
                if !address.is_empty() {
                    sqlx::query(
                        "INSERT INTO settings (key, value_json, updated_by, updated_at) VALUES ('shop.address', ?, ?, ?)",
                    )
                    .bind(serde_json::json!(address).to_string())
                    .bind(owner_id)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }
                if !phone.is_empty() {
                    sqlx::query(
                        "INSERT INTO settings (key, value_json, updated_by, updated_at) VALUES ('shop.phone', ?, ?, ?)",
                    )
                    .bind(serde_json::json!(phone).to_string())
                    .bind(owner_id)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }
                if !email.is_empty() {
                    sqlx::query(
                        "INSERT INTO settings (key, value_json, updated_by, updated_at) VALUES ('shop.email', ?, ?, ?)",
                    )
                    .bind(serde_json::json!(email).to_string())
                    .bind(owner_id)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }
                if let Some(dir) = backup_setting {
                    sqlx::query(
                        "INSERT INTO settings (key, value_json, updated_by, updated_at) VALUES ('backup.location', ?, ?, ?)",
                    )
                    .bind(serde_json::json!(dir).to_string())
                    .bind(owner_id)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }

                let location_id: i64 = sqlx::query_scalar(
                    "SELECT id FROM locations WHERE is_active = 1 ORDER BY id LIMIT 1",
                )
                .fetch_one(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE locations
                        SET name = '__retired_location_' || id, updated_at = ?
                      WHERE id <> ? AND name = ?",
                )
                .bind(&now)
                .bind(location_id)
                .bind(&shop_name)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE locations
                        SET name = ?, type = 'showroom', updated_at = ?
                      WHERE id = ?",
                )
                .bind(&shop_name)
                .bind(&now)
                .bind(location_id)
                .execute(&mut *tx)
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(owner_id),
                            action: "first_run.completed".into(),
                            entity_type: Some("system".into()),
                            after_json: Some(
                                serde_json::json!({ "shop": shop_name, "owner": username })
                                    .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;

                Ok(owner_id)
            })
        })
        .await?;

    // Create the session after the owner+settings transaction commits, on the
    // pool, so it never contends with the write lock held by `execute`.
    let session = state.sessions.create(owner_id).await?;
    let session_id = session.id;

    // Reuse the login profile path so the wizard can hand the user straight in.
    let principal = auth::resolve_session(state, &session_id).await?;
    Ok(LoginResultDto {
        session_id,
        profile: auth::profile_dto(&principal),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> FirstRunInput {
        FirstRunInput {
            shop_name: "Test Furniture".into(),
            shop_address: "12 Market Street".into(),
            shop_phone: "0300-1234567".into(),
            shop_email: "shop@example.com".into(),
            currency: "PKR".into(),
            timezone: "Asia/Karachi".into(),
            invoice_prefix: Some("TEST/".into()),
            backup_location: None,
            owner_username: "owner".into(),
            owner_full_name: "Owner One".into(),
            owner_password: "Owner Pass 123".into(),
        }
    }

    async fn fresh_state(label: &str) -> AppState {
        let dir = std::env::temp_dir().join(format!("furniture-shop-firstrun-{label}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        AppState::new(pool, paths)
    }

    #[tokio::test]
    async fn completes_owner_and_settings_in_single_transaction() {
        let state = fresh_state("ok").await;

        let result = complete(&state, &input(), "corr").await.unwrap();
        assert!(result.profile.roles.contains(&"owner".to_string()));

        let name: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'shop.name'")
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(name, Some(r#""Test Furniture""#.into()));

        let prefix: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'invoice.prefix'")
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(prefix, Some(r#""TEST/""#.into()));

        let completed: Option<String> = sqlx::query_scalar(
            "SELECT value_json FROM settings WHERE key = 'system.first_run_complete'",
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(completed, Some("true".into()));

        assert!(state.audits.verify_chain().await.unwrap().is_none());
        assert!(!state
            .sessions
            .get(&result.session_id)
            .await
            .unwrap()
            .unwrap()
            .locked_at
            .is_some());
    }

    #[tokio::test]
    async fn second_completion_is_rejected() {
        let state = fresh_state("idempotent").await;
        complete(&state, &input(), "corr").await.unwrap();
        let err = complete(&state, &input(), "corr").await.unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)));
        let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(users, 1);
    }

    #[tokio::test]
    async fn rejects_bad_invoice_prefix_and_relative_backup() {
        let state = fresh_state("validation").await;

        let mut bad = input();
        bad.invoice_prefix = Some("bad prefix spaces".into());
        assert!(matches!(
            complete(&state, &bad, "corr").await.unwrap_err(),
            AppError::Validation(_)
        ));

        let mut bad = input();
        bad.backup_location = Some("relative/backups".into());
        assert!(matches!(
            complete(&state, &bad, "corr").await.unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[tokio::test]
    async fn initial_administrator_login_survives_restart_without_password_reset() {
        let dir = std::env::temp_dir().join(format!(
            "furniture-shop-initial-admin-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let state = AppState::new(pool, paths);

        assert!(ensure_initial_administrator(&state).await.unwrap());
        let result = auth::login(&state, "admin", "admin123", "first-login")
            .await
            .unwrap();
        assert!(result.profile.roles.contains(&"owner".to_string()));

        let replacement = password::hash_password("Changed456").unwrap();
        sqlx::query("UPDATE users SET password_hash = ? WHERE username = 'admin'")
            .bind(replacement)
            .execute(&state.pool)
            .await
            .unwrap();
        state.pool.close().await;
        drop(state);

        // Reopening the same database models an application restart. The seed
        // must be a no-op and the changed password must remain authoritative.
        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let restarted = AppState::new(pool, paths);
        assert!(!ensure_initial_administrator(&restarted).await.unwrap());
        assert!(matches!(
            auth::login(&restarted, "admin", "admin123", "old-password")
                .await
                .unwrap_err(),
            AppError::InvalidCredentials
        ));
        assert!(
            auth::login(&restarted, "admin", "Changed456", "new-password")
                .await
                .is_ok()
        );

        let administrators: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
               FROM users u
               JOIN user_roles ur ON ur.user_id = u.id
               JOIN roles r ON r.id = ur.role_id
              WHERE r.code = 'owner'",
        )
        .fetch_one(&restarted.pool)
        .await
        .unwrap();
        assert_eq!(administrators, 1);

        restarted.pool.close().await;
        drop(restarted);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn existing_administrator_is_never_replaced() {
        let state = fresh_state("existing-admin").await;
        complete(&state, &input(), "corr").await.unwrap();

        assert!(!ensure_initial_administrator(&state).await.unwrap());
        let seeded_admins: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username = 'admin'")
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(seeded_admins, 0);
    }
}
