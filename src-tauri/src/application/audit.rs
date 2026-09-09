use crate::application::auth::Principal;
use crate::dto::{redacted_json, AuditEventDto, AuditPageDto};
use crate::error::AppError;
use crate::state::AppState;

pub struct AuditFilter {
    pub action: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
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
    AND ( ? IS NULL OR entity_id = ? )
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

/// Normalize a date filter bound. A bare wall-clock date (`YYYY-MM-DD`, as
/// picked by the audit viewer) is expanded to a full RFC3339 instant covering
/// that whole day; anything else passes through unchanged.
fn normalize_bound(value: &str, end_of_day: bool) -> Option<String> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if v.len() == 10 && v.chars().all(|c| c.is_ascii_digit() || c == '-') {
        let suffix = if end_of_day {
            "T23:59:59.999Z"
        } else {
            "T00:00:00.000Z"
        };
        return Some(format!("{v}{suffix}"));
    }
    Some(v.to_string())
}

/// Read-only, filtered view of the audit trail. Requires `audit.view`.
pub async fn query_audit(
    state: &AppState,
    principal: &Principal,
    filter: &AuditFilter,
) -> Result<AuditPageDto, AppError> {
    principal.require("audit.view")?;
    let limit = i64::from(filter.limit.clamp(1, 500));
    let offset = i64::from(filter.offset);
    let from = normalize_bound(filter.from.as_deref().unwrap_or(""), false);
    let to = normalize_bound(filter.to.as_deref().unwrap_or(""), true);

    let count_sql = format!("SELECT COUNT(*) FROM audit_logs {FILTER_CLAUSE}");
    let total: i64 = sqlx::query_scalar::<_, i64>(&count_sql)
        .bind(filter.action.clone())
        .bind(filter.action.clone())
        .bind(filter.entity_type.clone())
        .bind(filter.entity_type.clone())
        .bind(filter.entity_id.clone())
        .bind(filter.entity_id.clone())
        .bind(filter.user_id)
        .bind(filter.user_id)
        .bind(from.clone())
        .bind(from.clone())
        .bind(to.clone())
        .bind(to.clone())
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
        .bind(filter.entity_id.clone())
        .bind(filter.entity_id.clone())
        .bind(filter.user_id)
        .bind(filter.user_id)
        .bind(from.clone())
        .bind(from.clone())
        .bind(to.clone())
        .bind(to.clone())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_bounds_expand_to_full_day() {
        assert_eq!(
            normalize_bound("2026-09-08", false),
            Some("2026-09-08T00:00:00.000Z".into())
        );
        assert_eq!(
            normalize_bound("2026-09-08", true),
            Some("2026-09-08T23:59:59.999Z".into())
        );
        assert_eq!(
            normalize_bound("2026-09-08T09:30:00.000Z", false),
            Some("2026-09-08T09:30:00.000Z".into())
        );
        assert_eq!(normalize_bound("", false), None);
        assert_eq!(normalize_bound("   ", true), None);
    }
}
