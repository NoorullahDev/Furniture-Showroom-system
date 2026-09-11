use crate::dto::{LoginResultDto, SessionProfileDto};
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::infrastructure::password;
use crate::state::AppState;

/// Precomputed Argon2id hash used to equalize timing for unknown usernames.
static DUMMY_HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn dummy_hash() -> &'static str {
    DUMMY_HASH
        .get_or_init(|| password::hash_password("dummy-timing-1").expect("hash"))
        .as_str()
}

/// Authenticated identity resolved for one command invocation. Built only from
/// safe columns — the password hash never leaves the users table.
#[derive(Debug)]
pub struct Principal {
    pub session_id: String,
    pub user_id: i64,
    pub username: String,
    pub full_name: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

impl Principal {
    pub fn require(&self, permission: &str) -> Result<(), AppError> {
        if self.permissions.iter().any(|p| p == permission) {
            Ok(())
        } else {
            Err(AppError::Unauthorized(format!(
                "user lacks `{permission}` permission"
            )))
        }
    }

    /// Pass if the caller holds at least one of the given permissions.
    pub fn require_any(&self, permissions: &[&str]) -> Result<(), AppError> {
        if permissions
            .iter()
            .any(|p| self.permissions.iter().any(|held| held == p))
        {
            Ok(())
        } else {
            Err(AppError::Unauthorized(format!(
                "user lacks any of the permissions {permissions:?}"
            )))
        }
    }

    pub fn is_owner(&self) -> bool {
        self.roles.iter().any(|r| r == "owner")
    }
}

pub fn profile_dto(principal: &Principal) -> SessionProfileDto {
    SessionProfileDto {
        session_id: principal.session_id.clone(),
        user_id: principal.user_id,
        username: principal.username.clone(),
        full_name: principal.full_name.clone(),
        roles: principal.roles.clone(),
        permissions: principal.permissions.clone(),
        locked_at: None,
    }
}

/// Session state for the shell: `Ok(None)` when there is no usable session,
/// and a profile with `lockedAt` set when the session exists but is locked.
pub async fn current(
    state: &AppState,
    session_id: &str,
) -> Result<Option<SessionProfileDto>, AppError> {
    let Some(session) = state.sessions.get(session_id).await? else {
        return Ok(None);
    };
    if !session.active {
        return Ok(None);
    }
    let locked = session.locked_at.is_some();

    let user: Option<(String, String, i64)> =
        sqlx::query_as("SELECT username, full_name, is_active FROM users WHERE id = ?")
            .bind(session.user_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some((username, full_name, is_active)) = user else {
        return Ok(None);
    };
    if is_active == 0 {
        return Ok(None);
    }

    if !locked {
        state.sessions.touch(session_id).await?;
    }

    Ok(Some(SessionProfileDto {
        session_id: session_id.to_string(),
        user_id: session.user_id,
        username,
        full_name,
        roles: principal_roles(state, session.user_id).await?,
        permissions: principal_permissions(state, session.user_id).await?,
        locked_at: session.locked_at,
    }))
}

fn normalize_username(username: &str) -> String {
    username.trim().to_lowercase()
}

/// Resolve a principal for an active, unlocked session. Also refreshes
/// `last_used_at`. This is the single gate every protected command must pass.
pub async fn resolve_session(state: &AppState, session_id: &str) -> Result<Principal, AppError> {
    let session = state
        .sessions
        .get(session_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("session is not active".into()))?;
    if !session.active {
        return Err(AppError::Unauthorized("session is not active".into()));
    }
    if session.locked_at.is_some() {
        return Err(AppError::SessionLocked(
            "unlock this session before continuing".into(),
        ));
    }

    let (username, full_name, is_active): (String, String, i64) =
        sqlx::query_as("SELECT username, full_name, is_active FROM users WHERE id = ?")
            .bind(session.user_id)
            .fetch_one(&state.pool)
            .await?;
    if is_active == 0 {
        return Err(AppError::Unauthorized("user is deactivated".into()));
    }

    let roles: Vec<String> =
            sqlx::query_scalar("SELECT r.code FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = ? ORDER BY r.code")
            .bind(session.user_id)
            .fetch_all(&state.pool)
            .await?;

    let permissions: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT p.code
           FROM user_roles ur
           JOIN role_permissions rp ON rp.role_id = ur.role_id
           JOIN permissions p ON p.id = rp.permission_id
          WHERE ur.user_id = ?
          ORDER BY p.code",
    )
    .bind(session.user_id)
    .fetch_all(&state.pool)
    .await?;

    // Touch the session so the inactivity tracker sees the activity.
    state.sessions.touch(session_id).await?;

    Ok(Principal {
        session_id: session_id.to_string(),
        user_id: session.user_id,
        username,
        full_name,
        roles,
        permissions,
    })
}

pub async fn login(
    state: &AppState,
    username: &str,
    password: &str,
    correlation_id: &str,
) -> Result<LoginResultDto, AppError> {
    let key = normalize_username(username);

    let throttle = &state.throttle;
    if let Some(remaining) = throttle.remaining_lockout(&key) {
        return Err(AppError::RateLimited(
            i64::try_from(remaining.min(3600)).unwrap_or(3600),
        ));
    }

    let user: Option<(i64, String, String, i64)> = sqlx::query_as(
        "SELECT id, password_hash, full_name, is_active FROM users WHERE username = ?",
    )
    .bind(&key)
    .fetch_optional(&state.pool)
    .await?;

    let profile = match user {
        Some((user_id, stored_hash, full_name, is_active))
            if is_active != 0 && password::verify_password(password, &stored_hash) =>
        {
            throttle.record_success(&key);
            let session = state.sessions.create(user_id).await?;
            state
                .audits
                .record_pool(crate::infrastructure::AuditInput {
                    user_id: Some(user_id),
                    session_id: Some(session.id.clone()),
                    action: "auth.login".into(),
                    entity_type: Some("user".into()),
                    entity_id: Some(user_id.to_string()),
                    correlation_id: Some(correlation_id.to_string()),
                    ..Default::default()
                })
                .await?;

            let roles = principal_roles(state, user_id).await?;
            let permissions = principal_permissions(state, user_id).await?;
            Ok(LoginResultDto {
                session_id: session.id.clone(),
                profile: SessionProfileDto {
                    session_id: session.id,
                    user_id,
                    username: key.clone(),
                    full_name,
                    roles,
                    permissions,
                    locked_at: None,
                },
            })
        }
        Some((_, stored_hash, _, _)) => {
            // Same round-trip as a successful verify for constant-time-ish
            // behavior; the failure path below also burns one hash.
            let _ = password::verify_password(password, &stored_hash);
            Err(AppError::InvalidCredentials)
        }
        None => {
            let _ = password::verify_password(password, dummy_hash());
            Err(AppError::InvalidCredentials)
        }
    };

    if profile.is_err() {
        throttle.record_failure(&key);
        state
            .audits
            .record_pool(crate::infrastructure::AuditInput {
                user_id: None,
                action: "auth.login_failed".into(),
                entity_type: Some("user".into()),
                entity_id: Some(key),
                reason: Some("invalid credentials".into()),
                correlation_id: Some(correlation_id.to_string()),
                ..Default::default()
            })
            .await?;
    }
    profile
}

pub async fn logout(state: &AppState, session_id: &str) -> Result<(), AppError> {
    if let Some(session) = state.sessions.get(session_id).await? {
        state.sessions.deactivate(session_id).await?;
        state
            .audits
            .record_pool(crate::infrastructure::AuditInput {
                user_id: Some(session.user_id),
                session_id: Some(session_id.to_string()),
                action: "auth.logout".into(),
                entity_type: Some("session".into()),
                entity_id: Some(session_id.to_string()),
                ..Default::default()
            })
            .await?;
    }
    Ok(())
}

pub async fn lock_session(state: &AppState, session_id: &str) -> Result<(), AppError> {
    if let Some(session) = state.sessions.get(session_id).await? {
        state.sessions.lock(session_id).await?;
        state
            .audits
            .record_pool(crate::infrastructure::AuditInput {
                user_id: Some(session.user_id),
                session_id: Some(session_id.to_string()),
                action: "auth.session_lock".into(),
                entity_type: Some("session".into()),
                entity_id: Some(session_id.to_string()),
                ..Default::default()
            })
            .await?;
    }
    Ok(())
}

pub async fn unlock_session(
    state: &AppState,
    session_id: &str,
    password: &str,
) -> Result<(), AppError> {
    // Unlock must NOT go through `resolve_session`: that gate rejects locked
    // sessions. Read the session directly and verify the user is still usable.
    let session = state
        .sessions
        .get(session_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("session is not active".into()))?;
    if !session.active {
        return Err(AppError::Unauthorized("session is not active".into()));
    }

    let (stored, is_active): (String, i64) =
        sqlx::query_as("SELECT password_hash, is_active FROM users WHERE id = ?")
            .bind(session.user_id)
            .fetch_one(&state.pool)
            .await?;
    if is_active == 0 {
        return Err(AppError::Unauthorized("user is deactivated".into()));
    }
    if !password::verify_password(password, &stored) {
        return Err(AppError::InvalidCredentials);
    }
    state.sessions.unlock(session_id).await?;
    state
        .audits
        .record_pool(crate::infrastructure::AuditInput {
            user_id: Some(session.user_id),
            session_id: Some(session_id.to_string()),
            action: "auth.unlock".into(),
            entity_type: Some("session".into()),
            entity_id: Some(session_id.to_string()),
            ..Default::default()
        })
        .await?;
    Ok(())
}

pub async fn change_password(
    state: &AppState,
    principal: &Principal,
    current: &str,
    new_password: &str,
    correlation_id: &str,
) -> Result<(), AppError> {
    let stored: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(principal.user_id)
        .fetch_one(&state.pool)
        .await?;
    if !password::verify_password(current, &stored) {
        return Err(AppError::InvalidCredentials);
    }
    password::validate_strength(new_password)?;
    let hash = password::hash_password(new_password)?;
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?")
                    .bind(&hash)
                    .bind(&now)
                    .bind(actor_id)
                    .execute(&mut *tx)
                    .await?;
                audits
                    .record(
                        &mut *tx,
                        crate::infrastructure::AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "user.password_change".into(),
                            entity_type: Some("user".into()),
                            entity_id: Some(actor_id.to_string()),
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

/// Atomically update the signed-in owner's username and/or password after
/// verifying the current password. The user row is updated in place and only
/// this account's sessions are revoked.
pub async fn update_login_details(
    state: &AppState,
    principal: &Principal,
    current_password: &str,
    new_username: Option<&str>,
    new_password: Option<&str>,
    confirm_password: Option<&str>,
    correlation_id: &str,
) -> Result<String, AppError> {
    if !principal.is_owner() {
        return Err(AppError::Unauthorized(
            "only an owner can change owner login details".into(),
        ));
    }
    if current_password.is_empty() {
        return Err(AppError::Validation("current password is required".into()));
    }

    let username = match new_username.map(str::trim).filter(|v| !v.is_empty()) {
        Some(value) => {
            let clean = normalize_username(value);
            if !(3..=64).contains(&clean.len())
                || !clean
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            {
                return Err(AppError::Validation(
                    "username must be 3-64 characters using letters, digits, '.', '_' or '-'"
                        .into(),
                ));
            }
            Some(clean)
        }
        None => None,
    };

    let password_change = match (new_password, confirm_password) {
        (None, None) => None,
        (Some(""), Some("")) => None,
        (Some(next), Some(confirm)) if next == confirm => {
            password::validate_strength(next)?;
            Some(password::hash_password(next)?)
        }
        _ => {
            return Err(AppError::Validation(
                "new password and confirmation must match".into(),
            ))
        }
    };
    if username.is_none() && password_change.is_none() {
        return Err(AppError::Validation(
            "provide a new username, a new password, or both".into(),
        ));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let current_password = current_password.to_string();
    let now = state.clock.now_iso();
    let audits = state.audits.clone();
    let correlation = correlation_id.to_string();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let (old_username, stored_hash): (String, String) = sqlx::query_as(
                    "SELECT username, password_hash FROM users WHERE id = ? AND is_active = 1",
                )
                .bind(actor_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::Unauthorized("account is not active".into()))?;

                if !password::verify_password(&current_password, &stored_hash) {
                    return Err(AppError::InvalidCredentials);
                }

                let final_username = username.unwrap_or_else(|| old_username.clone());
                let password_changed = password_change.is_some();
                if final_username == old_username && !password_changed {
                    return Err(AppError::Validation(
                        "provide a different username, a new password, or both".into(),
                    ));
                }
                if final_username != old_username {
                    let duplicate: Option<i64> =
                        sqlx::query_scalar("SELECT id FROM users WHERE username = ? AND id <> ?")
                            .bind(&final_username)
                            .bind(actor_id)
                            .fetch_optional(&mut *tx)
                            .await?;
                    if duplicate.is_some() {
                        return Err(AppError::Conflict(format!(
                            "a user named `{final_username}` already exists"
                        )));
                    }
                }

                let final_hash = password_change.unwrap_or(stored_hash);
                sqlx::query(
                    "UPDATE users SET username = ?, password_hash = ?, updated_at = ? WHERE id = ?",
                )
                .bind(&final_username)
                .bind(&final_hash)
                .bind(&now)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?;

                sqlx::query("UPDATE sessions SET active = 0 WHERE user_id = ? AND active = 1")
                    .bind(actor_id)
                    .execute(&mut *tx)
                    .await?;

                audits
                    .record(
                        &mut *tx,
                        crate::infrastructure::AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "user.login_details_change".into(),
                            entity_type: Some("user".into()),
                            entity_id: Some(actor_id.to_string()),
                            before_json: Some(
                                serde_json::json!({ "username": old_username }).to_string(),
                            ),
                            after_json: Some(
                                serde_json::json!({
                                    "username": final_username,
                                    "password_changed": password_changed
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;

                Ok(final_username)
            })
        })
        .await
}

/// Roles for a user id (used by login and management views).
async fn principal_roles(state: &AppState, user_id: i64) -> Result<Vec<String>, AppError> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT r.code FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = ? ORDER BY r.code",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows)
}

async fn principal_permissions(state: &AppState, user_id: i64) -> Result<Vec<String>, AppError> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT p.code
           FROM user_roles ur
           JOIN role_permissions rp ON rp.role_id = ur.role_id
           JOIN permissions p ON p.id = rp.permission_id
          WHERE ur.user_id = ? ORDER BY p.code",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::password::hash_password;

    pub async fn seed_owner(state: &AppState) -> i64 {
        let hash = hash_password("Owner Pass 123").unwrap();
        let id: i64 = sqlx::query(
            "INSERT INTO users (username, password_hash, full_name) VALUES ('owner', ?, 'Owner')",
        )
        .bind(&hash)
        .execute(&state.pool)
        .await
        .unwrap()
        .last_insert_rowid();
        let role: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE code = 'owner'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
            .bind(id)
            .bind(role)
            .execute(&state.pool)
            .await
            .unwrap();
        id
    }

    #[tokio::test]
    async fn login_rejects_wrong_password_and_audits() {
        let dir = std::env::temp_dir().join("furniture-shop-auth-wrong");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let state = AppState::new(pool, paths);

        seed_owner(&state).await;
        let err = login(&state, "owner", "wrong-pass", "corr")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::InvalidCredentials));

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_logs WHERE action = 'auth.login_failed'",
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
        assert!(state.audits.verify_chain().await.unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn login_succeeds_and_resolves_principal_with_permissions() {
        let dir = std::env::temp_dir().join("furniture-shop-auth-ok");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let state = AppState::new(pool, paths);

        seed_owner(&state).await;
        let result = login(&state, "Owner", "Owner Pass 123", "corr")
            .await
            .expect("login");
        assert!(!result.session_id.is_empty());

        let principal = resolve_session(&state, &result.session_id).await.unwrap();
        assert_eq!(principal.username, "owner");
        assert!(principal.roles.contains(&"owner".to_string()));
        assert!(principal.permissions.iter().any(|p| p == "user.create"));
        assert!(principal.permissions.iter().any(|p| p == "role.manage"));
        assert!(principal.require("user.manage").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn locked_session_is_rejected_by_resolve() {
        let dir = std::env::temp_dir().join("furniture-shop-auth-lock");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let state = AppState::new(pool, paths);

        let user_id = seed_owner(&state).await;
        let session = state.sessions.create(user_id).await.unwrap();
        state.sessions.lock(&session.id).await.unwrap();
        let err = resolve_session(&state, &session.id).await.unwrap_err();
        assert!(matches!(err, AppError::SessionLocked(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn locked_session_unlocks_only_with_correct_password() {
        let dir = std::env::temp_dir().join("furniture-shop-auth-unlock");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let state = AppState::new(pool, paths);

        let user_id = seed_owner(&state).await;
        let session = state.sessions.create(user_id).await.unwrap();
        state.sessions.lock(&session.id).await.unwrap();

        let err = unlock_session(&state, &session.id, "wrong-pass")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::InvalidCredentials));
        assert!(matches!(
            resolve_session(&state, &session.id).await.unwrap_err(),
            AppError::SessionLocked(_)
        ));

        unlock_session(&state, &session.id, "Owner Pass 123")
            .await
            .unwrap();
        assert!(resolve_session(&state, &session.id).await.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn owner_login_details_update_is_atomic_and_revokes_only_that_owner() {
        let dir = std::env::temp_dir().join(format!(
            "furniture-shop-auth-self-update-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let state = AppState::new(pool, paths);

        let owner_id = seed_owner(&state).await;
        let other_hash = hash_password("Other Pass 123").unwrap();
        let other_id = sqlx::query(
            "INSERT INTO users (username, password_hash, full_name) VALUES ('taken', ?, 'Other')",
        )
        .bind(other_hash)
        .execute(&state.pool)
        .await
        .unwrap()
        .last_insert_rowid();
        let other_session = state.sessions.create(other_id).await.unwrap();

        let owner_session = state.sessions.create(owner_id).await.unwrap();
        let owner = resolve_session(&state, &owner_session.id).await.unwrap();

        let wrong = update_login_details(
            &state,
            &owner,
            "wrong",
            Some("renamed"),
            None,
            None,
            "wrong-current",
        )
        .await
        .unwrap_err();
        assert!(matches!(wrong, AppError::InvalidCredentials));

        let mismatch = update_login_details(
            &state,
            &owner,
            "Owner Pass 123",
            None,
            Some("New Pass 456"),
            Some("different"),
            "mismatch",
        )
        .await
        .unwrap_err();
        assert!(matches!(mismatch, AppError::Validation(_)));

        let duplicate = update_login_details(
            &state,
            &owner,
            "Owner Pass 123",
            Some("taken"),
            None,
            None,
            "duplicate",
        )
        .await
        .unwrap_err();
        assert!(matches!(duplicate, AppError::Conflict(_)));
        assert!(state.sessions.is_active(&owner_session.id).await.unwrap());

        update_login_details(
            &state,
            &owner,
            "Owner Pass 123",
            Some("renamed"),
            None,
            None,
            "username-only",
        )
        .await
        .unwrap();
        assert!(!state.sessions.is_active(&owner_session.id).await.unwrap());
        assert!(state.sessions.is_active(&other_session.id).await.unwrap());
        assert!(login(&state, "owner", "Owner Pass 123", "old-user")
            .await
            .is_err());

        let renamed_login = login(&state, "renamed", "Owner Pass 123", "renamed-login")
            .await
            .unwrap();
        let renamed = resolve_session(&state, &renamed_login.session_id)
            .await
            .unwrap();
        update_login_details(
            &state,
            &renamed,
            "Owner Pass 123",
            None,
            Some("New Pass 456"),
            Some("New Pass 456"),
            "password-only",
        )
        .await
        .unwrap();
        assert!(login(&state, "renamed", "Owner Pass 123", "old-password")
            .await
            .is_err());

        let new_password_login = login(&state, "renamed", "New Pass 456", "new-password")
            .await
            .unwrap();
        let renamed = resolve_session(&state, &new_password_login.session_id)
            .await
            .unwrap();
        update_login_details(
            &state,
            &renamed,
            "New Pass 456",
            Some("final-owner"),
            Some("Final Pass 789"),
            Some("Final Pass 789"),
            "combined",
        )
        .await
        .unwrap();

        assert!(login(&state, "renamed", "New Pass 456", "replaced")
            .await
            .is_err());
        let final_login = login(&state, "final-owner", "Final Pass 789", "final")
            .await
            .unwrap();
        assert_eq!(final_login.profile.user_id, owner_id);
        assert!(final_login.profile.roles.contains(&"owner".to_string()));
        assert!(state.sessions.is_active(&other_session.id).await.unwrap());

        state.pool.close().await;
        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let restarted = AppState::new(pool, paths);
        assert!(
            login(&restarted, "admin", "admin123", "bootstrap-must-not-reset")
                .await
                .is_err()
        );
        assert!(
            login(&restarted, "renamed", "New Pass 456", "old-after-restart")
                .await
                .is_err()
        );
        let after_restart = login(
            &restarted,
            "final-owner",
            "Final Pass 789",
            "new-after-restart",
        )
        .await
        .unwrap();
        assert_eq!(after_restart.profile.user_id, owner_id);
        restarted.pool.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
