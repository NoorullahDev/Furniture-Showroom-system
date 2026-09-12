use crate::application::auth::Principal;
use crate::dto::RoleDto;
use crate::error::AppError;
use crate::infrastructure::AuditInput;
use crate::state::AppState;

/// List role templates with their current permission codes. Requires `user.manage`.
pub async fn list_roles(state: &AppState, principal: &Principal) -> Result<Vec<RoleDto>, AppError> {
    principal.require("user.manage")?;
    let roles: Vec<(i64, String, String, Option<String>, i64)> =
        sqlx::query_as("SELECT id, code, name, description, is_system FROM roles ORDER BY id")
            .fetch_all(&state.pool)
            .await?;

    let all_perms: Vec<(i64, String)> = sqlx::query_as(
        "SELECT rp.role_id, p.code FROM role_permissions rp
         JOIN permissions p ON p.id = rp.permission_id
         ORDER BY rp.role_id, p.code",
    )
    .fetch_all(&state.pool)
    .await?;

    use std::collections::HashMap;
    let mut perm_map: HashMap<i64, Vec<String>> = HashMap::new();
    for (role_id, code) in all_perms {
        perm_map.entry(role_id).or_default().push(code);
    }

    let mut out = Vec::with_capacity(roles.len());
    for (id, code, name, description, is_system) in roles {
        out.push(RoleDto {
            id,
            code,
            name,
            description,
            is_system: is_system != 0,
            permissions: perm_map.remove(&id).unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Replace the permission set of a role template. Requires `role.manage`.
pub async fn set_role_permissions(
    state: &AppState,
    principal: &Principal,
    role_id: i64,
    permission_codes: Vec<String>,
    correlation_id: &str,
) -> Result<RoleDto, AppError> {
    principal.require("role.manage")?;
    for code in &permission_codes {
        let found: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM permissions WHERE code = ?")
            .bind(code)
            .fetch_one(&state.pool)
            .await?;
        if found == 0 {
            return Err(AppError::Validation(format!("unknown permission `{code}`")));
        }
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let codes = permission_codes.clone();
    let resulting_permissions = permission_codes;

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let role: Option<String> =
                    sqlx::query_scalar("SELECT code FROM roles WHERE id = ?")
                        .bind(role_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                let Some(code) = role else {
                    return Err(AppError::NotFound(format!("role {role_id}")));
                };

                let previous: Vec<String> = sqlx::query_scalar(
                    "SELECT p.code FROM role_permissions rp
                     JOIN permissions p ON p.id = rp.permission_id
                     WHERE rp.role_id = ? ORDER BY p.code",
                )
                .bind(role_id)
                .fetch_all(&mut *tx)
                .await?;

                sqlx::query("DELETE FROM role_permissions WHERE role_id = ?")
                    .bind(role_id)
                    .execute(&mut *tx)
                    .await?;
                for pcode in &codes {
                    sqlx::query(
                        "INSERT INTO role_permissions (role_id, permission_id)
                         SELECT ?, id FROM permissions WHERE code = ?",
                    )
                    .bind(role_id)
                    .bind(pcode)
                    .execute(&mut *tx)
                    .await?;
                }

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "role.permissions_set".into(),
                            entity_type: Some("role".into()),
                            entity_id: Some(role_id.to_string()),
                            before_json: Some(
                                serde_json::json!({ "role": code.clone(), "permissions": previous })
                                    .to_string(),
                            ),
                            after_json: Some(
                                serde_json::json!({ "role": code, "permissions": codes })
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

    let (code, name, description, is_system): (String, String, Option<String>, i64) =
        sqlx::query_as(
            "SELECT code, name, description, is_system FROM roles WHERE id = ?",
        )
        .bind(role_id)
        .fetch_one(&state.pool)
        .await?;

    Ok(RoleDto {
        id: role_id,
        code,
        name,
        description,
        is_system: is_system != 0,
        permissions: resulting_permissions,
    })
}
