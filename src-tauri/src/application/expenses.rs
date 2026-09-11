use sqlx::Row;

use crate::application::auth::Principal;
use crate::application::cash::{record_cash_entry, require_cash_balance};
use crate::application::documents::next_document_number;
use crate::application::suppliers::state_audit;
use crate::dto::expenses::{
    ExpenseCategoryDto, ExpenseCategoryInput, ExpenseCategoryUpdateInput, ExpenseDto, ExpenseInput,
    ExpenseListInput, ExpensePageDto, ExpensePageInput, ExpenseReverseInput, OwnerTransactionDto,
    OwnerTransactionInput, ProfitSummaryDto,
};
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::state::AppState;

fn validate_date(label: &str, value: &str) -> Result<(), AppError> {
    let trimmed = value.trim();
    if chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").is_err() {
        return Err(AppError::Validation(format!(
            "{label}: date must be a valid YYYY-MM-DD date"
        )));
    }
    Ok(())
}

async fn require_active_cash_account(
    tx: &mut sqlx::SqliteConnection,
    account_id: i64,
    label: &str,
) -> Result<(), AppError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM cash_accounts WHERE id = ? AND is_active = 1")
            .bind(account_id)
            .fetch_optional(&mut *tx)
            .await?;
    if found.is_none() {
        return Err(AppError::Validation(format!(
            "{label}: cash account {account_id} is not active"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Expense categories
// ---------------------------------------------------------------------------

pub async fn expense_category_list(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<ExpenseCategoryDto>, AppError> {
    principal.require("expense.view")?;
    let rows = sqlx::query(
        "SELECT id, code, name, is_active, created_by, created_at, updated_at
         FROM expense_categories ORDER BY is_active DESC, name",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows.into_iter().map(|r| map_category(&r)).collect())
}

fn map_category(r: &sqlx::sqlite::SqliteRow) -> ExpenseCategoryDto {
    ExpenseCategoryDto {
        id: r.get(0),
        code: r.get(1),
        name: r.get(2),
        is_active: r.get::<i64, _>(3) != 0,
        created_by: r.try_get(4).ok(),
        created_at: r.get(5),
        updated_at: r.get(6),
    }
}

pub async fn expense_category_create(
    state: &AppState,
    principal: &Principal,
    input: ExpenseCategoryInput,
    correlation_id: &str,
) -> Result<ExpenseCategoryDto, AppError> {
    principal.require("expense.create")?;

    let code = input.code.trim().to_uppercase();
    let name = input.name.trim().to_string();
    if code.is_empty() {
        return Err(AppError::Validation("category code is required".into()));
    }
    if name.is_empty() {
        return Err(AppError::Validation("category name is required".into()));
    }
    let is_active = input.is_active.unwrap_or(true);

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let id = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let code = code.clone();
            let name = name.clone();
            Box::pin(async move {
                let dup: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM expense_categories
                      WHERE code = ? OR name = ? COLLATE NOCASE",
                )
                .bind(&code)
                .bind(&name)
                .fetch_optional(&mut *tx)
                .await?;
                if dup.is_some() {
                    return Err(AppError::Conflict(format!(
                        "an expense category named '{name}' (or code '{code}') already exists"
                    )));
                }
                let id = sqlx::query(
                    "INSERT INTO expense_categories (code, name, is_active, created_by)
                     VALUES (?, ?, ?, ?)",
                )
                .bind(&code)
                .bind(&name)
                .bind(is_active)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "expense.category_create",
                    "expense_category",
                    id,
                    &correlation,
                    None,
                    Some(serde_json::json!({ "code": code, "name": name, "is_active": is_active })),
                )
                .await?;
                Ok(id)
            })
        })
        .await?;

    let row = sqlx::query(
        "SELECT id, code, name, is_active, created_by, created_at, updated_at
         FROM expense_categories WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    Ok(map_category(&row))
}

pub async fn expense_category_update(
    state: &AppState,
    principal: &Principal,
    input: ExpenseCategoryUpdateInput,
    correlation_id: &str,
) -> Result<ExpenseCategoryDto, AppError> {
    principal.require("expense.create")?;

    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("category name is required".into()));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let name = name.clone();
            Box::pin(async move {
                let existing: Option<String> =
                    sqlx::query_scalar("SELECT code FROM expense_categories WHERE id = ?")
                        .bind(input.id)
                        .fetch_optional(&mut *tx)
                        .await?;
                let Some(code) = existing else {
                    return Err(AppError::NotFound(format!(
                        "expense category {} not found",
                        input.id
                    )));
                };
                let duplicate_name: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM expense_categories
                      WHERE name = ? COLLATE NOCASE AND id != ?",
                )
                .bind(&name)
                .bind(input.id)
                .fetch_optional(&mut *tx)
                .await?;
                if duplicate_name.is_some() {
                    return Err(AppError::Conflict(format!(
                        "an expense category named '{name}' already exists"
                    )));
                }
                sqlx::query(
                    "UPDATE expense_categories SET name = ?, is_active = ?,
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(&name)
                .bind(input.is_active)
                .bind(input.id)
                .execute(&mut *tx)
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "expense.category_update",
                    "expense_category",
                    input.id,
                    &correlation,
                    Some(serde_json::json!({ "code": code })),
                    Some(serde_json::json!({
                        "name": name, "is_active": input.is_active
                    })),
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    let row = sqlx::query(
        "SELECT id, code, name, is_active, created_by, created_at, updated_at
         FROM expense_categories WHERE id = ?",
    )
    .bind(input.id)
    .fetch_one(&state.pool)
    .await?;
    Ok(map_category(&row))
}

// ---------------------------------------------------------------------------
// Expenses
// ---------------------------------------------------------------------------

pub async fn expense_list(
    state: &AppState,
    principal: &Principal,
    input: ExpenseListInput,
) -> Result<Vec<ExpenseDto>, AppError> {
    principal.require("expense.view")?;
    let limit = input.limit.unwrap_or(100).clamp(1, 10_000);

    let mut sql = String::from(
        "SELECT e.id, e.expense_number, e.category_id, c.code, c.name,
                e.amount_minor, e.expense_date, e.cash_account_id, a.name,
                e.payment_method_id, pm.name,
                e.description, e.payee, e.reference, e.attachment_path,
                e.status, e.idempotency_key, e.created_by, e.created_at,
                e.posted_by, e.posted_at, e.reversed_by, e.reversed_at,
                e.reversal_reason, e.updated_at
         FROM expenses e
         JOIN expense_categories c ON c.id = e.category_id
         JOIN cash_accounts a ON a.id = e.cash_account_id
         LEFT JOIN payment_methods pm ON pm.id = e.payment_method_id
        WHERE 1 = 1",
    );
    if input.status.is_some() {
        sql.push_str(" AND e.status = ?");
    }
    if input.category_id.is_some() {
        sql.push_str(" AND e.category_id = ?");
    }
    if input.cash_account_id.is_some() {
        sql.push_str(" AND e.cash_account_id = ?");
    }
    if input.from_date.is_some() {
        sql.push_str(" AND e.expense_date >= ?");
    }
    if input.to_date.is_some() {
        sql.push_str(" AND e.expense_date <= ?");
    }
    sql.push_str(" ORDER BY e.expense_date DESC, e.id DESC LIMIT ?");

    let mut query = sqlx::query(&sql);
    if let Some(status) = &input.status {
        query = query.bind(status);
    }
    if let Some(cat) = input.category_id {
        query = query.bind(cat);
    }
    if let Some(acc) = input.cash_account_id {
        query = query.bind(acc);
    }
    if let Some(d) = &input.from_date {
        query = query.bind(d);
    }
    if let Some(d) = &input.to_date {
        query = query.bind(d);
    }
    query = query.bind(limit);

    let rows = query.fetch_all(&state.pool).await?;
    Ok(rows.into_iter().map(|r| map_expense(&r)).collect())
}

fn map_expense(r: &sqlx::sqlite::SqliteRow) -> ExpenseDto {
    ExpenseDto {
        id: r.get(0),
        expense_number: r.try_get(1).ok(),
        category_id: r.get(2),
        category_code: r.get(3),
        category_name: r.get(4),
        amount_minor: r.get(5),
        expense_date: r.get(6),
        cash_account_id: r.get(7),
        cash_account_name: r.get(8),
        payment_method_id: r.try_get(9).ok(),
        payment_method_name: r.try_get(10).ok(),
        description: r.get(11),
        payee: r.try_get(12).ok(),
        reference: r.try_get(13).ok(),
        attachment_path: r.try_get(14).ok(),
        status: r.get(15),
        idempotency_key: r.try_get(16).ok(),
        created_by: r.get(17),
        created_at: r.get(18),
        posted_by: r.try_get(19).ok(),
        posted_at: r.try_get(20).ok(),
        reversed_by: r.try_get(21).ok(),
        reversed_at: r.try_get(22).ok(),
        reversal_reason: r.try_get(23).ok(),
        updated_at: r.get(24),
    }
}

pub async fn expense_page(
    state: &AppState,
    principal: &Principal,
    input: ExpensePageInput,
) -> Result<ExpensePageDto, AppError> {
    principal.require("expense.view")?;

    let limit = input.limit.unwrap_or(20).clamp(1, 100);
    let offset = input.offset.unwrap_or(0).max(0);
    if let Some(from) = input.from_date.as_deref() {
        validate_date("from date", from)?;
    }
    if let Some(to) = input.to_date.as_deref() {
        validate_date("to date", to)?;
    }
    if let (Some(from), Some(to)) = (input.from_date.as_ref(), input.to_date.as_ref()) {
        if from > to {
            return Err(AppError::Validation(
                "from date cannot be after to date".into(),
            ));
        }
    }
    if let Some(status) = input.status.as_deref() {
        if !matches!(status, "posted" | "reversed" | "draft") {
            return Err(AppError::Validation("invalid expense status".into()));
        }
    }

    let search = input
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{}%", value.to_lowercase()));
    let mut where_sql = String::from(" WHERE 1 = 1");
    if input.status.is_some() {
        where_sql.push_str(" AND e.status = ?");
    }
    if input.category_id.is_some() {
        where_sql.push_str(" AND e.category_id = ?");
    }
    if input.cash_account_id.is_some() {
        where_sql.push_str(" AND e.cash_account_id = ?");
    }
    if input.from_date.is_some() {
        where_sql.push_str(" AND e.expense_date >= ?");
    }
    if input.to_date.is_some() {
        where_sql.push_str(" AND e.expense_date <= ?");
    }
    if search.is_some() {
        where_sql.push_str(
            " AND (LOWER(e.description) LIKE ? OR LOWER(c.name) LIKE ?
                    OR LOWER(COALESCE(e.reference, '')) LIKE ?)",
        );
    }

    let summary_sql = format!(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN e.status = 'posted' THEN e.amount_minor ELSE 0 END), 0)
           FROM expenses e
           JOIN expense_categories c ON c.id = e.category_id{where_sql}"
    );
    let mut summary_query = sqlx::query(&summary_sql);
    if let Some(status) = input.status.as_ref() {
        summary_query = summary_query.bind(status);
    }
    if let Some(category_id) = input.category_id {
        summary_query = summary_query.bind(category_id);
    }
    if let Some(account_id) = input.cash_account_id {
        summary_query = summary_query.bind(account_id);
    }
    if let Some(from) = input.from_date.as_ref() {
        summary_query = summary_query.bind(from);
    }
    if let Some(to) = input.to_date.as_ref() {
        summary_query = summary_query.bind(to);
    }
    if let Some(term) = search.as_ref() {
        summary_query = summary_query.bind(term).bind(term).bind(term);
    }
    let summary = summary_query.fetch_one(&state.pool).await?;
    let total: i64 = summary.get(0);
    let total_amount_minor: i64 = summary.get(1);

    let sort_column = match input.sort_by.as_deref() {
        Some("category") => "c.name",
        Some("note") => "e.description",
        Some("amount") => "e.amount_minor",
        _ => "e.expense_date",
    };
    let sort_direction = if input
        .sort_direction
        .as_deref()
        .is_some_and(|direction| direction.eq_ignore_ascii_case("asc"))
    {
        "ASC"
    } else {
        "DESC"
    };
    let list_sql = format!(
        "SELECT e.id, e.expense_number, e.category_id, c.code, c.name,
                e.amount_minor, e.expense_date, e.cash_account_id, a.name,
                e.payment_method_id, pm.name,
                e.description, e.payee, e.reference, e.attachment_path,
                e.status, e.idempotency_key, e.created_by, e.created_at,
                e.posted_by, e.posted_at, e.reversed_by, e.reversed_at,
                e.reversal_reason, e.updated_at
           FROM expenses e
           JOIN expense_categories c ON c.id = e.category_id
           JOIN cash_accounts a ON a.id = e.cash_account_id
           LEFT JOIN payment_methods pm ON pm.id = e.payment_method_id{where_sql}
          ORDER BY {sort_column} {sort_direction}, e.id {sort_direction}
          LIMIT ? OFFSET ?"
    );
    let mut list_query = sqlx::query(&list_sql);
    if let Some(status) = input.status.as_ref() {
        list_query = list_query.bind(status);
    }
    if let Some(category_id) = input.category_id {
        list_query = list_query.bind(category_id);
    }
    if let Some(account_id) = input.cash_account_id {
        list_query = list_query.bind(account_id);
    }
    if let Some(from) = input.from_date.as_ref() {
        list_query = list_query.bind(from);
    }
    if let Some(to) = input.to_date.as_ref() {
        list_query = list_query.bind(to);
    }
    if let Some(term) = search.as_ref() {
        list_query = list_query.bind(term).bind(term).bind(term);
    }
    let rows = list_query
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;

    Ok(ExpensePageDto {
        items: rows.iter().map(map_expense).collect(),
        total,
        total_amount_minor,
    })
}

pub async fn expense_post(
    state: &AppState,
    principal: &Principal,
    input: ExpenseInput,
    correlation_id: &str,
) -> Result<ExpenseDto, AppError> {
    principal.require("expense.create")?;

    if input.amount_minor <= 0 {
        return Err(AppError::Validation(
            "expense amount must be positive".into(),
        ));
    }
    let description = input.description.trim().to_string();
    validate_date("expense date", &input.expense_date)?;
    let payee = input
        .payee
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let reference = input
        .reference
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let attachment = input
        .attachment_path
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let idem_key = input
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let id = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let description = description.clone();
            let payee = payee.map(str::to_string);
            let reference = reference.map(str::to_string);
            let attachment = attachment.map(str::to_string);
            let idem_key = idem_key.map(str::to_string);
            Box::pin(async move {
                if let Some(key) = &idem_key {
                    let existing: Option<i64> =
                        sqlx::query_scalar("SELECT id FROM expenses WHERE idempotency_key = ?")
                            .bind(key)
                            .fetch_optional(&mut *tx)
                            .await?;
                    if let Some(existing_id) = existing {
                        return Ok(existing_id);
                    }
                }

                require_active_cash_account(&mut *tx, input.cash_account_id, "expense").await?;

                let payment_method: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM payment_methods WHERE id = ? AND is_active = 1",
                )
                .bind(input.payment_method_id)
                .fetch_optional(&mut *tx)
                .await?;
                if payment_method.is_none() {
                    return Err(AppError::Validation(format!(
                        "payment method {} is not active",
                        input.payment_method_id
                    )));
                }

                let category: Option<(i64, String)> = sqlx::query_as(
                    "SELECT id, name FROM expense_categories WHERE id = ? AND is_active = 1",
                )
                .bind(input.category_id)
                .fetch_optional(&mut *tx)
                .await?;
                if category.is_none() {
                    return Err(AppError::Validation(format!(
                        "expense category {} is not active",
                        input.category_id
                    )));
                }

                let number = next_document_number(&mut *tx, "expense").await?;
                let id = sqlx::query(
                    "INSERT INTO expenses
                       (expense_number, category_id, amount_minor, expense_date,
                        cash_account_id, payment_method_id, description, payee, reference, attachment_path,
                        status, idempotency_key, created_by, posted_by,
                        posted_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'posted', ?, ?, ?,
                             strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
                )
                .bind(&number)
                .bind(input.category_id)
                .bind(input.amount_minor)
                .bind(&input.expense_date)
                .bind(input.cash_account_id)
                .bind(input.payment_method_id)
                .bind(&description)
                .bind(&payee)
                .bind(&reference)
                .bind(&attachment)
                .bind(&idem_key)
                .bind(actor_id)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                require_cash_balance(
                    &mut *tx,
                    input.cash_account_id,
                    -input.amount_minor,
                    "expense",
                )
                .await?;
                let reason = format!("Expense {number} — {description}");
                record_cash_entry(
                    &mut *tx,
                    input.cash_account_id,
                    "expense",
                    -input.amount_minor,
                    "expense",
                    id,
                    &reason,
                    actor_id,
                )
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "expense.post",
                    "expense",
                    id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "expense_number": number,
                        "category_id": input.category_id,
                        "amount_minor": input.amount_minor,
                        "expense_date": input.expense_date,
                        "cash_account_id": input.cash_account_id,
                        "payment_method_id": input.payment_method_id,
                        "description": description,
                        "payee": payee,
                        "reference": reference,
                    })),
                )
                .await?;
                Ok(id)
            })
        })
        .await?;

    expense_dto(state, id).await
}

pub async fn expense_reverse(
    state: &AppState,
    principal: &Principal,
    input: ExpenseReverseInput,
    correlation_id: &str,
) -> Result<ExpenseDto, AppError> {
    principal.require("expense.reverse")?;

    let reason = input.reason.trim().to_string();
    if reason.is_empty() {
        return Err(AppError::Validation("reversal reason is required".into()));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let now_iso = state.clock.now_iso();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = reason.clone();
            Box::pin(async move {
                let current: Option<(String, i64, i64, String)> = sqlx::query_as(
                    "SELECT status, amount_minor, cash_account_id, expense_number
                     FROM expenses WHERE id = ?",
                )
                .bind(input.expense_id)
                .fetch_optional(&mut *tx)
                .await?;
                let Some((status, amount, account_id, number)) = current else {
                    return Err(AppError::NotFound(format!(
                        "expense {} not found",
                        input.expense_id
                    )));
                };
                if status != "posted" {
                    return Err(AppError::Validation(format!(
                        "expense {number} is '{status}', only posted expenses can be reversed"
                    )));
                }

                record_cash_entry(
                    &mut *tx,
                    account_id,
                    "expense_reversal",
                    amount,
                    "expense",
                    input.expense_id,
                    &format!("Reversal of {number} — {reason}"),
                    actor_id,
                )
                .await?;

                sqlx::query(
                    "UPDATE expenses
                        SET status = 'reversed', reversed_by = ?, reversed_at = ?,
                            reversal_reason = ?,
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(actor_id)
                .bind(&now_iso)
                .bind(&reason)
                .bind(input.expense_id)
                .execute(&mut *tx)
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "expense.reverse",
                    "expense",
                    input.expense_id,
                    &correlation,
                    Some(serde_json::json!({ "status": status })),
                    Some(serde_json::json!({
                        "status": "reversed",
                        "reversed_amount_minor": amount,
                        "reversal_reason": reason,
                    })),
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    expense_dto(state, input.expense_id).await
}

async fn expense_dto(state: &AppState, id: i64) -> Result<ExpenseDto, AppError> {
    let row = sqlx::query(
        "SELECT e.id, e.expense_number, e.category_id, c.code, c.name,
                e.amount_minor, e.expense_date, e.cash_account_id, a.name,
                e.payment_method_id, pm.name,
                e.description, e.payee, e.reference, e.attachment_path,
                e.status, e.idempotency_key, e.created_by, e.created_at,
                e.posted_by, e.posted_at, e.reversed_by, e.reversed_at,
                e.reversal_reason, e.updated_at
         FROM expenses e
         JOIN expense_categories c ON c.id = e.category_id
         JOIN cash_accounts a ON a.id = e.cash_account_id
         LEFT JOIN payment_methods pm ON pm.id = e.payment_method_id
        WHERE e.id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(row) = row else {
        return Err(AppError::NotFound(format!("expense {id} not found")));
    };
    Ok(map_expense(&row))
}

// ---------------------------------------------------------------------------
// Owner capital and withdrawals
// ---------------------------------------------------------------------------

pub async fn owner_transaction_post(
    state: &AppState,
    principal: &Principal,
    input: OwnerTransactionInput,
    correlation_id: &str,
) -> Result<OwnerTransactionDto, AppError> {
    principal.require("owner.transfer")?;

    if input.kind != "capital_in" && input.kind != "withdrawal" {
        return Err(AppError::Validation(
            "kind must be capital_in or withdrawal".into(),
        ));
    }
    if input.amount_minor <= 0 {
        return Err(AppError::Validation(
            "transfer amount must be positive".into(),
        ));
    }
    validate_date("transaction date", &input.transaction_date)?;
    let notes = input
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let idem_key = input
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let entry_type = if input.kind == "capital_in" {
        "owner_capital"
    } else {
        "owner_withdrawal"
    };
    let amount_signed = if input.kind == "capital_in" {
        input.amount_minor
    } else {
        -input.amount_minor
    };

    let id = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let entry_type = entry_type.to_string();
            let notes = notes.map(str::to_string);
            let idem_key = idem_key.map(str::to_string);
            Box::pin(async move {
                if let Some(key) = &idem_key {
                    let existing: Option<i64> = sqlx::query_scalar(
                        "SELECT id FROM owner_transactions WHERE idempotency_key = ?",
                    )
                    .bind(key)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if let Some(existing_id) = existing {
                        return Ok(existing_id);
                    }
                }

                require_active_cash_account(&mut *tx, input.cash_account_id, "owner transfer")
                    .await?;
                if amount_signed < 0 {
                    require_cash_balance(
                        &mut *tx,
                        input.cash_account_id,
                        amount_signed,
                        "owner withdrawal",
                    )
                    .await?;
                }

                let number = next_document_number(&mut *tx, "owner_transaction").await?;
                let id = sqlx::query(
                    "INSERT INTO owner_transactions
                       (transaction_number, kind, amount_minor, transaction_date,
                        cash_account_id, notes, idempotency_key, created_by)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&number)
                .bind(&input.kind)
                .bind(input.amount_minor)
                .bind(&input.transaction_date)
                .bind(input.cash_account_id)
                .bind(&notes)
                .bind(&idem_key)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                let label = if input.kind == "capital_in" {
                    "capital contribution"
                } else {
                    "owner withdrawal"
                };
                let reason = format!(
                    "{label} {number}{}",
                    notes
                        .as_deref()
                        .map_or(String::new(), |n| format!(" — {n}"))
                );
                record_cash_entry(
                    &mut *tx,
                    input.cash_account_id,
                    &entry_type,
                    amount_signed,
                    "owner_transaction",
                    id,
                    &reason,
                    actor_id,
                )
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "owner.transfer",
                    "owner_transaction",
                    id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "transaction_number": number,
                        "kind": input.kind,
                        "amount_minor": input.amount_minor,
                        "transaction_date": input.transaction_date,
                        "cash_account_id": input.cash_account_id,
                        "notes": notes,
                    })),
                )
                .await?;
                Ok(id)
            })
        })
        .await?;

    owner_transaction_dto(state, id).await
}

pub async fn owner_transaction_list(
    state: &AppState,
    principal: &Principal,
    limit: Option<i64>,
) -> Result<Vec<OwnerTransactionDto>, AppError> {
    principal.require("owner.transfer")?;
    let limit = limit.unwrap_or(50).min(500);

    let rows = sqlx::query(
        "SELECT t.id, t.transaction_number, t.kind, t.amount_minor,
                t.transaction_date, t.cash_account_id, a.name,
                t.notes, t.idempotency_key, t.created_by, t.created_at
         FROM owner_transactions t
         JOIN cash_accounts a ON a.id = t.cash_account_id
         ORDER BY t.transaction_date DESC, t.id DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| OwnerTransactionDto {
            id: r.get(0),
            transaction_number: r.get(1),
            kind: r.get(2),
            amount_minor: r.get(3),
            transaction_date: r.get(4),
            cash_account_id: r.get(5),
            cash_account_name: r.get(6),
            notes: r.try_get(7).ok(),
            idempotency_key: r.try_get(8).ok(),
            created_by: r.get(9),
            created_at: r.get(10),
        })
        .collect())
}

async fn owner_transaction_dto(state: &AppState, id: i64) -> Result<OwnerTransactionDto, AppError> {
    let row = sqlx::query(
        "SELECT t.id, t.transaction_number, t.kind, t.amount_minor,
                t.transaction_date, t.cash_account_id, a.name,
                t.notes, t.idempotency_key, t.created_by, t.created_at
         FROM owner_transactions t
         JOIN cash_accounts a ON a.id = t.cash_account_id
        WHERE t.id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    Ok(OwnerTransactionDto {
        id: row.get(0),
        transaction_number: row.get(1),
        kind: row.get(2),
        amount_minor: row.get(3),
        transaction_date: row.get(4),
        cash_account_id: row.get(5),
        cash_account_name: row.get(6),
        notes: row.try_get(7).ok(),
        idempotency_key: row.try_get(8).ok(),
        created_by: row.get(9),
        created_at: row.get(10),
    })
}

// ---------------------------------------------------------------------------
// Profit and cash flow summary
// ---------------------------------------------------------------------------

/// Statutory profit on document dates, plus cash movement on entry (payment)
/// dates. Opening the ranges with the full calendar keeps the two perspectives
/// separate: revenue/COGS/expenses use document dates, the cash book uses the
/// day the money actually moved.
pub async fn profit_summary(
    state: &AppState,
    principal: &Principal,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<ProfitSummaryDto, AppError> {
    principal.require("profit.view")?;

    let from = from_date
        .clone()
        .unwrap_or_else(|| "0001-01-01".to_string());
    let to = to_date.clone().unwrap_or_else(|| "9999-12-31".to_string());
    validate_date("from date", &from)?;
    validate_date("to date", &to)?;
    if from > to {
        return Err(AppError::Validation(
            "from date cannot be after to date".into(),
        ));
    }

    let revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE((
            SELECT SUM(total_minor) FROM sales
             WHERE status = 'confirmed' AND sale_date BETWEEN ? AND ?
         ) - (
            SELECT COALESCE(SUM(total_minor), 0) FROM sales_returns
             WHERE status = 'posted' AND return_date BETWEEN ? AND ?
         ), 0)",
    )
    .bind(&from)
    .bind(&to)
    .bind(&from)
    .bind(&to)
    .fetch_one(&state.pool)
    .await?;

    let delivery_income: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(delivery_charge_minor), 0)
         FROM sales WHERE status = 'confirmed' AND sale_date BETWEEN ? AND ?",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(&state.pool)
    .await?;

    let cogs: i64 = sqlx::query_scalar(
        "SELECT COALESCE((
            SELECT SUM(cost_minor) FROM sales
             WHERE status = 'confirmed' AND sale_date BETWEEN ? AND ?
         ) - (
            SELECT COALESCE(SUM(ri.quantity * si.unit_cost_minor), 0)
              FROM sales_return_items ri
              JOIN sales_returns sr ON sr.id = ri.return_id AND sr.status = 'posted'
              JOIN sale_items si ON si.id = ri.sale_item_id
             WHERE ri.classification = 'sellable' AND sr.return_date BETWEEN ? AND ?
         ), 0)",
    )
    .bind(&from)
    .bind(&to)
    .bind(&from)
    .bind(&to)
    .fetch_one(&state.pool)
    .await?;

    let expenses: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0)
         FROM expenses WHERE status = 'posted' AND expense_date BETWEEN ? AND ?",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(&state.pool)
    .await?;

    let damage_loss: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(estimated_loss_minor), 0)
         FROM damage_records
        WHERE status = 'resolved' AND decision = 'write_off' AND damage_date BETWEEN ? AND ?",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(&state.pool)
    .await?;

    let owner_capital_in: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0)
         FROM owner_transactions
        WHERE kind = 'capital_in' AND transaction_date BETWEEN ? AND ?",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(&state.pool)
    .await?;

    let owner_withdrawals: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0)
         FROM owner_transactions
        WHERE kind = 'withdrawal' AND transaction_date BETWEEN ? AND ?",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(&state.pool)
    .await?;

    let cash_inflow: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0)
         FROM cash_entries WHERE amount_minor > 0 AND date(created_at) BETWEEN ? AND ?",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(&state.pool)
    .await?;

    let cash_outflow: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(-amount_minor), 0)
         FROM cash_entries WHERE amount_minor < 0 AND date(created_at) BETWEEN ? AND ?",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(&state.pool)
    .await?;

    let gross_profit = revenue - cogs;
    let operational_profit = gross_profit - expenses - damage_loss;

    Ok(ProfitSummaryDto {
        from_date: from,
        to_date: to,
        revenue_minor: revenue,
        cogs_minor: cogs,
        delivery_income_minor: delivery_income,
        gross_profit_minor: gross_profit,
        expenses_minor: expenses,
        damage_loss_minor: damage_loss,
        operational_profit_minor: operational_profit,
        owner_capital_in_minor: owner_capital_in,
        owner_withdrawals_minor: owner_withdrawals,
        cash_inflow_minor: cash_inflow,
        cash_outflow_minor: cash_outflow,
        net_cash_flow_minor: cash_inflow - cash_outflow,
    })
}
