use crate::application::auth::Principal;
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::infrastructure::AuditInput;
use crate::repositories::SettingsRepository;
use crate::state::AppState;

/// Read one settings value as raw JSON text.
pub async fn get(state: &AppState, key: &str) -> Result<Option<String>, AppError> {
    SettingsRepository::find(&state.pool, key).await
}

/// Row-level setter used by internal flows that already own a transaction
/// (e.g. first-run setup).
#[allow(dead_code)]
pub async fn set(
    state: &AppState,
    key: &str,
    value_json: &str,
    updated_by: Option<i64>,
) -> Result<(), AppError> {
    let key_owned = key.to_owned();
    let value_owned = value_json.to_owned();
    let now = state.clock.now_iso();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                SettingsRepository::upsert(tx, &key_owned, &value_owned, updated_by, &now).await?;
                Ok::<(), AppError>(())
            })
        })
        .await
}

/// Authorized settings write used by commands: requires `settings.manage` and
/// records the change in the audit trail.
#[allow(clippy::too_many_arguments)]
pub async fn set_authorized(
    state: &AppState,
    principal: &Principal,
    key: &str,
    value_json: &str,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("settings.manage")?;
    let before = SettingsRepository::find(&state.pool, key).await?;
    let key_owned = key.to_owned();
    let value_owned = value_json.to_owned();
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                SettingsRepository::upsert(
                    &mut *tx,
                    &key_owned,
                    &value_owned,
                    Some(actor_id),
                    &now,
                )
                .await?;
                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "settings.update".into(),
                            entity_type: Some("setting".into()),
                            entity_id: Some(key_owned.clone()),
                            before_json: before,
                            after_json: Some(value_owned.clone()),
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
