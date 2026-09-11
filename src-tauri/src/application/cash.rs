use sqlx::sqlite::SqliteConnection;
use sqlx::Row;

use crate::application::auth::Principal;
use crate::application::suppliers::state_audit;
use crate::dto::purchases::{CashAccountDto, CashAccountInput, CashEntryDto};
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodDto {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub is_active: bool,
}

/// Current balance of a cash account, rebuildable from the opening balance plus
/// the sum of all signed cash entries.
pub(crate) async fn cash_balance(
    conn: &mut SqliteConnection,
    account_id: i64,
) -> Result<i64, AppError> {
    let opening: i64 =
        sqlx::query_scalar("SELECT opening_balance_minor FROM cash_accounts WHERE id = ?")
            .bind(account_id)
            .fetch_one(&mut *conn)
            .await?;
    let entries: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0) FROM cash_entries WHERE cash_account_id = ?",
    )
    .bind(account_id)
    .fetch_one(&mut *conn)
    .await?;
    Ok(opening + entries)
}

/// Throws when posting would drive the account into an overdraft, exercising the
/// atomic-rollback path for every cash movement.
pub(crate) async fn require_cash_balance(
    conn: &mut SqliteConnection,
    account_id: i64,
    delta: i64,
    label: &str,
) -> Result<(), AppError> {
    let balance = cash_balance(conn, account_id).await?;
    if balance + delta < 0 {
        return Err(AppError::Validation(format!(
            "{label}: cash account {account_id} would go into overdraft (balance {balance})"
        )));
    }
    Ok(())
}

/// Append a cash entry and refresh the account balance inside the caller's write.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn record_cash_entry(
    tx: &mut SqliteConnection,
    account_id: i64,
    entry_type: &str,
    amount_minor: i64,
    reference_type: &str,
    reference_id: i64,
    reason: &str,
    actor_id: i64,
) -> Result<i64, AppError> {
    let id = sqlx::query(
        "INSERT INTO cash_entries
           (cash_account_id, entry_type, amount_minor, reference_type, reference_id, reason, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(account_id)
    .bind(entry_type)
    .bind(amount_minor)
    .bind(reference_type)
    .bind(reference_id)
    .bind(reason)
    .bind(actor_id)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    let balance = cash_balance(tx, account_id).await?;
    sqlx::query("UPDATE cash_accounts SET balance_minor = ? WHERE id = ?")
        .bind(balance)
        .bind(account_id)
        .execute(&mut *tx)
        .await?;
    Ok(id)
}

pub async fn list_payment_methods(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<PaymentMethodDto>, AppError> {
    principal.require_any(&["payable.view", "expense.view"])?;
    let rows = sqlx::query("SELECT id, code, name, is_active FROM payment_methods ORDER BY name")
        .fetch_all(&state.pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| PaymentMethodDto {
            id: r.get(0),
            code: r.get(1),
            name: r.get(2),
            is_active: r.get::<i64, _>(3) != 0,
        })
        .collect())
}

pub async fn list_cash_accounts(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<CashAccountDto>, AppError> {
    principal.require_any(&["payable.view", "expense.view"])?;
    let rows = sqlx::query(
        "SELECT id, code, name, kind, opening_balance_minor, balance_minor, is_active
         FROM cash_accounts ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows.into_iter().map(|r| map_cash_account(&r)).collect())
}

fn map_cash_account(r: &sqlx::sqlite::SqliteRow) -> CashAccountDto {
    CashAccountDto {
        id: r.get(0),
        code: r.get(1),
        name: r.get(2),
        kind: r.get(3),
        opening_balance_minor: r.get(4),
        balance_minor: r.get(5),
        is_active: r.get::<i64, _>(6) != 0,
    }
}

pub async fn create_cash_account(
    state: &AppState,
    principal: &Principal,
    input: CashAccountInput,
    correlation_id: &str,
) -> Result<CashAccountDto, AppError> {
    principal.require("purchase.create")?;

    let code = input.code.trim().to_uppercase();
    if code.is_empty() {
        return Err(AppError::Validation("cash account code is required".into()));
    }
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("cash account name is required".into()));
    }
    let kind = input.kind.unwrap_or_else(|| "cash".into());
    if kind != "cash" && kind != "bank" {
        return Err(AppError::Validation("kind must be cash or bank".into()));
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
            let kind = kind.clone();
            Box::pin(async move {
                let dup: Option<i64> = sqlx::query_scalar("SELECT 1 FROM cash_accounts WHERE code = ?")
                    .bind(&code)
                    .fetch_optional(&mut *tx)
                    .await?;
                if dup.is_some() {
                    return Err(AppError::Conflict(format!(
                        "cash account code '{code}' already exists"
                    )));
                }
                let id = sqlx::query(
                    "INSERT INTO cash_accounts (code, name, kind, opening_balance_minor, balance_minor)
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind(&code)
                .bind(&name)
                .bind(&kind)
                .bind(opening)
                .bind(opening)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "cash.account_create",
                    "cash_account",
                    id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "code": code, "name": name, "kind": kind, "opening_balance_minor": opening
                    })),
                )
                .await?;
                Ok(id)
            })
        })
        .await?;

    let row = sqlx::query(
        "SELECT id, code, name, kind, opening_balance_minor, balance_minor, is_active
         FROM cash_accounts WHERE id = ?",
    )
    .bind(result)
    .fetch_one(&state.pool)
    .await?;
    Ok(map_cash_account(&row))
}

pub async fn list_cash_entries(
    state: &AppState,
    principal: &Principal,
    account_id: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<CashEntryDto>, AppError> {
    principal.require("payable.view")?;
    let limit = limit.unwrap_or(100).min(500);

    let mut sql = String::from(
        "SELECT id, cash_account_id, entry_type, amount_minor, reference_type,
                reference_id, reason, created_by, created_at
         FROM cash_entries WHERE 1 = 1",
    );
    if account_id.is_some() {
        sql.push_str(" AND cash_account_id = ?");
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");

    let mut query = sqlx::query(&sql);
    if let Some(aid) = account_id {
        query = query.bind(aid);
    }
    query = query.bind(limit);

    let rows = query.fetch_all(&state.pool).await?;
    Ok(rows
        .into_iter()
        .map(|r| CashEntryDto {
            id: r.get(0),
            cash_account_id: r.get(1),
            entry_type: r.get(2),
            amount_minor: r.get(3),
            reference_type: r.try_get(4).ok(),
            reference_id: r.try_get(5).ok(),
            reason: r.try_get(6).ok(),
            created_by: r.get(7),
            created_at: r.get(8),
        })
        .collect())
}
