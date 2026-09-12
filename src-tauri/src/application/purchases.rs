use sqlx::sqlite::SqliteConnection;
use sqlx::Row;

use crate::application::auth::Principal;
use crate::application::cash::{record_cash_entry, require_cash_balance};
use crate::application::documents::{next_document_number, replay_guard};
use crate::application::inventory::{
    apply_on_hand_delta, next_move_seq, post_cost_layer, require_active_location, require_positive,
    withdraw_cost_layers,
};
use crate::application::suppliers::{require_supplier, state_audit, supplier_balance};
use crate::dto::purchases::{
    PayableAgingRowDto, PaymentAllocationDto, PurchaseCreateInput, PurchaseDto, PurchaseItemDto,
    PurchasePostInput, SupplierPaymentDto, SupplierPaymentInput, SupplierPaymentVoidInput,
    SupplierReturnCreateInput, SupplierReturnDto, SupplierReturnItemDto, SupplierReturnPostInput,
};
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Append a signed supplier-ledger entry and return the running balance after it.
#[allow(clippy::too_many_arguments)]
async fn record_ledger(
    tx: &mut SqliteConnection,
    supplier_id: i64,
    entry_type: &str,
    document_type: &str,
    document_id: i64,
    amount_minor: i64,
    notes: &str,
    actor_id: i64,
) -> Result<i64, AppError> {
    let prev = supplier_balance(tx, supplier_id).await?;
    let after = prev + amount_minor;
    sqlx::query(
        "INSERT INTO supplier_ledger_entries
           (supplier_id, entry_type, document_type, document_id,
            amount_minor, balance_after_minor, notes, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(supplier_id)
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

// ---------------------------------------------------------------------------
// Purchases
// ---------------------------------------------------------------------------

fn map_purchase_item(r: &sqlx::sqlite::SqliteRow) -> PurchaseItemDto {
    PurchaseItemDto {
        product_id: r.get(1),
        article_number: r.get(2),
        product_name: r.get(3),
        quantity: r.get(4),
        unit_cost_minor: r.get(5),
        line_total_minor: r.get(6),
    }
}

async fn purchase_dto(state: &AppState, purchase_id: i64) -> Result<PurchaseDto, AppError> {
    let row = sqlx::query(
        "SELECT p.id, p.purchase_number, p.supplier_id, p.supplier_name,
                p.location_id, p.invoice_number, p.invoice_date, p.purchase_date,
                p.status, p.total_minor, p.paid_minor, p.due_minor, p.notes,
                p.created_at, p.posted_at
         FROM purchases p WHERE p.id = ?",
    )
    .bind(purchase_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("purchase {purchase_id}")))?;

    let item_rows = sqlx::query(
        "SELECT id, product_id, article_number, product_name, quantity, unit_cost_minor, line_total_minor
         FROM purchase_items WHERE purchase_id = ? ORDER BY id",
    )
    .bind(purchase_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(PurchaseDto {
        id: row.get(0),
        purchase_number: row.get(1),
        supplier_id: row.get(2),
        supplier_name: row.get(3),
        location_id: row.get(4),
        invoice_number: row.get(5),
        invoice_date: row.get(6),
        purchase_date: row.get(7),
        status: row.get(8),
        total_minor: row.get(9),
        paid_minor: row.get(10),
        due_minor: row.get(11),
        notes: row.get(12),
        items: item_rows.iter().map(map_purchase_item).collect(),
        created_at: row.get(13),
        posted_at: row.get(14),
    })
}

pub async fn create_purchase(
    state: &AppState,
    principal: &Principal,
    input: PurchaseCreateInput,
    correlation_id: &str,
) -> Result<PurchaseDto, AppError> {
    principal.require("purchase.create")?;

    if input.invoice_number.trim().is_empty() {
        return Err(AppError::Validation(
            "supplier invoice number is required".into(),
        ));
    }
    if input.items.is_empty() {
        return Err(AppError::Validation(
            "purchase must have at least one item".into(),
        ));
    }
    for item in &input.items {
        if item.quantity <= 0 {
            return Err(AppError::Validation(
                "purchase quantity must be positive".into(),
            ));
        }
        if item.unit_cost_minor < 0 {
            return Err(AppError::Validation("unit cost cannot be negative".into()));
        }
    }

    let invoice_number = input.invoice_number.trim().to_string();
    let invoice_date = input.invoice_date.clone();
    let purchase_date = input
        .purchase_date
        .clone()
        .unwrap_or_else(|| input.invoice_date.clone());
    let notes = input.notes.clone();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let invoice_number = invoice_number.clone();
            let invoice_date = invoice_date.clone();
            let purchase_date = purchase_date.clone();
            let notes = notes.clone();
            Box::pin(async move {
                require_supplier(&mut *tx, input.supplier_id).await?;
                require_active_location(&mut *tx, input.location_id).await?;

                let dup: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM purchases WHERE supplier_id = ? AND invoice_number = ?",
                )
                .bind(input.supplier_id)
                .bind(&invoice_number)
                .fetch_optional(&mut *tx)
                .await?;
                if dup.is_some() {
                    return Err(AppError::Conflict(format!(
                        "supplier invoice '{invoice_number}' is already recorded"
                    )));
                }

                let supplier_name: String =
                    sqlx::query_scalar("SELECT name FROM suppliers WHERE id = ?")
                        .bind(input.supplier_id)
                        .fetch_one(&mut *tx)
                        .await?;

                let mut total: i64 = 0;
                for item in &input.items {
                    require_positive(&mut *tx, item.product_id).await?;
                    total = total
                        .checked_add(item.quantity.saturating_mul(item.unit_cost_minor))
                        .ok_or_else(|| AppError::Validation("purchase total overflow".into()))?;
                }

                let purchase_id = sqlx::query(
                    "INSERT INTO purchases
                       (supplier_id, supplier_name, location_id, invoice_number, invoice_date,
                        purchase_date, status, total_minor, notes, created_by)
                     VALUES (?, ?, ?, ?, ?, ?, 'draft', ?, ?, ?)",
                )
                .bind(input.supplier_id)
                .bind(&supplier_name)
                .bind(input.location_id)
                .bind(&invoice_number)
                .bind(&invoice_date)
                .bind(&purchase_date)
                .bind(total)
                .bind(notes.as_deref())
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                for item in &input.items {
                    let product: Option<(String, String)> =
                        sqlx::query_as("SELECT article_number, name FROM products WHERE id = ?")
                            .bind(item.product_id)
                            .fetch_optional(&mut *tx)
                            .await?;
                    let Some((article_number, product_name)) = product else {
                        return Err(AppError::NotFound(format!("product {}", item.product_id)));
                    };
                    let line_total = item.quantity.saturating_mul(item.unit_cost_minor);
                    sqlx::query(
                        "INSERT INTO purchase_items
                           (purchase_id, product_id, article_number, product_name,
                            quantity, unit_cost_minor, line_total_minor)
                         VALUES (?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(purchase_id)
                    .bind(item.product_id)
                    .bind(&article_number)
                    .bind(&product_name)
                    .bind(item.quantity)
                    .bind(item.unit_cost_minor)
                    .bind(line_total)
                    .execute(&mut *tx)
                    .await?;
                }

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "purchase.create",
                    "purchase",
                    purchase_id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "supplier_id": input.supplier_id,
                        "invoice_number": invoice_number,
                        "total_minor": total,
                        "item_count": input.items.len(),
                    })),
                )
                .await?;
                Ok(purchase_id)
            })
        })
        .await?;

    purchase_dto(state, result).await
}

pub async fn post_purchase(
    state: &AppState,
    principal: &Principal,
    input: PurchasePostInput,
    correlation_id: &str,
) -> Result<PurchaseDto, AppError> {
    principal.require("purchase.create")?;

    let paid = input.paid_minor.unwrap_or(0);
    if paid < 0 {
        return Err(AppError::Validation(
            "paid amount cannot be negative".into(),
        ));
    }
    if paid > 0 && (input.cash_account_id.is_none() || input.payment_method_id.is_none()) {
        return Err(AppError::Validation(
            "cash account and payment method are required for a paid purchase".into(),
        ));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let idempotency_key = input.idempotency_key.clone();
            Box::pin(async move {
                let purchase: Option<(String, i64, i64, String)> = sqlx::query_as(
                    "SELECT status, supplier_id, location_id, purchase_date FROM purchases WHERE id = ?",
                )
                .bind(input.purchase_id)
                .fetch_optional(&mut *tx)
                .await?;
                let Some((status, supplier_id, location_id, purchase_date)) = purchase else {
                    return Err(AppError::NotFound(format!("purchase {}", input.purchase_id)));
                };
                if replay_guard(&mut *tx, "purchases", input.purchase_id, &idempotency_key).await? {
                    return Ok(input.purchase_id);
                }
                if status != "draft" {
                    return Err(AppError::Conflict(format!(
                        "purchase {} is not a draft",
                        input.purchase_id
                    )));
                }

                require_supplier(&mut *tx, supplier_id).await?;
                require_active_location(&mut *tx, location_id).await?;

                let items: Vec<(i64, i64, i64)> = sqlx::query_as(
                    "SELECT product_id, quantity, unit_cost_minor
                     FROM purchase_items WHERE purchase_id = ? ORDER BY id",
                )
                .bind(input.purchase_id)
                .fetch_all(&mut *tx)
                .await?;
                if items.is_empty() {
                    return Err(AppError::Validation(
                        "purchase has no items to post".into(),
                    ));
                }

                let mut total: i64 = 0;
                for (product_id, quantity, unit_cost) in &items {
                    require_positive(&mut *tx, *product_id).await?;
                    total = total
                        .checked_add(quantity.saturating_mul(*unit_cost))
                        .ok_or_else(|| AppError::Validation("purchase total overflow".into()))?;
                }
                if paid > total {
                    return Err(AppError::Validation(
                        "paid amount cannot exceed the purchase total".into(),
                    ));
                }

                let number = next_document_number(&mut *tx, "purchase").await?;
                let reason = format!("purchase {number}");

                for (product_id, quantity, unit_cost) in &items {
                    let seq = next_move_seq(&mut *tx, location_id).await?;
                    let move_number = format!("PUR-{seq:06}");
                    let movement_id = sqlx::query(
                        "INSERT INTO stock_movements
                           (product_id, location_id, movement_type, quantity_delta,
                            move_number, unit_cost_minor, reference_type, reference_id, reason, created_by)
                         VALUES (?, ?, 'purchase_receipt', ?, ?, ?, 'purchase', ?, ?, ?)",
                    )
                    .bind(product_id)
                    .bind(location_id)
                    .bind(quantity)
                    .bind(&move_number)
                    .bind(unit_cost)
                    .bind(input.purchase_id)
                    .bind(&reason)
                    .bind(actor_id)
                    .execute(&mut *tx)
                    .await?
                    .last_insert_rowid();

                    apply_on_hand_delta(&mut *tx, *product_id, location_id, *quantity).await?;
                    post_cost_layer(&mut *tx, *product_id, *quantity, *unit_cost, movement_id)
                        .await?;
                }

                record_ledger(
                    &mut *tx,
                    supplier_id,
                    "invoice",
                    "purchase",
                    input.purchase_id,
                    total,
                    &reason,
                    actor_id,
                )
                .await?;

                let mut payment_id: Option<i64> = None;
                if paid > 0 {
                    let cash_account_id = input.cash_account_id.unwrap();
                    let payment_method_id = input.payment_method_id.unwrap();
                    require_cash_balance(&mut *tx, cash_account_id, -paid, "purchase payment").await?;

                    let pay_number = next_document_number(&mut *tx, "supplier_payment").await?;
                    let pay_id = sqlx::query(
                        "INSERT INTO supplier_payments
                           (payment_number, supplier_id, payment_method_id, cash_account_id,
                            payment_date, amount_minor, status, notes, created_by)
                         VALUES (?, ?, ?, ?, ?, ?, 'posted', 'payment against purchase', ?)",
                    )
                    .bind(&pay_number)
                    .bind(supplier_id)
                    .bind(payment_method_id)
                    .bind(cash_account_id)
                    .bind(&purchase_date)
                    .bind(paid)
                    .bind(actor_id)
                    .execute(&mut *tx)
                    .await?
                    .last_insert_rowid();

                    sqlx::query(
                        "INSERT INTO supplier_payment_allocations (payment_id, purchase_id, amount_minor)
                         VALUES (?, ?, ?)",
                    )
                    .bind(pay_id)
                    .bind(input.purchase_id)
                    .bind(paid)
                    .execute(&mut *tx)
                    .await?;

                    record_cash_entry(
                        &mut *tx,
                        cash_account_id,
                        "purchase_payment",
                        -paid,
                        "supplier_payment",
                        pay_id,
                        &reason,
                        actor_id,
                    )
                    .await?;

                    record_ledger(
                        &mut *tx,
                        supplier_id,
                        "payment",
                        "supplier_payment",
                        pay_id,
                        -paid,
                        &pay_number,
                        actor_id,
                    )
                    .await?;
                    payment_id = Some(pay_id);
                }

                sqlx::query(
                    "UPDATE purchases
                        SET status = 'posted', purchase_number = ?, paid_minor = ?, due_minor = ?,
                            idempotency_key = ?, posted_by = ?, posted_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(&number)
                .bind(paid)
                .bind(total - paid)
                .bind(idempotency_key.as_deref().filter(|k| !k.trim().is_empty()))
                .bind(actor_id)
                .bind(input.purchase_id)
                .execute(&mut *tx)
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "purchase.post",
                    "purchase",
                    input.purchase_id,
                    &correlation,
                    Some(serde_json::json!({ "status": status })),
                    Some(serde_json::json!({
                        "purchase_number": number,
                        "total_minor": total,
                        "paid_minor": paid,
                        "due_minor": total - paid,
                        "payment_id": payment_id,
                    })),
                )
                .await?;
                Ok(input.purchase_id)
            })
        })
        .await?;

    purchase_dto(state, result).await
}

pub async fn list_purchases(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<PurchaseDto>, AppError> {
    principal.require("payable.view")?;
    let ids: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM purchases ORDER BY created_at DESC, id DESC")
            .fetch_all(&state.pool)
            .await?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(purchase_dto(state, id).await?);
    }
    Ok(out)
}

pub async fn get_purchase(
    state: &AppState,
    principal: &Principal,
    purchase_id: i64,
) -> Result<PurchaseDto, AppError> {
    principal.require("payable.view")?;
    purchase_dto(state, purchase_id).await
}

// ---------------------------------------------------------------------------
// Payable aging
// ---------------------------------------------------------------------------

pub async fn payable_aging(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<PayableAgingRowDto>, AppError> {
    principal.require("payable.view")?;

    let today = state.clock.now_iso();
    let today_date = today.split('T').next().unwrap_or(&today).to_string();

    let rows = sqlx::query(
        "SELECT p.id, p.purchase_number, p.supplier_id, p.supplier_name,
                p.invoice_date, p.due_minor
         FROM purchases p
         WHERE p.status = 'posted' AND p.due_minor > 0
         ORDER BY p.invoice_date ASC, p.id ASC",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let invoice_date: String = row.get(4);
        let age_days = age_in_days(&invoice_date, &today_date);
        let bucket = bucket_for(age_days);
        out.push(PayableAgingRowDto {
            purchase_id: row.get(0),
            purchase_number: row.get(1),
            supplier_id: row.get(2),
            supplier_name: row.get(3),
            invoice_date,
            due_minor: row.get(5),
            age_days,
            bucket,
        });
    }
    Ok(out)
}

fn age_in_days(invoice_date: &str, today: &str) -> i64 {
    let parse = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    match (parse(invoice_date), parse(today)) {
        (Some(a), Some(b)) => (b - a).num_days(),
        _ => 0,
    }
}

fn bucket_for(age_days: i64) -> String {
    if age_days <= 30 {
        "0-30".to_string()
    } else if age_days <= 60 {
        "31-60".to_string()
    } else if age_days <= 90 {
        "61-90".to_string()
    } else {
        "90+".to_string()
    }
}

// ---------------------------------------------------------------------------
// Supplier payments
// ---------------------------------------------------------------------------

async fn payment_dto(state: &AppState, payment_id: i64) -> Result<SupplierPaymentDto, AppError> {
    let row = sqlx::query(
        "SELECT p.id, p.payment_number, p.supplier_id, s.name, p.payment_method_id, pm.name,
                p.cash_account_id, ca.name, p.payment_date, p.amount_minor, p.status,
                p.notes, p.created_at, p.voided_at
         FROM supplier_payments p
         JOIN suppliers s        ON s.id = p.supplier_id
         JOIN payment_methods pm ON pm.id = p.payment_method_id
         JOIN cash_accounts ca   ON ca.id = p.cash_account_id
         WHERE p.id = ?",
    )
    .bind(payment_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("supplier payment {payment_id}")))?;

    let alloc_rows = sqlx::query(
        "SELECT a.purchase_id, pu.purchase_number, a.amount_minor
         FROM supplier_payment_allocations a
         LEFT JOIN purchases pu ON pu.id = a.purchase_id
         WHERE a.payment_id = ? ORDER BY a.id",
    )
    .bind(payment_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(SupplierPaymentDto {
        id: row.get(0),
        payment_number: row.get(1),
        supplier_id: row.get(2),
        supplier_name: row.get(3),
        payment_method_id: row.get(4),
        payment_method_name: row.get(5),
        cash_account_id: row.get(6),
        cash_account_name: row.get(7),
        payment_date: row.get(8),
        amount_minor: row.get(9),
        status: row.get(10),
        notes: row.get(11),
        allocations: alloc_rows
            .iter()
            .map(|r| PaymentAllocationDto {
                purchase_id: r.get(0),
                purchase_number: r.get(1),
                amount_minor: r.get(2),
            })
            .collect(),
        created_at: row.get(12),
        voided_at: row.get(13),
    })
}

pub async fn create_payment(
    state: &AppState,
    principal: &Principal,
    input: SupplierPaymentInput,
    correlation_id: &str,
) -> Result<SupplierPaymentDto, AppError> {
    principal.require("supplier.pay")?;

    if input.amount_minor <= 0 {
        return Err(AppError::Validation(
            "payment amount must be positive".into(),
        ));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let idempotency_key = input.idempotency_key.clone();
            let notes = input.notes.clone();
            let payment_date = input.payment_date.clone();
            Box::pin(async move {
                require_supplier(&mut *tx, input.supplier_id).await?;

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
                require_cash_balance(
                    &mut *tx,
                    input.cash_account_id,
                    -input.amount_minor,
                    "supplier payment",
                )
                .await?;

                let existing_id: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM supplier_payments WHERE idempotency_key = ?",
                )
                .bind(idempotency_key.as_deref().filter(|k| !k.trim().is_empty()))
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(id) = existing_id {
                    return Ok(id);
                }

                let pay_number = next_document_number(&mut *tx, "supplier_payment").await?;
                let pay_id = sqlx::query(
                    "INSERT INTO supplier_payments
                       (payment_number, supplier_id, payment_method_id, cash_account_id,
                        payment_date, amount_minor, status, notes, idempotency_key, created_by)
                     VALUES (?, ?, ?, ?, ?, ?, 'posted', ?, ?, ?)",
                )
                .bind(&pay_number)
                .bind(input.supplier_id)
                .bind(input.payment_method_id)
                .bind(input.cash_account_id)
                .bind(&payment_date)
                .bind(input.amount_minor)
                .bind(notes.as_deref())
                .bind(idempotency_key.as_deref().filter(|k| !k.trim().is_empty()))
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                // Oldest-first allocation across open invoices.
                let open: Vec<i64> = sqlx::query_scalar(
                    "SELECT id FROM purchases
                     WHERE supplier_id = ? AND status = 'posted' AND due_minor > 0
                     ORDER BY invoice_date ASC, id ASC",
                )
                .bind(input.supplier_id)
                .fetch_all(&mut *tx)
                .await?;

                let mut remaining = input.amount_minor;
                for purchase_id in open {
                    if remaining <= 0 {
                        break;
                    }
                    let due: i64 =
                        sqlx::query_scalar("SELECT due_minor FROM purchases WHERE id = ?")
                            .bind(purchase_id)
                            .fetch_one(&mut *tx)
                            .await?;
                    let alloc = std::cmp::min(due, remaining);
                    sqlx::query(
                        "UPDATE purchases
                            SET paid_minor = paid_minor + ?, due_minor = due_minor - ?,
                                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                          WHERE id = ?",
                    )
                    .bind(alloc)
                    .bind(alloc)
                    .bind(purchase_id)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "INSERT INTO supplier_payment_allocations (payment_id, purchase_id, amount_minor)
                         VALUES (?, ?, ?)",
                    )
                    .bind(pay_id)
                    .bind(purchase_id)
                    .bind(alloc)
                    .execute(&mut *tx)
                    .await?;
                    remaining -= alloc;
                }
                if remaining > 0 {
                    return Err(AppError::Validation(format!(
                        "payment exceeds open payables for supplier {} by {}",
                        input.supplier_id, remaining
                    )));
                }

                record_cash_entry(
                    &mut *tx,
                    input.cash_account_id,
                    "purchase_payment",
                    -input.amount_minor,
                    "supplier_payment",
                    pay_id,
                    &format!("supplier payment {pay_number}"),
                    actor_id,
                )
                .await?;

                record_ledger(
                    &mut *tx,
                    input.supplier_id,
                    "payment",
                    "supplier_payment",
                    pay_id,
                    -input.amount_minor,
                    &pay_number,
                    actor_id,
                )
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "supplier.payment",
                    "supplier_payment",
                    pay_id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "payment_number": pay_number,
                        "supplier_id": input.supplier_id,
                        "amount_minor": input.amount_minor,
                    })),
                )
                .await?;
                Ok(pay_id)
            })
        })
        .await?;

    payment_dto(state, result).await
}

pub async fn void_payment(
    state: &AppState,
    principal: &Principal,
    input: SupplierPaymentVoidInput,
    correlation_id: &str,
) -> Result<SupplierPaymentDto, AppError> {
    principal.require("payment.void")?;

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
                    "SELECT supplier_id, cash_account_id, amount_minor, status, payment_number
                     FROM supplier_payments WHERE id = ?",
                )
                .bind(input.payment_id)
                .fetch_optional(&mut *tx)
                .await?;
                let Some((supplier_id, cash_account_id, amount_minor, status, payment_number)) =
                    payment
                else {
                    return Err(AppError::NotFound(format!(
                        "supplier payment {}",
                        input.payment_id
                    )));
                };
                if status != "posted" {
                    return Err(AppError::Conflict(format!(
                        "payment {} is already voided",
                        input.payment_id
                    )));
                }

                let allocations: Vec<(i64, i64)> = sqlx::query_as(
                    "SELECT purchase_id, amount_minor FROM supplier_payment_allocations
                     WHERE payment_id = ?",
                )
                .bind(input.payment_id)
                .fetch_all(&mut *tx)
                .await?;
                for (purchase_id, alloc) in allocations {
                    sqlx::query(
                        "UPDATE purchases
                            SET paid_minor = MAX(paid_minor - ?, 0), due_minor = due_minor + ?,
                                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                          WHERE id = ?",
                    )
                    .bind(alloc)
                    .bind(alloc)
                    .bind(purchase_id)
                    .execute(&mut *tx)
                    .await?;
                }

                record_cash_entry(
                    &mut *tx,
                    cash_account_id,
                    "payment_void",
                    amount_minor,
                    "supplier_payment",
                    input.payment_id,
                    &format!("void {payment_number}"),
                    actor_id,
                )
                .await?;

                record_ledger(
                    &mut *tx,
                    supplier_id,
                    "void",
                    "supplier_payment",
                    input.payment_id,
                    amount_minor,
                    &format!("void {payment_number}"),
                    actor_id,
                )
                .await?;

                sqlx::query(
                    "UPDATE supplier_payments
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
                    "supplier.payment_void",
                    "supplier_payment",
                    input.payment_id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "payment_number": payment_number,
                        "amount_minor": amount_minor,
                        "reason": reason,
                    })),
                )
                .await?;
                Ok(input.payment_id)
            })
        })
        .await?;

    payment_dto(state, input.payment_id).await
}

pub async fn list_payments(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<SupplierPaymentDto>, AppError> {
    principal.require("payable.view")?;
    let ids: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM supplier_payments ORDER BY created_at DESC, id DESC")
            .fetch_all(&state.pool)
            .await?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(payment_dto(state, id).await?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Supplier returns
// ---------------------------------------------------------------------------

fn map_return_item(r: &sqlx::sqlite::SqliteRow) -> SupplierReturnItemDto {
    SupplierReturnItemDto {
        product_id: r.get(1),
        article_number: r.get(2),
        product_name: r.get(3),
        quantity: r.get(4),
        unit_cost_minor: r.get(5),
        line_total_minor: r.get(6),
    }
}

async fn return_dto(state: &AppState, return_id: i64) -> Result<SupplierReturnDto, AppError> {
    let row = sqlx::query(
        "SELECT r.id, r.return_number, r.supplier_id, r.supplier_name, r.purchase_id,
                r.location_id, r.return_date, r.status, r.total_minor, r.refund_minor,
                r.due_reduction_minor, r.notes, r.created_at, r.posted_at
         FROM supplier_returns r WHERE r.id = ?",
    )
    .bind(return_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("supplier return {return_id}")))?;

    let item_rows = sqlx::query(
        "SELECT id, product_id, article_number, product_name, quantity, unit_cost_minor, line_total_minor
         FROM supplier_return_items WHERE return_id = ? ORDER BY id",
    )
    .bind(return_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(SupplierReturnDto {
        id: row.get(0),
        return_number: row.get(1),
        supplier_id: row.get(2),
        supplier_name: row.get(3),
        purchase_id: row.get(4),
        location_id: row.get(5),
        return_date: row.get(6),
        status: row.get(7),
        total_minor: row.get(8),
        refund_minor: row.get(9),
        due_reduction_minor: row.get(10),
        notes: row.get(11),
        items: item_rows.iter().map(map_return_item).collect(),
        created_at: row.get(12),
        posted_at: row.get(13),
    })
}

pub async fn create_return(
    state: &AppState,
    principal: &Principal,
    input: SupplierReturnCreateInput,
    correlation_id: &str,
) -> Result<SupplierReturnDto, AppError> {
    principal.require("supplier.return")?;

    if input.items.is_empty() {
        return Err(AppError::Validation(
            "return must have at least one item".into(),
        ));
    }
    for item in &input.items {
        if item.quantity <= 0 {
            return Err(AppError::Validation(
                "return quantity must be positive".into(),
            ));
        }
    }
    let refund = input.refund_minor.unwrap_or(0);
    if refund < 0 {
        return Err(AppError::Validation("refund cannot be negative".into()));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let return_date = input.return_date.clone();
            let notes = input.notes.clone();
            Box::pin(async move {
                require_supplier(&mut *tx, input.supplier_id).await?;
                require_active_location(&mut *tx, input.location_id).await?;

                if let Some(pid) = input.purchase_id {
                    let purchase: Option<(String, i64)> = sqlx::query_as(
                        "SELECT status, supplier_id FROM purchases WHERE id = ?",
                    )
                    .bind(pid)
                    .fetch_optional(&mut *tx)
                    .await?;
                    let Some((status, purchase_supplier)) = purchase else {
                        return Err(AppError::NotFound(format!("purchase {pid}")));
                    };
                    if status != "posted" {
                        return Err(AppError::Validation(format!(
                            "return can only reference a posted purchase (purchase {pid} is {status})"
                        )));
                    }
                    if purchase_supplier != input.supplier_id {
                        return Err(AppError::Validation(format!(
                            "purchase {pid} belongs to a different supplier"
                        )));
                    }
                }

                let supplier_name: String =
                    sqlx::query_scalar("SELECT name FROM suppliers WHERE id = ?")
                        .bind(input.supplier_id)
                        .fetch_one(&mut *tx)
                        .await?;

                let return_id = sqlx::query(
                    "INSERT INTO supplier_returns
                       (supplier_id, supplier_name, purchase_id, location_id, return_date,
                        status, refund_minor, notes, created_by)
                     VALUES (?, ?, ?, ?, ?, 'draft', ?, ?, ?)",
                )
                .bind(input.supplier_id)
                .bind(&supplier_name)
                .bind(input.purchase_id)
                .bind(input.location_id)
                .bind(&return_date)
                .bind(refund)
                .bind(notes.as_deref())
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                let mut total: i64 = 0;
                for item in &input.items {
                    require_positive(&mut *tx, item.product_id).await?;
                    let product: Option<(String, String)> = sqlx::query_as(
                        "SELECT article_number, name FROM products WHERE id = ?",
                    )
                    .bind(item.product_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                    let Some((article_number, product_name)) = product else {
                        return Err(AppError::NotFound(format!("product {}", item.product_id)));
                    };
                    let line_total = item.quantity.saturating_mul(item.unit_cost_minor);
                    total = total.saturating_add(line_total);
                    sqlx::query(
                        "INSERT INTO supplier_return_items
                           (return_id, product_id, article_number, product_name,
                            quantity, unit_cost_minor, line_total_minor)
                         VALUES (?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(return_id)
                    .bind(item.product_id)
                    .bind(&article_number)
                    .bind(&product_name)
                    .bind(item.quantity)
                    .bind(item.unit_cost_minor)
                    .bind(line_total)
                    .execute(&mut *tx)
                    .await?;
                }
                sqlx::query("UPDATE supplier_returns SET total_minor = ? WHERE id = ?")
                    .bind(total)
                    .bind(return_id)
                    .execute(&mut *tx)
                    .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "supplier.return_draft",
                    "supplier_return",
                    return_id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "supplier_id": input.supplier_id,
                        "total_minor": total,
                        "refund_minor": refund,
                        "item_count": input.items.len(),
                    })),
                )
                .await?;
                Ok(return_id)
            })
        })
        .await?;

    return_dto(state, result).await
}

pub async fn post_return(
    state: &AppState,
    principal: &Principal,
    input: SupplierReturnPostInput,
    correlation_id: &str,
) -> Result<SupplierReturnDto, AppError> {
    principal.require("supplier.return")?;

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let idempotency_key = input.idempotency_key.clone();
            Box::pin(async move {
                let ret: Option<(String, i64, i64, i64)> = sqlx::query_as(
                    "SELECT status, supplier_id, location_id, refund_minor FROM supplier_returns WHERE id = ?",
                )
                .bind(input.return_id)
                .fetch_optional(&mut *tx)
                .await?;
                let Some((status, supplier_id, location_id, refund)) = ret else {
                    return Err(AppError::NotFound(format!(
                        "supplier return {}",
                        input.return_id
                    )));
                };
                if replay_guard(&mut *tx, "supplier_returns", input.return_id, &idempotency_key)
                    .await?
                {
                    return Ok(input.return_id);
                }
                if status != "draft" {
                    return Err(AppError::Conflict(format!(
                        "supplier return {} is not a draft",
                        input.return_id
                    )));
                }

                require_supplier(&mut *tx, supplier_id).await?;
                require_active_location(&mut *tx, location_id).await?;

                let items: Vec<(i64, i64, i64)> = sqlx::query_as(
                    "SELECT id, product_id, quantity FROM supplier_return_items
                     WHERE return_id = ? ORDER BY id",
                )
                .bind(input.return_id)
                .fetch_all(&mut *tx)
                .await?;
                if items.is_empty() {
                    return Err(AppError::Validation(
                        "return has no items to post".into(),
                    ));
                }

                let number = next_document_number(&mut *tx, "supplier_return").await?;
                let reason = format!("supplier return {number}");
                let mut total: i64 = 0;

                for (item_id, product_id, quantity) in &items {
                    let net_received: i64 = sqlx::query_scalar(
                        "SELECT COALESCE(SUM(quantity_delta), 0) FROM stock_movements
                         WHERE product_id = ? AND location_id = ?
                           AND movement_type IN ('purchase_receipt', 'supplier_return')",
                    )
                    .bind(product_id)
                    .bind(location_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    if *quantity > net_received {
                        return Err(AppError::Validation(format!(
                            "cannot return {quantity} of product {product_id}: only {net_received} net received"
                        )));
                    }

                    let Some(avg_cost) =
                        withdraw_cost_layers(&mut *tx, *product_id, *quantity).await?
                    else {
                        return Err(AppError::Validation(format!(
                            "no cost basis for product {product_id}"
                        )));
                    };
                    let line_total = (*quantity).saturating_mul(avg_cost);
                    total = total.saturating_add(line_total);

                    sqlx::query(
                        "UPDATE supplier_return_items SET unit_cost_minor = ?, line_total_minor = ? WHERE id = ?",
                    )
                    .bind(avg_cost)
                    .bind(line_total)
                    .bind(item_id)
                    .execute(&mut *tx)
                    .await?;

                    let seq = next_move_seq(&mut *tx, location_id).await?;
                    let move_number = format!("SRN-{seq:06}");
                    sqlx::query(
                        "INSERT INTO stock_movements
                           (product_id, location_id, movement_type, quantity_delta,
                            move_number, unit_cost_minor, reference_type, reference_id, reason, created_by)
                         VALUES (?, ?, 'supplier_return', ?, ?, ?, 'supplier_return', ?, ?, ?)",
                    )
                    .bind(product_id)
                    .bind(location_id)
                    .bind(-*quantity)
                    .bind(&move_number)
                    .bind(avg_cost)
                    .bind(input.return_id)
                    .bind(&reason)
                    .bind(actor_id)
                    .execute(&mut *tx)
                    .await?;

                    apply_on_hand_delta(&mut *tx, *product_id, location_id, -*quantity).await?;
                }

                if refund > total {
                    return Err(AppError::Validation(format!(
                        "refund {refund} exceeds return total {total}"
                    )));
                }
                let due_reduction = total - refund;

                if due_reduction > 0 {
                    let after = record_ledger(
                        &mut *tx,
                        supplier_id,
                        "return",
                        "supplier_return",
                        input.return_id,
                        -due_reduction,
                        &reason,
                        actor_id,
                    )
                    .await?;
                    if after < 0 {
                        return Err(AppError::Validation(format!(
                            "return exceeds the supplier payable balance (would reduce to {after})"
                        )));
                    }
                }
                if refund > 0 {
                    let main_cash: Option<i64> = sqlx::query_scalar(
                        "SELECT id FROM cash_accounts WHERE code = 'main_cash'",
                    )
                    .fetch_optional(&mut *tx)
                    .await?;
                    let Some(account_id) = main_cash else {
                        return Err(AppError::Internal(
                            "main cash account is not configured".into(),
                        ));
                    };
                    record_cash_entry(
                        &mut *tx,
                        account_id,
                        "supplier_refund",
                        refund,
                        "supplier_return",
                        input.return_id,
                        &reason,
                        actor_id,
                    )
                    .await?;
                }

                sqlx::query(
                    "UPDATE supplier_returns
                        SET status = 'posted', return_number = ?, total_minor = ?, refund_minor = ?,
                            due_reduction_minor = ?, idempotency_key = ?, posted_by = ?,
                            posted_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(&number)
                .bind(total)
                .bind(refund)
                .bind(due_reduction)
                .bind(idempotency_key.as_deref().filter(|k| !k.trim().is_empty()))
                .bind(actor_id)
                .bind(input.return_id)
                .execute(&mut *tx)
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "supplier.return_post",
                    "supplier_return",
                    input.return_id,
                    &correlation,
                    Some(serde_json::json!({ "status": status })),
                    Some(serde_json::json!({
                        "return_number": number,
                        "total_minor": total,
                        "refund_minor": refund,
                        "due_reduction_minor": due_reduction,
                    })),
                )
                .await?;
                Ok(input.return_id)
            })
        })
        .await?;

    return_dto(state, result).await
}

pub async fn list_returns(
    state: &AppState,
    principal: &Principal,
) -> Result<Vec<SupplierReturnDto>, AppError> {
    principal.require("payable.view")?;
    let ids: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM supplier_returns ORDER BY created_at DESC, id DESC")
            .fetch_all(&state.pool)
            .await?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(return_dto(state, id).await?);
    }
    Ok(out)
}
