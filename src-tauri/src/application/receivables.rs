use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::application::auth::Principal;
use crate::application::customers::require_customer;
use crate::dto::receivables::{
    CustomerReceiptPreviewInput, CustomerStatementDto, CustomerStatementInput,
    ReceiptAllocationPreviewDto, ReceiptPreviewDto, ReceivableCustomerDto, ReceivableSaleDto,
    ReceivablesDto,
};
use crate::dto::sales::CustomerLedgerEntryDto;
use crate::dto::PdfResultDto;
use crate::error::AppError;
use crate::infrastructure::{generate_receipt_pdf, ReceiptAllocationLine, ReceiptRecord};
use crate::state::AppState;

/// SQLite decodes a NULL TEXT as an empty string; obtain the optional value
/// safely and collapse both representations to `None`.
fn opt_str(r: &SqliteRow, idx: usize) -> Option<String> {
    r.try_get::<Option<String>, _>(idx)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
}

/// Statement opening balance: the running balance of the last ledger entry
/// recorded strictly before `from_date`, else the customer's opening balance
/// (a non-zero opening balance is itself seeded as a ledger entry).
async fn statement_opening(
    conn: &mut sqlx::SqliteConnection,
    customer_id: i64,
    from_date: &str,
) -> Result<i64, AppError> {
    let opening: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(
            (SELECT balance_after_minor FROM customer_ledger_entries e
              WHERE e.customer_id = c.id AND date(e.created_at) < date(?)
              ORDER BY e.id DESC LIMIT 1),
            c.opening_balance_minor)
         FROM customers c WHERE c.id = ?",
    )
    .bind(from_date)
    .bind(customer_id)
    .fetch_one(conn)
    .await?;
    Ok(opening.unwrap_or(0))
}

/// Customer account statement for a date range. The opening balance is the
/// ledger running balance just before `from_date`, and every entry carries its
/// stored running balance, so the closing balance reconciles exactly against
/// the underlying ledger for any tested range.
pub async fn customer_statement(
    state: &AppState,
    principal: &Principal,
    input: CustomerStatementInput,
) -> Result<CustomerStatementDto, AppError> {
    principal.require("customer.view")?;
    let from_date = input.from_date.trim().to_string();
    let to_date = input.to_date.trim().to_string();
    if from_date.is_empty() || to_date.is_empty() {
        return Err(AppError::Validation(
            "statement date range is required".into(),
        ));
    }
    if from_date > to_date {
        return Err(AppError::Validation(
            "statement from-date cannot be after to-date".into(),
        ));
    }

    let mut conn = state.pool.acquire().await?;
    require_customer(&mut conn, input.customer_id).await?;
    let customer_name: String = sqlx::query_scalar("SELECT name FROM customers WHERE id = ?")
        .bind(input.customer_id)
        .fetch_one(&mut *conn)
        .await?;

    let opening = statement_opening(&mut conn, input.customer_id, &from_date).await?;

    let rows = sqlx::query(
        "SELECT id, entry_type, document_type, document_id, amount_minor,
                balance_after_minor, notes, created_by, created_at
         FROM customer_ledger_entries
         WHERE customer_id = ?
           AND date(created_at) BETWEEN date(?) AND date(?)
         ORDER BY id ASC",
    )
    .bind(input.customer_id)
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(&mut *conn)
    .await?;

    let entries: Vec<CustomerLedgerEntryDto> = rows
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
        .collect();

    let closing = entries
        .last()
        .map(|e| e.balance_after_minor)
        .unwrap_or(opening);

    Ok(CustomerStatementDto {
        customer_id: input.customer_id,
        customer_name,
        from_date,
        to_date,
        opening_balance_minor: opening,
        closing_balance_minor: closing,
        entries,
    })
}

/// Preview how a receipt amount would be split if posted right now: applied
/// oldest-first across open confirmed invoices with the remainder as advance.
/// Mirrors the automatic allocation path in `customers::create_receipt` so the
/// preview always matches what posting would produce.
pub async fn receipt_preview(
    state: &AppState,
    principal: &Principal,
    input: CustomerReceiptPreviewInput,
) -> Result<ReceiptPreviewDto, AppError> {
    principal.require("payment.receive")?;
    if input.amount_minor <= 0 {
        return Err(AppError::Validation(
            "preview amount must be positive".into(),
        ));
    }

    let mut conn = state.pool.acquire().await?;
    require_customer(&mut conn, input.customer_id).await?;
    let customer_name: String = sqlx::query_scalar("SELECT name FROM customers WHERE id = ?")
        .bind(input.customer_id)
        .fetch_one(&mut *conn)
        .await?;

    let rows = sqlx::query(
        "SELECT id, sale_number, sale_date, due_date, total_minor, due_minor
         FROM sales
         WHERE customer_id = ? AND status = 'confirmed' AND due_minor > 0
         ORDER BY confirmed_at ASC, id ASC LIMIT 500",
    )
    .bind(input.customer_id)
    .fetch_all(&mut *conn)
    .await?;

    let mut remaining = input.amount_minor;
    let mut allocations = Vec::new();
    for r in rows {
        if remaining <= 0 {
            break;
        }
        let due: i64 = r.get(5);
        let allocated = std::cmp::min(due, remaining);
        allocations.push(ReceiptAllocationPreviewDto {
            sale_id: r.get(0),
            sale_number: opt_str(&r, 1),
            sale_date: r.get(2),
            due_date: opt_str(&r, 3),
            total_minor: r.get(4),
            due_minor: due,
            allocated_minor: allocated,
        });
        remaining -= allocated;
    }

    Ok(ReceiptPreviewDto {
        customer_id: input.customer_id,
        customer_name,
        amount_minor: input.amount_minor,
        allocations,
        advance_minor: remaining,
    })
}

/// Phase 7 due-control snapshot for the exception screens: overdue invoices,
/// invoices due within the next seven days, the largest open balances, and
/// customers whose account balance exceeds their credit limit.
pub async fn receivables(
    state: &AppState,
    principal: &Principal,
) -> Result<ReceivablesDto, AppError> {
    principal.require_any(&["payment.receive", "customer.view"])?;

    let overdue: Vec<ReceivableSaleDto> = sqlx::query(
        "SELECT s.id, s.sale_number, s.customer_id, s.customer_name, s.sale_date,
                s.due_date, s.total_minor, s.paid_minor, s.advance_used_minor, s.due_minor,
                CAST(julianday(date('now')) - julianday(s.due_date) AS INTEGER)
         FROM sales s
         WHERE s.status = 'confirmed' AND s.due_minor > 0
           AND s.due_date IS NOT NULL AND s.due_date < date('now')
         ORDER BY s.due_date ASC, s.id ASC LIMIT 500",
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| ReceivableSaleDto {
        sale_id: r.get(0),
        sale_number: opt_str(&r, 1),
        customer_id: r.get(2),
        customer_name: r.get(3),
        sale_date: r.get(4),
        due_date: opt_str(&r, 5),
        total_minor: r.get(6),
        paid_minor: r.get(7),
        advance_used_minor: r.get(8),
        due_minor: r.get(9),
        days: r.get(10),
    })
    .collect();

    let due_soon: Vec<ReceivableSaleDto> = sqlx::query(
        "SELECT s.id, s.sale_number, s.customer_id, s.customer_name, s.sale_date,
                s.due_date, s.total_minor, s.paid_minor, s.advance_used_minor, s.due_minor,
                CAST(julianday(date(s.due_date)) - julianday(date('now')) AS INTEGER)
         FROM sales s
         WHERE s.status = 'confirmed' AND s.due_minor > 0
           AND s.due_date IS NOT NULL
           AND date(s.due_date) BETWEEN date('now') AND date('now', '+7 days')
         ORDER BY s.due_date ASC, s.id ASC LIMIT 500",
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| ReceivableSaleDto {
        sale_id: r.get(0),
        sale_number: opt_str(&r, 1),
        customer_id: r.get(2),
        customer_name: r.get(3),
        sale_date: r.get(4),
        due_date: opt_str(&r, 5),
        total_minor: r.get(6),
        paid_minor: r.get(7),
        advance_used_minor: r.get(8),
        due_minor: r.get(9),
        days: r.get(10),
    })
    .collect();

    let balances: Vec<ReceivableCustomerDto> = sqlx::query(
        "SELECT c.id, c.name, c.phone,
                COALESCE((SELECT balance_after_minor FROM customer_ledger_entries e
                           WHERE e.customer_id = c.id ORDER BY e.id DESC LIMIT 1),
                         c.opening_balance_minor) AS balance_minor,
                c.credit_limit_minor,
                COALESCE(SUM(CASE WHEN s.status = 'confirmed' AND s.due_minor > 0
                                  THEN s.due_minor END), 0) AS due_total,
                COALESCE(SUM(CASE WHEN s.status = 'confirmed' AND s.due_minor > 0
                                    AND s.due_date IS NOT NULL AND s.due_date < date('now')
                                  THEN s.due_minor END), 0) AS overdue_total
         FROM customers c
         LEFT JOIN sales s ON s.customer_id = c.id
         WHERE c.is_active = 1
         GROUP BY c.id
         HAVING balance_minor > 0
         ORDER BY balance_minor DESC",
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| ReceivableCustomerDto {
        customer_id: r.get(0),
        customer_name: r.get(1),
        phone: opt_str(&r, 2),
        balance_minor: r.get(3),
        credit_limit_minor: r.get(4),
        due_minor_total: r.get(5),
        overdue_minor_total: r.get(6),
    })
    .collect();

    let high_balance: Vec<ReceivableCustomerDto> = balances.iter().take(20).cloned().collect();
    let mut exceptions: Vec<ReceivableCustomerDto> = balances
        .into_iter()
        .filter(|b| b.balance_minor > b.credit_limit_minor)
        .collect();
    exceptions.sort_by_key(|b| b.credit_limit_minor - b.balance_minor);

    Ok(ReceivablesDto {
        overdue,
        due_soon,
        high_balance,
        credit_limit_exceptions: exceptions,
    })
}

/// Printable payment receipt, mirroring the invoice report flow.
pub async fn customer_receipt_pdf(
    state: &AppState,
    principal: &Principal,
    payment_id: i64,
) -> Result<PdfResultDto, AppError> {
    principal.require("payment.receive")?;

    let row = sqlx::query(
        "SELECT p.id, p.receipt_number, p.customer_id, c.name,
                p.payment_date, p.amount_minor, p.advance_alloc_minor, p.status,
                m.name, a.name
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

    let status: String = row.get(7);
    if status != "posted" {
        return Err(AppError::Validation(
            "only posted receipts can be printed".into(),
        ));
    }

    let allocations: Vec<ReceiptAllocationLine> = sqlx::query(
        "SELECT a.sale_id, s.sale_number, a.amount_minor
         FROM customer_payment_allocations a
         JOIN sales s ON s.id = a.sale_id
         WHERE a.payment_id = ? ORDER BY a.id",
    )
    .bind(payment_id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| ReceiptAllocationLine {
        sale_id: r.get(0),
        sale_number: opt_str(&r, 1),
        amount_minor: r.get(2),
    })
    .collect();

    let shop_name: String =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'shop.name'")
            .fetch_optional(&state.pool)
            .await?
            .and_then(|v: String| serde_json::from_str::<serde_json::Value>(&v).ok())
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "Furniture Shop".into());

    let shop_address = sqlx::query_scalar::<_, String>(
        "SELECT value_json FROM settings WHERE key = 'shop.address'",
    )
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()
    .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok())
    .and_then(|v| v.as_str().map(str::to_string));

    let record = ReceiptRecord {
        receipt_number: row.get::<Option<String>, _>(1).unwrap_or_default(),
        payment_date: row.get(4),
        customer_name: row.get(3),
        method: row.get(8),
        cash_account: row.get(9),
        amount_minor: row.get(5),
        advance_minor: row.get(6),
        allocations,
        shop_name,
        shop_address,
    };

    let reports_dir = state.paths.reports_dir.clone();
    let pdf = tokio::task::spawn_blocking(move || generate_receipt_pdf(&reports_dir, &record))
        .await
        .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    Ok(PdfResultDto {
        report_path: pdf.path,
        pages: pdf.pages,
        bytes: pdf.bytes,
    })
}
