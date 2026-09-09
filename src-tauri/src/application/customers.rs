use sqlx::sqlite::SqliteConnection;
use sqlx::Row;

use crate::application::auth::Principal;
use crate::application::cash::record_cash_entry;
use crate::application::documents::next_document_number;
use crate::application::suppliers::state_audit;
use crate::dto::sales::{
    CustomerDto, CustomerInput, CustomerLedgerEntryDto, CustomerPaymentDto,
    CustomerPaymentVoidInput, CustomerReceiptInput, SalePaymentAllocationDto,
};
use crate::error::AppError;
use crate::state::AppState;

pub(crate) async fn require_customer(
    conn: &mut SqliteConnection,
    customer_id: i64,
) -> Result<(), AppError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM customers WHERE id = ? AND is_active = 1")
            .bind(customer_id)
            .fetch_optional(conn)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!("customer {customer_id}")));
    }
    Ok(())
}

/// Current account balance: the running ledger balance, or the opening balance
/// when no entries exist yet. Positive = customer owes (due); negative = advance.
pub(crate) async fn customer_balance(
    conn: &mut SqliteConnection,
    customer_id: i64,
) -> Result<i64, AppError> {
    let latest: Option<i64> = sqlx::query_scalar(
        "SELECT balance_after_minor FROM customer_ledger_entries
         WHERE customer_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(customer_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(balance) = latest {
        return Ok(balance);
    }
    let opening: i64 =
        sqlx::query_scalar("SELECT opening_balance_minor FROM customers WHERE id = ?")
            .bind(customer_id)
            .fetch_one(&mut *conn)
            .await?;
    Ok(opening)
}

pub(crate) async fn customer_advance(
    conn: &mut SqliteConnection,
    customer_id: i64,
) -> Result<i64, AppError> {
    let balance = customer_balance(conn, customer_id).await?;
    Ok(if balance < 0 { -balance } else { 0 })
}

/// Append a signed customer-ledger entry and return the running balance after it.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn record_ledger(
    tx: &mut SqliteConnection,
    customer_id: i64,
    entry_type: &str,
    document_type: &str,
    document_id: i64,
    amount_minor: i64,
    notes: &str,
    actor_id: i64,
) -> Result<i64, AppError> {
    let prev = customer_balance(tx, customer_id).await?;
    let after = prev + amount_minor;
    sqlx::query(
        "INSERT INTO customer_ledger_entries
           (customer_id, entry_type, document_type, document_id,
            amount_minor, balance_after_minor, notes, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(customer_id)
    .bind(entry_type)
    .bind(document_type)
    .bind(document_id)
    .bind(amount_minor)
    .bind(after)
    .bind(notes)
    .bind(actor_id)
    .execute(&mut *tx)
    .await?;
    Ok(after)
}

async fn customer_dto(state: &AppState, customer_id: i64) -> Result<CustomerDto, AppError> {
    let row = sqlx::query(
        "SELECT c.id, c.code, c.name, c.phone, c.email, c.address,
                c.credit_limit_minor, c.credit_days, c.is_active, c.created_at,
                c.opening_balance_minor
         FROM customers c WHERE c.id = ?",
    )
    .bind(customer_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("customer {customer_id}")))?;

    let mut pool_conn = state.pool.acquire().await?;
    let balance = customer_balance(&mut pool_conn, customer_id).await?;
    let advance = customer_advance(&mut pool_conn, customer_id).await?;

    Ok(CustomerDto {
        id: row.get(0),
        code: row.get(1),
        name: row.get(2),
        phone: row.try_get(3).ok(),
        email: row.try_get(4).ok(),
        address: row.try_get(5).ok(),
        credit_limit_minor: row.get(6),
        credit_days: row.get(7),
        is_active: row.get::<i64, _>(8) != 0,
        created_at: row.get(9),
        balance_minor: balance,
        advance_minor: advance,
        opening_balance_minor: row.get::<i64, _>(10),
    })
}

pub async fn create(
    state: &AppState,
    principal: &Principal,
    input: CustomerInput,
    correlation_id: &str,
) -> Result<CustomerDto, AppError> {
    principal.require("customer.create")?;

    let code = input.code.trim().to_uppercase();
    if code.is_empty() {
        return Err(AppError::Validation("customer code is required".into()));
    }
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("customer name is required".into()));
    }
    let opening = input.opening_balance_minor.unwrap_or(0);
    if opening < 0 {
        return Err(AppError::Validation(
            "opening balance cannot be negative".into(),
        ));
    }
    let credit_limit = input.credit_limit_minor.unwrap_or(0);
    if credit_limit < 0 {
        return Err(AppError::Validation(
            "credit limit cannot be negative".into(),
        ));
    }
    let credit_days = input.credit_days.unwrap_or(30);
    if credit_days < 0 {
        return Err(AppError::Validation(
            "credit terms cannot be negative".into(),
        ));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let id = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let code = code.clone();
            let name = input.name.trim().to_string();
            let phone = input.phone.clone();
            let email = input.email.clone();
            let address = input.address.clone();
            Box::pin(async move {
                let existing: Option<i64> =
                    sqlx::query_scalar("SELECT 1 FROM customers WHERE code = ?")
                        .bind(&code)
                        .fetch_optional(&mut *tx)
                        .await?;
                if existing.is_some() {
                    return Err(AppError::Conflict(format!(
                        "customer code '{code}' already exists"
                    )));
                }

                let id = sqlx::query(
                    "INSERT INTO customers
                       (code, name, phone, email, address,
                        credit_limit_minor, credit_days, opening_balance_minor, created_by)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&code)
                .bind(&name)
                .bind(phone.as_deref())
                .bind(email.as_deref())
                .bind(address.as_deref())
                .bind(credit_limit)
                .bind(credit_days)
                .bind(opening)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                if opening > 0 {
                    record_ledger(
                        &mut *tx,
                        id,
                        "opening_balance",
                        "customer",
                        id,
                        opening,
                        "opening balance",
                        actor_id,
                    )
                    .await?;
                }

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "customer.create",
                    "customer",
                    id,
                    &correlation,
                    None,
                    None,
                )
                .await?;
                Ok(id)
            })
        })
        .await?;

    customer_dto(state, id).await
}

pub async fn update(
    state: &AppState,
    principal: &Principal,
    customer_id: i64,
    input: CustomerInput,
    correlation_id: &str,
) -> Result<CustomerDto, AppError> {
    principal.require("customer.create")?;

    let code = input.code.trim().to_uppercase();
    if code.is_empty() {
        return Err(AppError::Validation("customer code is required".into()));
    }
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("customer name is required".into()));
    }
    let credit_limit = input.credit_limit_minor.unwrap_or(0);
    if credit_limit < 0 {
        return Err(AppError::Validation(
            "credit limit cannot be negative".into(),
        ));
    }
    let credit_days = input.credit_days.unwrap_or(30);
    if credit_days < 0 {
        return Err(AppError::Validation(
            "credit terms cannot be negative".into(),
        ));
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
                let conflict: Option<i64> =
                    sqlx::query_scalar("SELECT 1 FROM customers WHERE code = ? AND id != ?")
                        .bind(&code)
                        .bind(customer_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                if conflict.is_some() {
                    return Err(AppError::Conflict(format!(
                        "customer code '{code}' already exists"
                    )));
                }
                let updated = sqlx::query(
                    "UPDATE customers
                        SET code = ?, name = ?, phone = ?, email = ?, address = ?,
                            credit_limit_minor = ?, credit_days = ?, is_active = ?,
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(&code)
                .bind(&name)
                .bind(phone.as_deref())
                .bind(email.as_deref())
                .bind(address.as_deref())
                .bind(credit_limit)
                .bind(credit_days)
                .bind(if is_active { 1 } else { 0 })
                .bind(customer_id)
                .execute(&mut *tx)
                .await?;
                if updated.rows_affected() == 0 {
                    return Err(AppError::NotFound(format!("customer {customer_id}")));
                }

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "customer.update",
                    "customer",
                    customer_id,
                    &correlation,
                    None,
                    None,
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    customer_dto(state, customer_id).await
}

pub async fn list(state: &AppState, principal: &Principal) -> Result<Vec<CustomerDto>, AppError> {
    principal.require("customer.view")?;
    let rows = sqlx::query(
        "SELECT c.id, c.code, c.name, c.phone, c.email, c.address,
                c.credit_limit_minor, c.credit_days, c.is_active, c.created_at,
                c.opening_balance_minor,
                COALESCE((SELECT balance_after_minor FROM customer_ledger_entries e
                           WHERE e.customer_id = c.id ORDER BY id DESC LIMIT 1),
                         c.opening_balance_minor) AS balance_minor
         FROM customers c ORDER BY c.name",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let balance = r.get::<i64, _>(11);
            CustomerDto {
                id: r.get(0),
                code: r.get(1),
                name: r.get(2),
                phone: r.try_get(3).ok(),
                email: r.try_get(4).ok(),
                address: r.try_get(5).ok(),
                credit_limit_minor: r.get(6),
                credit_days: r.get(7),
                is_active: r.get::<i64, _>(8) != 0,
                created_at: r.get(9),
                opening_balance_minor: r.get::<i64, _>(10),
                balance_minor: balance,
                advance_minor: i64::max(0, -balance),
            }
        })
        .collect())
}

pub async fn get(
    state: &AppState,
    principal: &Principal,
    customer_id: i64,
) -> Result<CustomerDto, AppError> {
    principal.require("customer.view")?;
    customer_dto(state, customer_id).await
}

pub async fn ledger(
    state: &AppState,
    principal: &Principal,
    customer_id: i64,
) -> Result<Vec<CustomerLedgerEntryDto>, AppError> {
    principal.require("customer.view")?;
    let mut conn = state.pool.acquire().await?;
    require_customer(&mut conn, customer_id).await?;
    let rows = sqlx::query(
        "SELECT id, entry_type, document_type, document_id, amount_minor,
                balance_after_minor, notes, created_by, created_at
         FROM customer_ledger_entries WHERE customer_id = ? ORDER BY id DESC",
    )
    .bind(customer_id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| CustomerLedgerEntryDto {
            id: r.get(0),
            entry_type: r.get(1),
            document_type: r.try_get(2).ok(),
            document_id: r.try_get(3).ok(),
            amount_minor: r.get(4),
            balance_after_minor: r.get(5),
            notes: r.try_get(6).ok(),
            created_by: r.get(7),
            created_at: r.get(8),
        })
        .collect())
}

async fn customer_payment_dto(
    state: &AppState,
    payment_id: i64,
) -> Result<CustomerPaymentDto, AppError> {
    let row = sqlx::query(
        "SELECT p.id, p.receipt_number, p.customer_id, c.name,
                p.sale_id, p.payment_method_id, m.name, p.cash_account_id, a.name,
                p.payment_date, p.amount_minor, p.advance_alloc_minor, p.status,
                p.notes, p.created_at, p.voided_at
         FROM customer_payments p
         JOIN customers c ON c.id = p.customer_id
         JOIN payment_methods m ON m.id = p.payment_method_id
         JOIN cash_accounts a ON a.id = p.cash_account_id
         WHERE p.id = ?",
    )
    .bind(payment_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("customer payment {payment_id}")))?;

    let allocations: Vec<SalePaymentAllocationDto> = sqlx::query(
        "SELECT a.sale_id, s.sale_number, a.amount_minor
         FROM customer_payment_allocations a
         JOIN sales s ON s.id = a.sale_id
         WHERE a.payment_id = ? ORDER BY a.id",
    )
    .bind(payment_id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| SalePaymentAllocationDto {
        sale_id: r.get(0),
        sale_number: r.get(1),
        amount_minor: r.get(2),
    })
    .collect();

    Ok(CustomerPaymentDto {
        id: row.get(0),
        receipt_number: row.get(1),
        customer_id: row.get(2),
        customer_name: row.get(3),
        sale_id: row.try_get(4).ok(),
        payment_method_id: row.get(5),
        payment_method_name: row.get(6),
        cash_account_id: row.get(7),
        cash_account_name: row.get(8),
        payment_date: row.get(9),
        amount_minor: row.get(10),
        advance_alloc_minor: row.get(11),
        status: row.get(12),
        notes: row.try_get(13).ok(),
        created_at: row.get(14),
        voided_at: row.try_get(15).ok(),
        allocations,
    })
}

/// Receive money from a customer: allocated oldest-first across open confirmed
/// sales, with any surplus recorded as an advance on the customer's account.
pub async fn create_receipt(
    state: &AppState,
    principal: &Principal,
    input: CustomerReceiptInput,
    correlation_id: &str,
) -> Result<CustomerPaymentDto, AppError> {
    principal.require("payment.receive")?;

    if input.amount_minor <= 0 {
        return Err(AppError::Validation(
            "payment amount must be positive".into(),
        ));
    }
    if input.payment_date.trim().is_empty() {
        return Err(AppError::Validation("payment date is required".into()));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let payment_id = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let idempotency_key = input.idempotency_key.clone();
            let notes = input.notes.clone();
            let payment_date = input.payment_date.clone();
            Box::pin(async move {
                require_customer(&mut *tx, input.customer_id).await?;

                let account: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM cash_accounts WHERE id = ? AND is_active = 1",
                )
                .bind(input.cash_account_id)
                .fetch_optional(&mut *tx)
                .await?;
                if account.is_none() {
                    return Err(AppError::NotFound(format!(
                        "cash account {}",
                        input.cash_account_id
                    )));
                }
                let method: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM payment_methods WHERE id = ? AND is_active = 1",
                )
                .bind(input.payment_method_id)
                .fetch_optional(&mut *tx)
                .await?;
                if method.is_none() {
                    return Err(AppError::NotFound(format!(
                        "payment method {}",
                        input.payment_method_id
                    )));
                }

                let existing_id: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM customer_payments WHERE idempotency_key = ?",
                )
                .bind(idempotency_key.as_deref().filter(|k| !k.trim().is_empty()))
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(id) = existing_id {
                    return Ok(id);
                }

                let numbered = next_document_number(&mut *tx, "customer_payment").await?;
                let pay_id = sqlx::query(
                    "INSERT INTO customer_payments
                       (receipt_number, customer_id, payment_method_id, cash_account_id,
                        payment_date, amount_minor, advance_alloc_minor, status,
                        notes, idempotency_key, created_by)
                     VALUES (?, ?, ?, ?, ?, ?, ?, 'posted', ?, ?, ?)",
                )
                .bind(&numbered)
                .bind(input.customer_id)
                .bind(input.payment_method_id)
                .bind(input.cash_account_id)
                .bind(&payment_date)
                .bind(input.amount_minor)
                .bind(0i64)
                .bind(notes.as_deref())
                .bind(idempotency_key.as_deref().filter(|k| !k.trim().is_empty()))
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                let advance = match input.allocations.as_ref() {
                    Some(allocs) => {
                        // Explicit allocation list is authoritative: a zero
                        // amount for a sale leaves that invoice untouched, and
                        // the unallocated remainder becomes an advance. An
                        // empty (or fully zeroed) list therefore holds the
                        // whole amount as an advance rather than auto-applying
                        // it oldest-first.
                        let alloc_sum: i64 = allocs.iter().map(|a| a.amount_minor).sum();
                        if alloc_sum > input.amount_minor {
                            return Err(AppError::Validation(format!(
                                "allocations {alloc_sum} exceed payment amount {}",
                                input.amount_minor
                            )));
                        }
                        let mut seen = std::collections::HashSet::new();
                        for alloc in allocs {
                            if alloc.amount_minor <= 0 {
                                continue;
                            }
                            if !seen.insert(alloc.sale_id) {
                                return Err(AppError::Validation(format!(
                                    "duplicate allocation for sale {}",
                                    alloc.sale_id
                                )));
                            }
                            let sale: Option<(i64, String, i64)> = sqlx::query_as(
                                "SELECT customer_id, status, due_minor FROM sales WHERE id = ?",
                            )
                            .bind(alloc.sale_id)
                            .fetch_optional(&mut *tx)
                            .await?;
                            let Some((cid, status, due)) = sale else {
                                return Err(AppError::NotFound(format!("sale {}", alloc.sale_id)));
                            };
                            if cid != input.customer_id {
                                return Err(AppError::Validation(format!(
                                    "sale {} does not belong to this customer",
                                    alloc.sale_id
                                )));
                            }
                            if status != "confirmed" {
                                return Err(AppError::Conflict(format!(
                                    "sale {} is not a confirmed invoice",
                                    alloc.sale_id
                                )));
                            }
                            if alloc.amount_minor > due {
                                return Err(AppError::Validation(format!(
                                    "allocation {} exceeds the due amount {due} on sale {}",
                                    alloc.amount_minor, alloc.sale_id
                                )));
                            }
                        }
                        for alloc in allocs {
                            if alloc.amount_minor <= 0 {
                                continue;
                            }
                            sqlx::query(
                                "UPDATE sales
                                    SET paid_minor = paid_minor + ?, due_minor = due_minor - ?,
                                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                                  WHERE id = ?",
                            )
                            .bind(alloc.amount_minor)
                            .bind(alloc.amount_minor)
                            .bind(alloc.sale_id)
                            .execute(&mut *tx)
                            .await?;
                            sqlx::query(
                                "INSERT INTO customer_payment_allocations
                                   (payment_id, sale_id, amount_minor)
                                 VALUES (?, ?, ?)",
                            )
                            .bind(pay_id)
                            .bind(alloc.sale_id)
                            .bind(alloc.amount_minor)
                            .execute(&mut *tx)
                            .await?;
                        }
                        input.amount_minor - alloc_sum
                    }
                    None => {
                        let open: Vec<i64> = sqlx::query_scalar(
                            "SELECT id FROM sales
                             WHERE customer_id = ? AND status = 'confirmed' AND due_minor > 0
                             ORDER BY confirmed_at ASC, id ASC",
                        )
                        .bind(input.customer_id)
                        .fetch_all(&mut *tx)
                        .await?;

                        let mut remaining = input.amount_minor;
                        for sale_id in open {
                            if remaining <= 0 {
                                break;
                            }
                            let due: i64 =
                                sqlx::query_scalar("SELECT due_minor FROM sales WHERE id = ?")
                                    .bind(sale_id)
                                    .fetch_one(&mut *tx)
                                    .await?;
                            let alloc = std::cmp::min(due, remaining);
                            sqlx::query(
                                "UPDATE sales
                                    SET paid_minor = paid_minor + ?, due_minor = due_minor - ?,
                                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                                  WHERE id = ?",
                            )
                            .bind(alloc)
                            .bind(alloc)
                            .bind(sale_id)
                            .execute(&mut *tx)
                            .await?;
                            sqlx::query(
                                "INSERT INTO customer_payment_allocations
                                   (payment_id, sale_id, amount_minor)
                                 VALUES (?, ?, ?)",
                            )
                            .bind(pay_id)
                            .bind(sale_id)
                            .bind(alloc)
                            .execute(&mut *tx)
                            .await?;
                            remaining -= alloc;
                        }
                        remaining
                    }
                };

                sqlx::query("UPDATE customer_payments SET advance_alloc_minor = ? WHERE id = ?")
                    .bind(advance)
                    .bind(pay_id)
                    .execute(&mut *tx)
                    .await?;

                record_cash_entry(
                    &mut *tx,
                    input.cash_account_id,
                    "customer_receipt",
                    input.amount_minor,
                    "customer_payment",
                    pay_id,
                    &format!("receipt {numbered}"),
                    actor_id,
                )
                .await?;

                record_ledger(
                    &mut *tx,
                    input.customer_id,
                    "payment",
                    "customer_payment",
                    pay_id,
                    -input.amount_minor,
                    &format!("receipt {numbered}"),
                    actor_id,
                )
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "customer.payment.receive",
                    "customer_payment",
                    pay_id,
                    &correlation,
                    None,
                    None,
                )
                .await?;
                Ok(pay_id)
            })
        })
        .await?;

    customer_payment_dto(state, payment_id).await
}

pub async fn void_receipt(
    state: &AppState,
    principal: &Principal,
    input: CustomerPaymentVoidInput,
    correlation_id: &str,
) -> Result<CustomerPaymentDto, AppError> {
    principal.require("payment.receive")?;

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = input.reason.clone().unwrap_or_default();
            Box::pin(async move {
                let payment: Option<(i64, i64, i64, String, String)> = sqlx::query_as(
                    "SELECT customer_id, cash_account_id, amount_minor, status, receipt_number
                     FROM customer_payments WHERE id = ?",
                )
                .bind(input.payment_id)
                .fetch_optional(&mut *tx)
                .await?;
                let Some((customer_id, cash_account_id, amount_minor, status, receipt_number)) =
                    payment
                else {
                    return Err(AppError::NotFound(format!(
                        "customer payment {}",
                        input.payment_id
                    )));
                };
                if status != "posted" {
                    return Err(AppError::Conflict(format!(
                        "receipt {} is already voided",
                        input.payment_id
                    )));
                }

                let allocations: Vec<(i64, i64)> = sqlx::query_as(
                    "SELECT sale_id, amount_minor FROM customer_payment_allocations
                     WHERE payment_id = ?",
                )
                .bind(input.payment_id)
                .fetch_all(&mut *tx)
                .await?;
                for (sale_id, alloc) in allocations {
                    sqlx::query(
                        "UPDATE sales
                            SET paid_minor = MAX(paid_minor - ?, 0), due_minor = due_minor + ?,
                                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                          WHERE id = ?",
                    )
                    .bind(alloc)
                    .bind(alloc)
                    .bind(sale_id)
                    .execute(&mut *tx)
                    .await?;
                }

                record_cash_entry(
                    &mut *tx,
                    cash_account_id,
                    "receipt_void",
                    -amount_minor,
                    "customer_payment",
                    input.payment_id,
                    &format!("void {receipt_number}: {reason}"),
                    actor_id,
                )
                .await?;

                record_ledger(
                    &mut *tx,
                    customer_id,
                    "payment_refund",
                    "customer_payment",
                    input.payment_id,
                    amount_minor,
                    &format!("void {receipt_number}: {reason}"),
                    actor_id,
                )
                .await?;

                sqlx::query(
                    "UPDATE customer_payments
                        SET status = 'voided', voided_by = ?, voided_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(actor_id)
                .bind(input.payment_id)
                .execute(&mut *tx)
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "customer.payment.void",
                    "customer_payment",
                    input.payment_id,
                    &correlation,
                    None,
                    None,
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    customer_payment_dto(state, input.payment_id).await
}

pub async fn list_receipts(
    state: &AppState,
    principal: &Principal,
    customer_id: i64,
) -> Result<Vec<CustomerPaymentDto>, AppError> {
    principal.require("payment.receive")?;
    let mut conn = state.pool.acquire().await?;
    require_customer(&mut conn, customer_id).await?;
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM customer_payments WHERE customer_id = ? ORDER BY id DESC",
    )
    .bind(customer_id)
    .fetch_all(&mut *conn)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for id in rows {
        out.push(customer_payment_dto(state, id).await?);
    }
    Ok(out)
}
