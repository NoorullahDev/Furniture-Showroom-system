use sqlx::sqlite::SqliteConnection;
use sqlx::Row;

use crate::application::auth::Principal;
use crate::dto::purchases::{SupplierDto, SupplierInput, SupplierLedgerEntryDto};
use crate::error::AppError;
use crate::infrastructure::audit::AuditService;
use crate::infrastructure::AuditInput;
use crate::state::AppState;

pub(crate) async fn require_supplier(
    conn: &mut SqliteConnection,
    supplier_id: i64,
) -> Result<(), AppError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM suppliers WHERE id = ? AND is_active = 1")
            .bind(supplier_id)
            .fetch_optional(conn)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!("supplier {supplier_id}")));
    }
    Ok(())
}

/// Current payable balance for a supplier: the running balance from the ledger,
/// or the account opening balance when no entries exist yet.
pub(crate) async fn supplier_balance(
    conn: &mut SqliteConnection,
    supplier_id: i64,
) -> Result<i64, AppError> {
    let latest: Option<i64> = sqlx::query_scalar(
        "SELECT balance_after_minor FROM supplier_ledger_entries
         WHERE supplier_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(supplier_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(balance) = latest {
        return Ok(balance);
    }
    let opening: i64 =
        sqlx::query_scalar("SELECT opening_balance_minor FROM suppliers WHERE id = ?")
            .bind(supplier_id)
            .fetch_one(&mut *conn)
            .await?;
    Ok(opening)
}

fn map_supplier(row: &sqlx::sqlite::SqliteRow, balance_minor: i64) -> SupplierDto {
    SupplierDto {
        id: row.get(0),
        code: row.get(1),
        name: row.get(2),
        phone: row.try_get(3).ok(),
        email: row.try_get(4).ok(),
        address: row.try_get(5).ok(),
        opening_balance_minor: row.get(6),
        balance_minor,
        is_active: row.get::<i64, _>(7) != 0,
        created_at: row.get(8),
    }
}

pub async fn create(
    state: &AppState,
    principal: &Principal,
    input: SupplierInput,
    correlation_id: &str,
) -> Result<SupplierDto, AppError> {
    principal.require("supplier.create")?;

    let code = input.code.trim().to_uppercase();
    if code.is_empty() {
        return Err(AppError::Validation("supplier code is required".into()));
    }
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("supplier name is required".into()));
    }
    let opening = input.opening_balance_minor.unwrap_or(0);
    if opening < 0 {
        return Err(AppError::Validation(
            "opening balance cannot be negative".into(),
        ));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let code = code.clone();
            let name = input.name.trim().to_string();
            let phone = input.phone.clone();
            let email = input.email.clone();
            let address = input.address.clone();
            Box::pin(async move {
                let existing: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM suppliers WHERE code = ?",
                )
                .bind(&code)
                .fetch_optional(&mut *tx)
                .await?;
                if existing.is_some() {
                    return Err(AppError::Conflict(format!(
                        "supplier code '{code}' already exists"
                    )));
                }

                let id = sqlx::query(
                    "INSERT INTO suppliers (code, name, phone, email, address, opening_balance_minor, created_by)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&code)
                .bind(&name)
                .bind(phone.as_deref())
                .bind(email.as_deref())
                .bind(address.as_deref())
                .bind(opening)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                if opening != 0 {
                    let balance = supplier_balance(&mut *tx, id).await?;
                    sqlx::query(
                        "INSERT INTO supplier_ledger_entries
                           (supplier_id, entry_type, document_type, document_id,
                            amount_minor, balance_after_minor, notes, created_by)
                         VALUES (?, 'opening_balance', 'supplier', ?, ?, ?, ?, ?)",
                    )
                    .bind(id)
                    .bind(id)
                    .bind(opening)
                    .bind(balance)
                    .bind("account opening balance".to_string())
                    .bind(actor_id)
                    .execute(&mut *tx)
                    .await?;
                }

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "supplier.create",
                    "supplier",
                    id,
                    &correlation,
                    Some(serde_json::json!({
                        "code": code, "name": name, "opening_balance_minor": opening
                    })),
                    None,
                )
                .await?;
                Ok(id)
            })
        })
        .await?;

    get(state, principal, result).await
}

pub async fn update(
    state: &AppState,
    principal: &Principal,
    supplier_id: i64,
    input: SupplierInput,
    correlation_id: &str,
) -> Result<SupplierDto, AppError> {
    principal.require("supplier.create")?;

    let code = input.code.trim().to_uppercase();
    if code.is_empty() {
        return Err(AppError::Validation("supplier code is required".into()));
    }
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("supplier name is required".into()));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let code = code.clone();
            let name = input.name.trim().to_string();
            let phone = input.phone.clone();
            let email = input.email.clone();
            let address = input.address.clone();
            let is_active = input.is_active.unwrap_or(true);
            Box::pin(async move {
                let exists: Option<i64> =
                    sqlx::query_scalar("SELECT 1 FROM suppliers WHERE id = ?")
                        .bind(supplier_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                if exists.is_none() {
                    return Err(AppError::NotFound(format!("supplier {supplier_id}")));
                }
                let dup: Option<i64> =
                    sqlx::query_scalar("SELECT 1 FROM suppliers WHERE code = ? AND id != ?")
                        .bind(&code)
                        .bind(supplier_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                if dup.is_some() {
                    return Err(AppError::Conflict(format!(
                        "supplier code '{code}' already exists"
                    )));
                }
                sqlx::query(
                    "UPDATE suppliers SET code = ?, name = ?, phone = ?, email = ?,
                            address = ?, is_active = ?,
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                     WHERE id = ?",
                )
                .bind(&code)
                .bind(&name)
                .bind(phone.as_deref())
                .bind(email.as_deref())
                .bind(address.as_deref())
                .bind(is_active as i64)
                .bind(supplier_id)
                .execute(&mut *tx)
                .await?;
                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "supplier.update",
                    "supplier",
                    supplier_id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "code": code, "name": name, "is_active": is_active
                    })),
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    get(state, principal, supplier_id).await
}

pub async fn list(state: &AppState, principal: &Principal) -> Result<Vec<SupplierDto>, AppError> {
    principal.require("payable.view")?;
    let rows = sqlx::query(
        "SELECT id, code, name, phone, email, address, opening_balance_minor, is_active, created_at
         FROM suppliers ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = row.get(0);
        let mut conn = state.pool.acquire().await?;
        let balance = supplier_balance(&mut conn, id).await?;
        out.push(map_supplier(&row, balance));
    }
    Ok(out)
}

pub async fn get(
    state: &AppState,
    principal: &Principal,
    supplier_id: i64,
) -> Result<SupplierDto, AppError> {
    principal.require("payable.view")?;
    let row = sqlx::query(
        "SELECT id, code, name, phone, email, address, opening_balance_minor, is_active, created_at
         FROM suppliers WHERE id = ?",
    )
    .bind(supplier_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("supplier {supplier_id}")))?;
    let mut conn = state.pool.acquire().await?;
    let balance = supplier_balance(&mut conn, supplier_id).await?;
    Ok(map_supplier(&row, balance))
}

pub async fn ledger(
    state: &AppState,
    principal: &Principal,
    supplier_id: i64,
) -> Result<Vec<SupplierLedgerEntryDto>, AppError> {
    principal.require("payable.view")?;
    let rows = sqlx::query(
        "SELECT id, entry_type, document_type, document_id, amount_minor,
                balance_after_minor, notes, created_by, created_at
         FROM supplier_ledger_entries
         WHERE supplier_id = ?
         ORDER BY id DESC",
    )
    .bind(supplier_id)
    .fetch_all(&state.pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(SupplierLedgerEntryDto {
            id: row.get(0),
            entry_type: row.get(1),
            document_type: row.try_get(2).ok(),
            document_id: row.try_get(3).ok(),
            amount_minor: row.get(4),
            balance_after_minor: row.get(5),
            notes: row.try_get(6).ok(),
            created_by: row.get(7),
            created_at: row.get(8),
        });
    }
    Ok(out)
}

/// Record an audit event inside the caller's write transaction.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn state_audit(
    tx: &mut SqliteConnection,
    audits: &AuditService,
    actor_id: i64,
    actor_session: &str,
    action: &str,
    entity_type: &str,
    entity_id: i64,
    correlation: &str,
    before_json: Option<serde_json::Value>,
    after_json: Option<serde_json::Value>,
) -> Result<(), AppError> {
    audits
        .record(
            tx,
            AuditInput {
                user_id: Some(actor_id),
                session_id: Some(actor_session.to_string()),
                action: action.into(),
                entity_type: Some(entity_type.into()),
                entity_id: Some(entity_id.to_string()),
                before_json: before_json.map(|v| v.to_string()),
                after_json: after_json.map(|v| v.to_string()),
                correlation_id: Some(correlation.to_string()),
                ..Default::default()
            },
        )
        .await?;
    Ok(())
}
