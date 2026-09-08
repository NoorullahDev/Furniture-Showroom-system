use crate::application::auth::Principal;
use crate::dto::{redacted_json, AuditEventDto, AuditPageDto};
use crate::error::AppError;
use crate::state::AppState;

pub struct AuditFilter {
    pub action: Option<String>,
    pub entity_type: Option<String>,
    pub user_id: Option<i64>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

/// Filter clause: each optional filter is bound twice, once as a NULL probe on
/// the left so the comparison short-circuits when the filter is absent.
const FILTER_CLAUSE: &str = "
  WHERE ( ? IS NULL OR action = ? )
    AND ( ? IS NULL OR entity_type = ? )
    AND ( ? IS NULL OR user_id = ? )
    AND ( ? IS NULL OR created_at >= ? )
    AND ( ? IS NULL OR created_at <= ? )";

type AuditQueryRow = (
    i64,
    Option<i64>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);

/// Read-only, filtered view of the audit trail. Requires `audit.view`.
pub async fn query_audit(
    state: &AppState,
    principal: &Principal,
    filter: &AuditFilter,
) -> Result<AuditPageDto, AppError> {
    principal.require("audit.view")?;
    let limit = i64::from(filter.limit.clamp(1, 500));
    let offset = i64::from(filter.offset);

    let count_sql = format!("SELECT COUNT(*) FROM audit_logs {FILTER_CLAUSE}");
    let total: i64 = sqlx::query_scalar::<_, i64>(&count_sql)
        .bind(filter.action.clone())
        .bind(filter.action.clone())
        .bind(filter.entity_type.clone())
        .bind(filter.entity_type.clone())
        .bind(filter.user_id)
        .bind(filter.user_id)
        .bind(filter.from.clone())
        .bind(filter.from.clone())
        .bind(filter.to.clone())
        .bind(filter.to.clone())
        .fetch_one(&state.pool)
        .await?;

    let rows_sql = format!(
        "SELECT id, user_id, action, entity_type, entity_id, reason, before_json,
                after_json, approval_user_id, session_id, app_version,
                correlation_id, prev_hash, created_at
           FROM audit_logs {FILTER_CLAUSE}
          ORDER BY id DESC LIMIT ? OFFSET ?"
    );
    let rows: Vec<AuditQueryRow> = sqlx::query_as(&rows_sql)
        .bind(filter.action.clone())
        .bind(filter.action.clone())
        .bind(filter.entity_type.clone())
        .bind(filter.entity_type.clone())
        .bind(filter.user_id)
        .bind(filter.user_id)
        .bind(filter.from.clone())
        .bind(filter.from.clone())
        .bind(filter.to.clone())
        .bind(filter.to.clone())
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;

    let items = rows
        .into_iter()
        .map(
            |(
                id,
                user_id,
                action,
                entity_type,
                entity_id,
                reason,
                before_json,
                after_json,
                approval_user_id,
                session_id,
                app_version,
                correlation_id,
                prev_hash,
                created_at,
            )| {
                AuditEventDto {
                    id,
                    user_id,
                    action,
                    entity_type,
                    entity_id,
                    reason,
                    before_json: before_json.map(|v| redacted_json(&v)),
                    after_json: after_json.map(|v| redacted_json(&v)),
                    approval_user_id,
                    session_id,
                    app_version,
                    correlation_id,
                    prev_hash,
                    created_at,
                }
            },
        )
        .collect();

    Ok(AuditPageDto { total, items })
}
