use crate::application::auth::Principal;
use crate::dto::UserDto;
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::infrastructure::{password, AuditInput};
use crate::state::AppState;

fn normalize_username(username: &str) -> Result<String, AppError> {
    let clean = username.trim().to_lowercase();
    if !(3..=64).contains(&clean.len()) {
        return Err(AppError::Validation(
            "username must be between 3 and 64 characters".into(),
        ));
    }
    if !clean
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(AppError::Validation(
            "username may contain only letters, digits, '.', '_' and '-'".into(),
        ));
    }
    Ok(clean)
}

fn validate_full_name(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() || trimmed.chars().count() > 200 {
        return Err(AppError::Validation(
            "full name is required and must be 200 characters or fewer".into(),
        ));
    }
    Ok(trimmed)
}

/// Create a user with the given role codes. The creator must hold
/// `user.create`. Passwords never cross the DTO boundary.
pub async fn create_user(
    state: &AppState,
    principal: &Principal,
    username: &str,
    full_name: &str,
    password: &str,
    role_codes: &[String],
    correlation_id: &str,
) -> Result<UserDto, AppError> {
    principal.require("user.create")?;
    let username = normalize_username(username)?;
    let full_name = validate_full_name(full_name)?;
    password::validate_strength(password)?;
    if role_codes.is_empty() {
        return Err(AppError::Validation("at least one role is required".into()));
    }
    let roles_for_dto = role_codes.to_vec();
    let hash = password::hash_password(password)?;
    let now = state.clock.now_iso();
    let created_at = now.clone();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let dto_username = username.clone();
    let dto_full_name = full_name.clone();
    let dto_roles = roles_for_dto.clone();

    let created = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                for code in &roles_for_dto {
                    let found: i64 =
                        sqlx::query_scalar("SELECT COUNT(*) FROM roles WHERE code = ?")
                            .bind(code)
                            .fetch_one(&mut *tx)
                            .await?;
                    if found == 0 {
                        return Err(AppError::Validation(format!("unknown role `{code}`")));
                    }
                }

                let exists: Option<i64> =
                    sqlx::query_scalar("SELECT 1 FROM users WHERE username = ?")
                        .bind(&username)
                        .fetch_optional(&mut *tx)
                        .await?;
                if exists.is_some() {
                    return Err(AppError::Conflict(format!(
                        "a user named `{username}` already exists"
                    )));
                }

                let id = sqlx::query(
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
                let id_str = id.to_string();

                for code in &roles_for_dto {
                    sqlx::query(
                        "INSERT INTO user_roles (user_id, role_id)
                         SELECT ?, id FROM roles WHERE code = ?",
                    )
                    .bind(id)
                    .bind(code)
                    .execute(&mut *tx)
                    .await?;
                }

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "user.create".into(),
                            entity_type: Some("user".into()),
                            entity_id: Some(id_str.clone()),
                            after_json: Some(
                                serde_json::json!({
                                    "username": username,
                                    "full_name": full_name,
                                    "roles": roles_for_dto,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;

                Ok(id)
            })
        })
        .await?;

    Ok(UserDto {
        id: created,
        username: dto_username,
        full_name: dto_full_name,
        is_active: true,
        roles: dto_roles,
        created_at,
    })
}

/// Update display name, active flag, and role membership; optionally reset the
/// password. Requires `user.edit`.
#[allow(clippy::too_many_arguments)]
pub async fn update_user(
    state: &AppState,
    principal: &Principal,
    user_id: i64,
    full_name: Option<String>,
    is_active: Option<bool>,
    role_codes: Option<Vec<String>>,
    new_password: Option<String>,
    correlation_id: &str,
) -> Result<UserDto, AppError> {
    principal.require("user.edit")?;
    if user_id == principal.user_id && is_active == Some(false) {
        return Err(AppError::Validation(
            "you cannot deactivate your own account".into(),
        ));
    }
    if role_codes.as_deref().is_some_and(|codes| codes.is_empty()) {
        return Err(AppError::Validation("at least one role is required".into()));
    }

    let current = load_user(state, user_id).await?;
    let full_name = match &full_name {
        Some(name) => validate_full_name(name)?,
        None => current.1.clone(),
    };
    if let Some(pw) = &new_password {
        password::validate_strength(pw)?;
    }
    let hash = match &new_password {
        Some(pw) => Some(password::hash_password(pw)?),
        None => None,
    };
    let roles_for_dto = role_codes.clone().unwrap_or_else(|| current.3.clone());
    let before_json = serde_json::json!({
        "username": current.0,
        "full_name": current.1,
        "is_active": current.2,
        "roles": current.3,
    })
    .to_string();
    let full_name_for_dto = full_name.clone();
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let audits = state.audits.clone();
    let correlation = correlation_id.to_string();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let full_name = full_name.clone();
            let role_codes = role_codes.clone().unwrap_or_default();
            let hash = hash.clone();
            Box::pin(async move {
                for code in &role_codes {
                    let found: i64 =
                        sqlx::query_scalar("SELECT COUNT(*) FROM roles WHERE code = ?")
                            .bind(code)
                            .fetch_one(&mut *tx)
                            .await?;
                    if found == 0 {
                        return Err(AppError::Validation(format!("unknown role `{code}`")));
                    }
                }

                sqlx::query("UPDATE users SET full_name = ?, updated_at = ? WHERE id = ?")
                    .bind(&full_name)
                    .bind(&now)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;

                if let Some(active) = is_active {
                    sqlx::query("UPDATE users SET is_active = ?, updated_at = ? WHERE id = ?")
                        .bind(active as i64)
                        .bind(&now)
                        .bind(user_id)
                        .execute(&mut *tx)
                        .await?;
                    if !active {
                        sqlx::query(
                            "UPDATE sessions SET active = 0 WHERE user_id = ? AND active = 1",
                        )
                        .bind(user_id)
                        .execute(&mut *tx)
                        .await?;
                    }
                }

                if let Some(hash) = &hash {
                    sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?")
                        .bind(hash)
                        .bind(&now)
                        .bind(user_id)
                        .execute(&mut *tx)
                        .await?;
                    sqlx::query("UPDATE sessions SET active = 0 WHERE user_id = ? AND active = 1")
                        .bind(user_id)
                        .execute(&mut *tx)
                        .await?;
                }

                if !role_codes.is_empty() {
                    sqlx::query("DELETE FROM user_roles WHERE user_id = ?")
                        .bind(user_id)
                        .execute(&mut *tx)
                        .await?;
                    for code in &role_codes {
                        sqlx::query(
                            "INSERT INTO user_roles (user_id, role_id)
                             SELECT ?, id FROM roles WHERE code = ?",
                        )
                        .bind(user_id)
                        .bind(code)
                        .execute(&mut *tx)
                        .await?;
                    }
                }

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "user.update".into(),
                            entity_type: Some("user".into()),
                            entity_id: Some(user_id.to_string()),
                            before_json: Some(before_json),
                            after_json: Some(
                                serde_json::json!({
                                    "full_name": full_name,
                                    "is_active": is_active,
                                    "password_changed": hash.is_some(),
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

    Ok(UserDto {
        id: user_id,
        username: current.0,
        full_name: full_name_for_dto,
        is_active: is_active.unwrap_or(current.2),
        roles: roles_for_dto,
        created_at: String::new(),
    })
}

/// List users (name, active, roles). Requires `user.manage`.
pub async fn list_users(state: &AppState, principal: &Principal) -> Result<Vec<UserDto>, AppError> {
    principal.require("user.manage")?;
    let users: Vec<(i64, String, String, i64, String)> = sqlx::query_as(
        "SELECT id, username, full_name, is_active, created_at FROM users ORDER BY username",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut out = Vec::with_capacity(users.len());
    for (id, username, full_name, is_active, created_at) in users {
        let roles = load_roles(state, id).await?;
        out.push(UserDto {
            id,
            username,
            full_name,
            is_active: is_active != 0,
            roles,
            created_at,
        });
    }
    Ok(out)
}

/// Soft-delete: deactivate the account and revoke every live session.
/// Requires `user.deactivate`.
pub async fn deactivate_user(
    state: &AppState,
    principal: &Principal,
    user_id: i64,
    reason: &str,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("user.deactivate")?;
    if user_id == principal.user_id {
        return Err(AppError::Validation(
            "you cannot deactivate your own account".into(),
        ));
    }

    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let reason_owned = reason.to_string();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let before: Option<(String, i64)> =
                    sqlx::query_as("SELECT username, is_active FROM users WHERE id = ?")
                        .bind(user_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                let Some((username, was_active)) = before else {
                    return Err(AppError::NotFound(format!("user {user_id}")));
                };
                if was_active == 0 {
                    return Err(AppError::Conflict("user is already deactivated".into()));
                }

                sqlx::query("UPDATE users SET is_active = 0, updated_at = ? WHERE id = ?")
                    .bind(&now)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE sessions SET active = 0 WHERE user_id = ? AND active = 1")
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "user.deactivate".into(),
                            entity_type: Some("user".into()),
                            entity_id: Some(user_id.to_string()),
                            before_json: Some(format!("{{\"username\":\"{username}\"}}")),
                            reason: Some(reason_owned),
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

/// Reset a password to a known value (owner-only flow). Revokes the target's
/// live sessions. Requires `user.reset_password`.
pub async fn reset_password(
    state: &AppState,
    principal: &Principal,
    user_id: i64,
    new_password: &str,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("user.reset_password")?;
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
                let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM users WHERE id = ?")
                    .bind(user_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                if exists.is_none() {
                    return Err(AppError::NotFound(format!("user {user_id}")));
                }
                sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?")
                    .bind(&hash)
                    .bind(&now)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE sessions SET active = 0 WHERE user_id = ? AND active = 1")
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "user.password_reset".into(),
                            entity_type: Some("user".into()),
                            entity_id: Some(user_id.to_string()),
                            after_json: Some(serde_json::json!({"password": "reset"}).to_string()),
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

async fn load_user(
    state: &AppState,
    user_id: i64,
) -> Result<(String, String, bool, Vec<String>), AppError> {
    let row: Option<(String, String, i64)> =
        sqlx::query_as("SELECT username, full_name, is_active FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some((username, full_name, is_active)) = row else {
        return Err(AppError::NotFound(format!("user {user_id}")));
    };
    let roles = load_roles(state, user_id).await?;
    Ok((username, full_name, is_active != 0, roles))
}

async fn load_roles(state: &AppState, user_id: i64) -> Result<Vec<String>, AppError> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT r.code FROM user_roles ur JOIN roles r ON r.id = ur.role_id
         WHERE ur.user_id = ? ORDER BY r.code",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows)
}
