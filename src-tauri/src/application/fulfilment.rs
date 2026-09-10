use sqlx::sqlite::SqliteConnection;
use sqlx::Row;

use crate::application::auth::Principal;
use crate::application::cash::{record_cash_entry, require_cash_balance};
use crate::application::customers::{customer_balance, record_ledger};
use crate::application::documents::next_document_number;
use crate::application::inventory::{
    apply_damage_shift, apply_damaged_delta, apply_on_hand_delta, apply_repair_shift,
    next_move_seq, post_cost_layer, require_active_location, require_positive,
    withdraw_cost_layers,
};
use crate::application::suppliers::state_audit;
use crate::dto::fulfilment::{
    CreditNoteDto, CreditNoteListInput, DamageDecisionInput, DamageListInput, DamageRecordDto,
    DamageRecordInput, DeliveryCreateInput, DeliveryDto, DeliveryItemDto, DeliveryListInput,
    DeliveryRescheduleInput, DeliveryTransitionInput, ReturnListInput, ReturnVoidInput,
    SaleReturnDto, SaleReturnInput, SaleReturnItemDto,
};
use crate::dto::PdfResultDto;
use crate::error::AppError;
use crate::infrastructure::{
    generate_credit_note_pdf, generate_delivery_note_pdf, CreditNoteRecord, DeliveryNoteLine,
    DeliveryNoteRecord,
};
use crate::state::AppState;

const ALLOWED_SOURCES: [&str; 4] = ["in_hand", "customer_return", "count", "other"];
const ALLOWED_DECISIONS: [&str; 4] = ["repair", "supplier_return", "damaged_sale", "write_off"];
const ALLOWED_CLASSIFICATIONS: [&str; 4] = ["sellable", "damaged", "repair", "disposed"];

// ---------------------------------------------------------------------------
// DTO mappers
// ---------------------------------------------------------------------------

async fn shop_identity(conn: &mut SqliteConnection) -> Result<(String, Option<String>), AppError> {
    let shop_name =
        sqlx::query_scalar::<_, String>("SELECT value_json FROM settings WHERE key = 'shop.name'")
            .fetch_optional(&mut *conn)
            .await?
            .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok())
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "Furniture Shop".into());

    let shop_address = sqlx::query_scalar::<_, String>(
        "SELECT value_json FROM settings WHERE key = 'shop.address'",
    )
    .fetch_optional(&mut *conn)
    .await
    .ok()
    .flatten()
    .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok())
    .and_then(|v| v.as_str().map(str::to_string));

    Ok((shop_name, shop_address))
}

async fn delivery_dto(state: &AppState, delivery_id: i64) -> Result<DeliveryDto, AppError> {
    let row = sqlx::query(
        "SELECT d.id, d.delivery_number, d.sale_id, s.sale_number, d.customer_id,
                d.customer_name, d.location_id, d.status, d.scheduled_at, d.address,
                d.contact_name, d.contact_phone, d.driver_note, d.vehicle_note,
                d.receiver_name, d.proof_reference, d.delivery_charge_minor, d.notes,
                d.reschedule_count, d.delivered_at, d.delivered_by, d.dispatched_at,
                d.dispatched_by, d.failed_reason, d.failed_at, d.failed_by,
                d.cancelled_reason, d.cancelled_at, d.cancelled_by,
                d.created_by, d.created_at, d.updated_at
         FROM deliveries d LEFT JOIN sales s ON s.id = d.sale_id
         WHERE d.id = ?",
    )
    .bind(delivery_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("delivery {delivery_id}")))?;

    let items = sqlx::query(
        "SELECT id, delivery_id, sale_item_id, product_id, article_number, product_name,
                quantity, unit_price_minor, line_total_minor
         FROM delivery_items WHERE delivery_id = ? ORDER BY id",
    )
    .bind(delivery_id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| DeliveryItemDto {
        id: r.get(0),
        delivery_id: r.get(1),
        sale_item_id: r.get(2),
        product_id: r.try_get(3).ok(),
        article_number: r.get(4),
        product_name: r.get(5),
        quantity: r.get(6),
        unit_price_minor: r.get(7),
        line_total_minor: r.get(8),
    })
    .collect();

    Ok(DeliveryDto {
        id: row.get(0),
        delivery_number: row
            .try_get::<Option<String>, _>(1)
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()),
        sale_id: row.get(2),
        sale_number: row
            .try_get::<Option<String>, _>(3)
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()),
        customer_id: row.try_get(4).ok(),
        customer_name: row.try_get(5).ok(),
        location_id: row.get(6),
        status: row.get(7),
        scheduled_at: row.try_get(8).ok(),
        address: row.try_get(9).ok(),
        contact_name: row.try_get(10).ok(),
        contact_phone: row.try_get(11).ok(),
        driver_note: row.try_get(12).ok(),
        vehicle_note: row.try_get(13).ok(),
        receiver_name: row.try_get(14).ok(),
        proof_reference: row.try_get(15).ok(),
        delivery_charge_minor: row.get(16),
        notes: row.try_get(17).ok(),
        reschedule_count: row.get(18),
        delivered_at: row.try_get(19).ok(),
        delivered_by: row.try_get(20).ok(),
        dispatched_at: row.try_get(21).ok(),
        dispatched_by: row.try_get(22).ok(),
        failed_reason: row.try_get(23).ok(),
        failed_at: row.try_get(24).ok(),
        failed_by: row.try_get(25).ok(),
        cancelled_reason: row.try_get(26).ok(),
        cancelled_at: row.try_get(27).ok(),
        cancelled_by: row.try_get(28).ok(),
        items,
        created_by: row.get(29),
        created_at: row.get(30),
        updated_at: row.get(31),
    })
}

async fn sale_return_dto(state: &AppState, return_id: i64) -> Result<SaleReturnDto, AppError> {
    let row = sqlx::query(
        "SELECT r.id, r.return_number, r.sale_id, s.sale_number, r.customer_id,
                c.name, r.location_id, r.return_date, r.status, r.refund_type,
                r.total_minor, r.total_refund_minor, r.cash_refund_minor,
                r.credit_note_minor, r.notes, r.posted_by, r.posted_at,
                r.voided_by, r.voided_at, r.created_at
         FROM sales_returns r
         LEFT JOIN sales s ON s.id = r.sale_id
         LEFT JOIN customers c ON c.id = r.customer_id
         WHERE r.id = ?",
    )
    .bind(return_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("sales return {return_id}")))?;

    let items = sqlx::query(
        "SELECT id, return_id, sale_item_id, product_id, bundle_id, article_number,
                product_name, quantity, unit_price_minor, unit_refund_minor,
                line_refund_minor, classification
         FROM sales_return_items WHERE return_id = ? ORDER BY id",
    )
    .bind(return_id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| SaleReturnItemDto {
        id: r.get(0),
        return_id: r.get(1),
        sale_item_id: r.get(2),
        product_id: r.try_get(3).ok(),
        bundle_id: r.try_get(4).ok(),
        article_number: r.get(5),
        product_name: r.get(6),
        quantity: r.get(7),
        unit_price_minor: r.get(8),
        unit_refund_minor: r.get(9),
        line_refund_minor: r.get(10),
        classification: r.get(11),
    })
    .collect();

    Ok(SaleReturnDto {
        id: row.get(0),
        return_number: row
            .try_get::<Option<String>, _>(1)
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()),
        sale_id: row.get(2),
        sale_number: row
            .try_get::<Option<String>, _>(3)
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()),
        customer_id: row.try_get(4).ok(),
        customer_name: row
            .try_get::<Option<String>, _>(5)
            .ok()
            .flatten()
            .filter(|s: &String| !s.is_empty()),
        location_id: row.get(6),
        return_date: row.get(7),
        status: row.get(8),
        refund_type: row.get(9),
        total_minor: row.get(10),
        total_refund_minor: row.get(11),
        cash_refund_minor: row.get(12),
        credit_note_minor: row.get(13),
        notes: row.try_get(14).ok(),
        posted_by: row.try_get(15).ok(),
        posted_at: row.try_get(16).ok(),
        voided_by: row.try_get(17).ok(),
        voided_at: row.try_get(18).ok(),
        items,
        created_at: row.get(19),
    })
}

async fn credit_note_dto(state: &AppState, credit_id: i64) -> Result<CreditNoteDto, AppError> {
    let row = sqlx::query(
        "SELECT id, credit_number, customer_id, return_id, sale_id, amount_minor,
                status, notes, applied_at, created_by, created_at
         FROM credit_notes WHERE id = ?",
    )
    .bind(credit_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("credit note {credit_id}")))?;

    Ok(CreditNoteDto {
        id: row.get(0),
        credit_number: row
            .try_get::<Option<String>, _>(1)
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()),
        customer_id: row.get(2),
        return_id: row.try_get(3).ok(),
        sale_id: row.try_get(4).ok(),
        amount_minor: row.get(5),
        status: row.get(6),
        notes: row.try_get(7).ok(),
        applied_at: row.try_get(8).ok(),
        created_by: row.get(9),
        created_at: row.get(10),
    })
}

async fn damage_dto(state: &AppState, damage_id: i64) -> Result<DamageRecordDto, AppError> {
    let row = sqlx::query(
        "SELECT d.id, d.damage_number, d.product_id, p.article_number, p.name,
                d.location_id, l.name, d.quantity, d.damage_date, d.source, d.reason,
                d.estimated_loss_minor, d.photo_path, d.status, d.decision,
                d.decision_note, d.linked_sale_id, d.resolved_by, d.resolved_at,
                d.created_by, d.created_at, d.updated_at
         FROM damage_records d
         JOIN products p ON p.id = d.product_id
         JOIN locations l ON l.id = d.location_id
         WHERE d.id = ?",
    )
    .bind(damage_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("damage record {damage_id}")))?;

    Ok(DamageRecordDto {
        id: row.get(0),
        damage_number: row
            .try_get::<Option<String>, _>(1)
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()),
        product_id: row.get(2),
        article_number: row.get(3),
        product_name: row.get(4),
        location_id: row.get(5),
        location_name: row.get(6),
        quantity: row.get(7),
        damage_date: row.get(8),
        source: row.get(9),
        reason: row.try_get(10).ok(),
        estimated_loss_minor: row.get(11),
        photo_path: row.try_get(12).ok(),
        status: row.get(13),
        decision: row.try_get(14).ok(),
        decision_note: row.try_get(15).ok(),
        linked_sale_id: row.try_get(16).ok(),
        resolved_by: row.try_get(17).ok(),
        resolved_at: row.try_get(18).ok(),
        created_by: row.get(19),
        created_at: row.get(20),
        updated_at: row.get(21),
    })
}

// ---------------------------------------------------------------------------
// Deliveries
// ---------------------------------------------------------------------------

pub async fn create_delivery(
    state: &AppState,
    principal: &Principal,
    input: DeliveryCreateInput,
    correlation_id: &str,
) -> Result<DeliveryDto, AppError> {
    principal.require("delivery.create")?;
    if input.items.is_empty() {
        return Err(AppError::Validation("delivery has no items".into()));
    }
    for item in &input.items {
        if item.quantity <= 0 {
            return Err(AppError::Validation(
                "delivery item quantities must be positive".into(),
            ));
        }
    }
    let charge = input.delivery_charge_minor.unwrap_or(0);
    if charge < 0 {
        return Err(AppError::Validation(
            "delivery charge cannot be negative".into(),
        ));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let input = input.clone();
            Box::pin(async move {
                let sale = sqlx::query(
                    "SELECT id, customer_id, customer_name, location_id, status, sale_date
                     FROM sales WHERE id = ?",
                )
                .bind(input.sale_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("sale {}", input.sale_id)))?;

                let status: String = sale.get(4);
                if status != "confirmed" {
                    return Err(AppError::Conflict(format!(
                        "deliveries require a confirmed sale (sale {} is '{status}')",
                        input.sale_id
                    )));
                }
                let location_id = sale.get::<i64, _>(3);
                require_active_location(&mut *tx, location_id).await?;

                let sale_items: Vec<(i64, i64)> = sqlx::query(
                    "SELECT id, quantity FROM sale_items WHERE sale_id = ? ORDER BY id",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1)))
                .collect();

                for item in &input.items {
                    let Some((_, quantity)) =
                        sale_items.iter().find(|(id, _)| *id == item.sale_item_id)
                    else {
                        return Err(AppError::Validation(format!(
                            "sale item {} is not on sale {}",
                            item.sale_item_id, input.sale_id
                        )));
                    };
                    let delivered: i64 = sqlx::query_scalar(
                        "SELECT COALESCE(SUM(di.quantity), 0)
                         FROM delivery_items di
                         JOIN deliveries d ON d.id = di.delivery_id
                         WHERE di.sale_item_id = ? AND d.status != 'cancelled'",
                    )
                    .bind(item.sale_item_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    let remaining = quantity - delivered;
                    if item.quantity > remaining {
                        return Err(AppError::Validation(format!(
                            "sale item {}: delivery of {} exceeds remaining {remaining}",
                            item.sale_item_id, item.quantity
                        )));
                    }
                }

                let delivery_number = next_document_number(&mut *tx, "delivery").await?;
                let delivery_id = sqlx::query(
                    "INSERT INTO deliveries
                       (delivery_number, sale_id, customer_id, customer_name, location_id,
                        status, scheduled_at, address, contact_name, contact_phone,
                        driver_note, vehicle_note, delivery_charge_minor, notes, created_by)
                     VALUES (?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&delivery_number)
                .bind(input.sale_id)
                .bind(sale.get::<Option<i64>, _>(1))
                .bind(sale.get::<Option<String>, _>(2))
                .bind(location_id)
                .bind(input.scheduled_at.as_deref())
                .bind(input.address.as_deref())
                .bind(input.contact_name.as_deref())
                .bind(input.contact_phone.as_deref())
                .bind(input.driver_note.as_deref())
                .bind(input.vehicle_note.as_deref())
                .bind(charge)
                .bind(input.notes.as_deref())
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                let item_meta: Vec<(i64, Option<i64>, String, String, i64)> = sqlx::query(
                    "SELECT id, product_id, article_number, product_name, unit_price_minor
                     FROM sale_items WHERE sale_id = ? ORDER BY id",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3), r.get(4)))
                .collect();

                for item in &input.items {
                    let Some((_, pid, article, name, unit_price)) = item_meta
                        .iter()
                        .find(|(id, _, _, _, _)| *id == item.sale_item_id)
                    else {
                        continue;
                    };
                    sqlx::query(
                        "INSERT INTO delivery_items
                           (delivery_id, sale_item_id, product_id, article_number,
                            product_name, quantity, unit_price_minor, line_total_minor)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(delivery_id)
                    .bind(item.sale_item_id)
                    .bind(pid)
                    .bind(article)
                    .bind(name)
                    .bind(item.quantity)
                    .bind(unit_price)
                    .bind(unit_price * item.quantity)
                    .execute(&mut *tx)
                    .await?;
                }

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "delivery.create",
                    "delivery",
                    delivery_id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "delivery_number": delivery_number,
                        "sale_id": input.sale_id,
                        "items": input.items.len(),
                    })),
                )
                .await?;
                Ok(delivery_id)
            })
        })
        .await?;

    delivery_dto(state, result).await
}

pub async fn transition_delivery(
    state: &AppState,
    principal: &Principal,
    input: DeliveryTransitionInput,
    correlation_id: &str,
) -> Result<DeliveryDto, AppError> {
    principal.require("delivery.update")?;

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let row = sqlx::query(
                    "SELECT id, status, sale_id, scheduled_at FROM deliveries WHERE id = ?",
                )
                .bind(input.delivery_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("delivery {}", input.delivery_id)))?;

                let current: String = row.get(1);
                let now = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";
                let outcome: Result<String, AppError> = match input.action.as_str() {
                    "ready" if current == "pending" => {
                        sqlx::query(&format!(
                            "UPDATE deliveries SET status = 'ready', updated_at = {now} WHERE id = ?"
                        ))
                        .bind(input.delivery_id)
                        .execute(&mut *tx)
                        .await?;
                        Ok("ready".into())
                    }
                    "dispatched" if current == "ready" => {
                        sqlx::query(&format!(
                            "UPDATE deliveries SET status = 'dispatched', dispatched_at = {now},
                             dispatched_by = ?, updated_at = {now} WHERE id = ?"
                        ))
                        .bind(actor_id)
                        .bind(input.delivery_id)
                        .execute(&mut *tx)
                        .await?;
                        Ok("dispatched".into())
                    }
                    "delivered" if current == "dispatched" => {
                        sqlx::query(&format!(
                            "UPDATE deliveries SET status = 'delivered', receiver_name = ?,
                             proof_reference = ?, delivered_at = {now}, delivered_by = ?,
                             updated_at = {now} WHERE id = ?"
                        ))
                        .bind(input.receiver_name.as_deref())
                        .bind(input.proof_reference.as_deref())
                        .bind(actor_id)
                        .bind(input.delivery_id)
                        .execute(&mut *tx)
                        .await?;
                        Ok("delivered".into())
                    }
                    "failed"
                        if matches!(current.as_str(), "pending" | "ready" | "dispatched") =>
                    {
                        sqlx::query(&format!(
                            "UPDATE deliveries SET status = 'failed', failed_reason = ?,
                             failed_at = {now}, failed_by = ?, updated_at = {now} WHERE id = ?"
                        ))
                        .bind(input.reason.as_deref())
                        .bind(actor_id)
                        .bind(input.delivery_id)
                        .execute(&mut *tx)
                        .await?;
                        Ok("failed".into())
                    }
                    "cancelled"
                        if matches!(current.as_str(), "pending" | "ready" | "failed") =>
                    {
                        sqlx::query(&format!(
                            "UPDATE deliveries SET status = 'cancelled', cancelled_reason = ?,
                             cancelled_at = {now}, cancelled_by = ?, updated_at = {now} WHERE id = ?"
                        ))
                        .bind(input.reason.as_deref())
                        .bind(actor_id)
                        .bind(input.delivery_id)
                        .execute(&mut *tx)
                        .await?;
                        Ok("cancelled".into())
                    }
                    "reschedule"
                        if matches!(current.as_str(), "pending" | "ready" | "failed") =>
                    {
                        let Some(new_date) = input.scheduled_at.as_deref() else {
                            return Err(AppError::Validation(
                                "reschedule requires a scheduled_at date".into(),
                            ));
                        };
                        if new_date.trim().is_empty() {
                            return Err(AppError::Validation(
                                "reschedule requires a scheduled_at date".into(),
                            ));
                        }
                        sqlx::query("UPDATE deliveries
                                    SET status = 'pending', scheduled_at = ?,
                                        reschedule_count = reschedule_count + 1,
                                        notes = CASE WHEN ? IS NOT NULL THEN
                                            COALESCE(notes, '') || '; rescheduled: ' || ?
                                        ELSE notes END,
                                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                                    WHERE id = ?")
                            .bind(new_date)
                            .bind(input.reason.as_deref())
                            .bind(input.reason.as_deref())
                            .bind(input.delivery_id)
                            .execute(&mut *tx)
                            .await?;
                        Ok("pending".into())
                    }
                    _ => Err(AppError::Conflict(format!(
                        "delivery {} cannot transition from '{current}' via '{}'",
                        input.delivery_id, input.action
                    ))),
                };
                let outcome = outcome?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "delivery.transition",
                    "delivery",
                    input.delivery_id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "from": current,
                        "to": outcome,
                        "reason": input.reason,
                    })),
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    delivery_dto(state, input.delivery_id).await
}

pub async fn reschedule_delivery(
    state: &AppState,
    principal: &Principal,
    input: DeliveryRescheduleInput,
    correlation_id: &str,
) -> Result<DeliveryDto, AppError> {
    transition_delivery(
        state,
        principal,
        DeliveryTransitionInput {
            delivery_id: input.delivery_id,
            action: "reschedule".into(),
            reason: input.reason,
            scheduled_at: Some(input.scheduled_at),
            receiver_name: None,
            proof_reference: None,
        },
        correlation_id,
    )
    .await
}

pub async fn list_deliveries(
    state: &AppState,
    principal: &Principal,
    input: DeliveryListInput,
) -> Result<Vec<DeliveryDto>, AppError> {
    principal.require("delivery.view")?;
    let limit = input.limit.unwrap_or(100).min(500);
    let offset = input.offset.unwrap_or(0).max(0);

    let mut sql =
        String::from("SELECT id FROM deliveries WHERE 1 = 1 ORDER BY id DESC LIMIT ? OFFSET ?");
    let mut predicates = String::new();
    if input.status.is_some() {
        predicates.push_str(" AND status = ?");
    }
    if input.sale_id.is_some() {
        predicates.push_str(" AND sale_id = ?");
    }
    if input.customer_id.is_some() {
        predicates.push_str(" AND customer_id = ?");
    }
    sql = sql.replace("WHERE 1 = 1", &format!("WHERE 1 = 1{predicates}"));

    let mut query = sqlx::query(&sql);
    if let Some(status) = &input.status {
        query = query.bind(status);
    }
    if let Some(sale_id) = input.sale_id {
        query = query.bind(sale_id);
    }
    if let Some(customer_id) = input.customer_id {
        query = query.bind(customer_id);
    }
    query = query.bind(limit).bind(offset);

    let ids: Vec<i64> = query
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|r| r.get::<i64, _>(0))
        .collect();

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(delivery_dto(state, id).await?);
    }
    Ok(out)
}

pub async fn get_delivery(
    state: &AppState,
    principal: &Principal,
    delivery_id: i64,
) -> Result<DeliveryDto, AppError> {
    principal.require("delivery.view")?;
    delivery_dto(state, delivery_id).await
}

pub async fn delivery_note_pdf(
    state: &AppState,
    principal: &Principal,
    delivery_id: i64,
) -> Result<PdfResultDto, AppError> {
    principal.require("delivery.view")?;

    let dto = delivery_dto(state, delivery_id).await?;
    let mut pool_conn = state.pool.acquire().await?;
    let (shop_name, shop_address) = shop_identity(&mut pool_conn).await?;

    let record = DeliveryNoteRecord {
        number: dto.delivery_number.clone().unwrap_or_default(),
        delivery_date: dto.created_at.clone(),
        customer_name: dto.customer_name.clone(),
        address: dto.address.clone(),
        contact_phone: dto.contact_phone.clone(),
        driver_note: dto.driver_note.clone(),
        vehicle_note: dto.vehicle_note.clone(),
        items: dto
            .items
            .iter()
            .map(|i| DeliveryNoteLine {
                article: i.article_number.clone(),
                name: i.product_name.clone(),
                quantity: i.quantity,
                unit_price_minor: i.unit_price_minor,
                line_total_minor: i.line_total_minor,
            })
            .collect(),
        total_units: dto.items.iter().map(|i| i.quantity).sum(),
        shop_name,
        shop_address,
    };

    let reports_dir = state.paths.reports_dir.clone();
    let pdf =
        tokio::task::spawn_blocking(move || generate_delivery_note_pdf(&reports_dir, &record))
            .await
            .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    Ok(PdfResultDto {
        report_path: pdf.path,
        pages: pdf.pages,
        bytes: pdf.bytes,
    })
}

// ---------------------------------------------------------------------------
// Sales returns, refunds, and credit notes
// ---------------------------------------------------------------------------

/// Returned-goods value with the sale discount allocated fairly across the
/// returned line. Zero when the sale has no items (mathematically undefined).
fn line_refund_alloc(
    line_total: i64,
    subtotal: i64,
    discount: i64,
    quantity: i64,
    returned_qty: i64,
) -> i64 {
    if subtotal <= 0 {
        return 0;
    }
    let goods = i128::from(line_total) * i128::from(subtotal - discount) / i128::from(subtotal);
    let unit = goods / i128::from(quantity);
    (unit * i128::from(returned_qty)).min(i64::MAX as i128) as i64
}

async fn already_returned(
    tx: &mut SqliteConnection,
    sale_item_id: i64,
    return_id: i64,
) -> Result<i64, AppError> {
    let qty: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(r.quantity), 0)
         FROM sales_return_items r
         JOIN sales_returns sr ON sr.id = r.return_id
         WHERE r.sale_item_id = ? AND sr.status = 'posted' AND sr.id != ?",
    )
    .bind(sale_item_id)
    .bind(return_id)
    .fetch_one(&mut *tx)
    .await?;
    Ok(qty)
}

#[allow(clippy::too_many_arguments)]
async fn write_restore_movement(
    tx: &mut SqliteConnection,
    location_id: i64,
    product_id: i64,
    qty: i64,
    classification: &str,
    return_id: i64,
    return_number: &str,
    unit_cost: i64,
    actor_id: i64,
) -> Result<(), AppError> {
    let label = format!("sales return {return_number}");
    match classification {
        "sellable" => {
            let seq = next_move_seq(tx, location_id).await?;
            let move_number = format!("CSR-{seq:06}");
            let movement_id = sqlx::query(
                "INSERT INTO stock_movements
                   (product_id, location_id, movement_type, quantity_delta, move_number,
                    unit_cost_minor, reference_type, reference_id, reason, created_by)
                 VALUES (?, ?, 'customer_return', ?, ?, ?, 'sales_return', ?, ?, ?)",
            )
            .bind(product_id)
            .bind(location_id)
            .bind(qty)
            .bind(&move_number)
            .bind(unit_cost)
            .bind(return_id)
            .bind(&label)
            .bind(actor_id)
            .execute(&mut *tx)
            .await?
            .last_insert_rowid();
            apply_on_hand_delta(tx, product_id, location_id, qty).await?;
            if unit_cost > 0 {
                post_cost_layer(tx, product_id, qty, unit_cost, movement_id).await?;
            }
        }
        "damaged" | "repair" => {
            let seq = next_move_seq(tx, location_id).await?;
            let move_number = format!("CSR-{seq:06}");
            sqlx::query(
                "INSERT INTO stock_movements
                   (product_id, location_id, movement_type, quantity_delta, move_number,
                    unit_cost_minor, reference_type, reference_id, reason, created_by)
                 VALUES (?, ?, 'customer_return', ?, ?, 0, 'sales_return', ?, ?, ?)",
            )
            .bind(product_id)
            .bind(location_id)
            .bind(qty)
            .bind(&move_number)
            .bind(return_id)
            .bind(&label)
            .bind(actor_id)
            .execute(&mut *tx)
            .await?;
            apply_on_hand_delta(tx, product_id, location_id, qty).await?;
            apply_damage_shift(tx, product_id, location_id, qty).await?;
        }
        "disposed" => {
            let seq = next_move_seq(tx, location_id).await?;
            let move_number = format!("CSR-{seq:06}");
            sqlx::query(
                "INSERT INTO stock_movements
                   (product_id, location_id, movement_type, quantity_delta, move_number,
                    unit_cost_minor, reference_type, reference_id, reason, created_by)
                 VALUES (?, ?, 'customer_return', ?, ?, 0, 'sales_return', ?, ?, ?)",
            )
            .bind(product_id)
            .bind(location_id)
            .bind(qty)
            .bind(&move_number)
            .bind(return_id)
            .bind(&label)
            .bind(actor_id)
            .execute(&mut *tx)
            .await?;
            apply_on_hand_delta(tx, product_id, location_id, qty).await?;
            apply_damage_shift(tx, product_id, location_id, qty).await?;

            let seq = next_move_seq(tx, location_id).await?;
            let writeoff_number = format!("CSR-{seq:06}");
            sqlx::query(
                "INSERT INTO stock_movements
                   (product_id, location_id, movement_type, quantity_delta, move_number,
                    unit_cost_minor, reference_type, reference_id, reason, created_by)
                 VALUES (?, ?, 'adjustment', ?, ?, 0, 'sales_return', ?, ?, ?)",
            )
            .bind(product_id)
            .bind(location_id)
            .bind(-qty)
            .bind(&writeoff_number)
            .bind(return_id)
            .bind("disposed return write-off")
            .bind(actor_id)
            .execute(&mut *tx)
            .await?;
            apply_damaged_delta(tx, product_id, location_id, qty).await?;
        }
        _ => {
            return Err(AppError::Validation(format!(
                "unsupported return classification '{classification}'"
            )));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn restore_returned_item(
    tx: &mut SqliteConnection,
    location_id: i64,
    sale_item_id: i64,
    qty: i64,
    classification: &str,
    return_id: i64,
    return_number: &str,
    actor_id: i64,
) -> Result<(), AppError> {
    let row =
        sqlx::query("SELECT product_id, bundle_id, unit_cost_minor FROM sale_items WHERE id = ?")
            .bind(sale_item_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("sale item {sale_item_id}")))?;

    let product_id: Option<i64> = row.get(0);
    let bundle_id: Option<i64> = row.get(1);
    let unit_cost: i64 = row.get(2);

    let targets: Vec<(i64, i64, i64)> = if let Some(pid) = product_id {
        vec![(pid, qty, unit_cost)]
    } else if bundle_id.is_some() {
        sqlx::query(
            "SELECT product_id, quantity, unit_cost_minor
             FROM sale_item_components WHERE sale_item_id = ? ORDER BY id",
        )
        .bind(sale_item_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|r| (r.get(0), r.get::<i64, _>(1) * qty, r.get::<i64, _>(2)))
        .collect()
    } else {
        Vec::new()
    };

    for (pid, unit_qty, unit_cost) in targets {
        write_restore_movement(
            tx,
            location_id,
            pid,
            unit_qty,
            classification,
            return_id,
            return_number,
            unit_cost,
            actor_id,
        )
        .await?;
    }
    Ok(())
}

pub async fn post_return(
    state: &AppState,
    principal: &Principal,
    input: SaleReturnInput,
    correlation_id: &str,
) -> Result<SaleReturnDto, AppError> {
    principal.require("sale.return")?;
    if input.items.is_empty() {
        return Err(AppError::Validation("return has no items".into()));
    }
    if !matches!(input.refund_type.as_str(), "credit" | "cash" | "exchange") {
        return Err(AppError::Validation(
            "refund_type must be credit, cash, or exchange".into(),
        ));
    }
    if input.return_date.trim().is_empty() {
        return Err(AppError::Validation("return date is required".into()));
    }
    for item in &input.items {
        if item.quantity <= 0 {
            return Err(AppError::Validation(
                "return item quantities must be positive".into(),
            ));
        }
        if !ALLOWED_CLASSIFICATIONS.contains(&item.classification.as_str()) {
            return Err(AppError::Validation(format!(
                "unsupported classification '{}'",
                item.classification
            )));
        }
    }
    let cash_mode = input.refund_type == "cash";

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let input = input.clone();
            Box::pin(async move {
                if let Some(key) = input.idempotency_key.as_deref() {
                    if !key.trim().is_empty() {
                        let existing: Option<(i64, i64)> = sqlx::query_as(
                            "SELECT id, sale_id FROM sales_returns WHERE idempotency_key = ?",
                        )
                        .bind(key)
                        .fetch_optional(&mut *tx)
                        .await?;
                        if let Some((existing_id, existing_sale)) = existing {
                            if existing_sale != input.sale_id {
                                return Err(AppError::Conflict(format!(
                                    "idempotency key '{key}' was already used for a different sale"
                                )));
                            }
                            return Ok(existing_id);
                        }
                    }
                }

                let sale = sqlx::query(
                    "SELECT id, customer_id, customer_name, location_id, sale_date, status,
                            subtotal_minor, discount_minor, total_minor, paid_minor,
                            advance_used_minor, due_minor, sale_number
                     FROM sales WHERE id = ?",
                )
                .bind(input.sale_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("sale {}", input.sale_id)))?;

                let status: String = sale.get(5);
                if status != "confirmed" {
                    return Err(AppError::Conflict(format!(
                        "sale {} cannot be returned in status '{status}'",
                        input.sale_id
                    )));
                }

                let subtotal = sale.get::<i64, _>(6);
                let discount = sale.get::<i64, _>(7);
                let total = sale.get::<i64, _>(8);
                let paid = sale.get::<i64, _>(9);
                let advance = sale.get::<i64, _>(10);
                let due = sale.get::<i64, _>(11);
                let sale_number = sale.get::<String, _>(12);
                let location_id = sale.get::<i64, _>(3);
                let customer_id = sale.get::<Option<i64>, _>(1);
                let customer_name = sale
                    .try_get::<Option<String>, _>(2)
                    .ok()
                    .flatten()
                    .filter(|s| !s.is_empty());

                if cash_mode {
                    require_active_location(&mut *tx, location_id).await?;
                }
                if let Some(cid) = customer_id {
                    require_active_location(&mut *tx, location_id).await?;
                    let _balance = customer_balance(&mut *tx, cid).await?;
                }

                #[allow(clippy::type_complexity)]
                let items_meta: Vec<(
                    i64,
                    Option<i64>,
                    Option<i64>,
                    i64,
                    i64,
                    i64,
                    i64,
                    String,
                    String,
                )> = sqlx::query(
                    "SELECT id, product_id, bundle_id, quantity, unit_price_minor,
                                line_total_minor, unit_cost_minor, article_number, product_name
                         FROM sale_items WHERE sale_id = ? ORDER BY id",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| {
                    (
                        r.get(0),
                        r.get(1),
                        r.get(2),
                        r.get(3),
                        r.get(4),
                        r.get(5),
                        r.get(6),
                        r.get(7),
                        r.get(8),
                    )
                })
                .collect();

                // Resolve and validate lines, computing the refund allocation.
                #[allow(clippy::type_complexity)]
                let mut lines: Vec<(
                    i64,
                    i64,
                    String,
                    i64,
                    i64,
                    i64,
                    String,
                    String,
                )> = Vec::new();
                let mut r_total: i64 = 0;
                for item in &input.items {
                    let Some((
                        _,
                        pid,
                        bid,
                        quantity,
                        unit_price,
                        line_total,
                        _unit_cost,
                        _article,
                        name,
                    )) = items_meta
                        .iter()
                        .find(|(id, _, _, _, _, _, _, _, _)| *id == item.sale_item_id)
                    else {
                        return Err(AppError::Validation(format!(
                            "sale item {} is not on sale {}",
                            item.sale_item_id, input.sale_id
                        )));
                    };
                    let already = already_returned(&mut *tx, item.sale_item_id, 0).await?;
                    let remaining = *quantity - already;
                    if item.quantity > remaining {
                        return Err(AppError::Validation(format!(
                            "sale item {}: only {remaining} of {quantity} remain returnable",
                            item.sale_item_id
                        )));
                    }
                    let line_refund = line_refund_alloc(
                        *line_total,
                        subtotal,
                        discount,
                        *quantity,
                        item.quantity,
                    );
                    let unit_refund = if item.quantity > 0 {
                        line_refund / item.quantity
                    } else {
                        0
                    };
                    r_total += line_refund;
                    lines.push((
                        item.sale_item_id,
                        item.quantity,
                        item.classification.clone(),
                        *unit_price,
                        unit_refund,
                        line_refund,
                        (*pid).map(|p| p.to_string()).unwrap_or_else(|| {
                            bid.map(|b| format!("bundle {b}")).unwrap_or_default()
                        }),
                        name.clone(),
                    ));
                }

                let collected = paid + advance;
                let refundable = (collected + r_total - total).max(0);

                if customer_id.is_none() && !cash_mode && refundable > 0 {
                    return Err(AppError::Validation(format!(
                        "cannot refund walk-in sale {sale_number} as {} — credit notes require a customer; refund to cash instead",
                        input.refund_type
                    )));
                }

                // Validate the refund position before consuming any document numbers.
                let cash_account_id = if cash_mode {
                    if refundable > 0 {
                        let account: Option<i64> = sqlx::query_scalar(
                            "SELECT 1 FROM cash_accounts WHERE id = ? AND is_active = 1",
                        )
                        .bind(input.cash_account_id.unwrap_or(0))
                        .fetch_optional(&mut *tx)
                        .await?;
                        if account.is_none() {
                            return Err(AppError::NotFound("cash account".into()));
                        }
                        require_cash_balance(
                            &mut *tx,
                            input.cash_account_id.unwrap_or(0),
                            -refundable,
                            "sales return refund",
                        )
                        .await?;
                    }
                    input.cash_account_id
                } else {
                    None
                };

                let return_number = next_document_number(&mut *tx, "sales_return").await?;
                let return_id = sqlx::query(
                    "INSERT INTO sales_returns
                       (return_number, sale_id, customer_id, location_id, return_date,
                        status, refund_type, cash_account_id, notes, idempotency_key, created_by)
                     VALUES (?, ?, ?, ?, ?, 'posted', ?, ?, ?, ?, ?)",
                )
                .bind(&return_number)
                .bind(input.sale_id)
                .bind(customer_id)
                .bind(location_id)
                .bind(input.return_date.as_str())
                .bind(input.refund_type.as_str())
                .bind(cash_account_id)
                .bind(input.notes.as_deref())
                .bind(input.idempotency_key.as_deref())
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                for (
                    sale_item_id,
                    qty,
                    classification,
                    unit_price,
                    unit_refund,
                    line_refund,
                    _art,
                    _name,
                ) in &lines
                {
                    let meta = items_meta
                        .iter()
                        .find(|(id, _, _, _, _, _, _, _, _)| *id == *sale_item_id);
                    sqlx::query(
                        "INSERT INTO sales_return_items
                           (return_id, sale_item_id, product_id, bundle_id, article_number,
                            product_name, quantity, unit_price_minor, unit_refund_minor,
                            line_refund_minor, classification)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(return_id)
                    .bind(sale_item_id)
                    .bind(meta.and_then(|m| m.1))
                    .bind(meta.and_then(|m| m.2))
                    .bind(meta.map(|m| m.7.clone()).unwrap_or_default())
                    .bind(meta.map(|m| m.8.clone()).unwrap_or_default())
                    .bind(qty)
                    .bind(unit_price)
                    .bind(unit_refund)
                    .bind(line_refund)
                    .bind(classification)
                    .execute(&mut *tx)
                    .await?;

                    restore_returned_item(
                        &mut *tx,
                        location_id,
                        *sale_item_id,
                        *qty,
                        classification,
                        return_id,
                        &return_number,
                        actor_id,
                    )
                    .await?;
                }

                if let Some(cid) = customer_id {
                    record_ledger(
                        &mut *tx,
                        cid,
                        "sales_return",
                        "sales_return",
                        return_id,
                        -r_total,
                        &format!("sales return {return_number}"),
                        actor_id,
                    )
                    .await?;

                    if cash_mode && refundable > 0 {
                        record_ledger(
                            &mut *tx,
                            cid,
                            "payment_refund",
                            "sales_return",
                            return_id,
                            refundable,
                            &format!("cash refund for {return_number}"),
                            actor_id,
                        )
                        .await?;
                    }
                }

                if cash_mode && refundable > 0 {
                    record_cash_entry(
                        &mut *tx,
                        cash_account_id.unwrap_or(0),
                        "sale_refund",
                        -refundable,
                        "sales_return",
                        return_id,
                        &format!("refund for {return_number}"),
                        actor_id,
                    )
                    .await?;
                }

                let credit_note_minor = if !cash_mode && refundable > 0 {
                    let credit_number = next_document_number(&mut *tx, "credit_note").await?;
                    let credit_id = sqlx::query(
                        "INSERT INTO credit_notes
                           (credit_number, customer_id, return_id, amount_minor, status,
                            notes, created_by)
                         VALUES (?, ?, ?, ?, 'open', ?, ?)",
                    )
                    .bind(&credit_number)
                    .bind(customer_id.unwrap_or(0))
                    .bind(return_id)
                    .bind(refundable)
                    .bind(format!("from sales return {return_number}"))
                    .bind(actor_id)
                    .execute(&mut *tx)
                    .await?
                    .last_insert_rowid();
                    let _ = credit_id;
                    refundable
                } else {
                    0
                };

                let new_due = due.saturating_sub(r_total).max(0);
                sqlx::query(
                    "UPDATE sales SET due_minor = ?,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
                )
                .bind(new_due)
                .bind(input.sale_id)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "UPDATE sales_returns
                        SET total_minor = ?, total_refund_minor = ?, cash_refund_minor = ?,
                            credit_note_minor = ?, posted_by = ?,
                            posted_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(r_total)
                .bind(refundable)
                .bind(if cash_mode { refundable } else { 0 })
                .bind(credit_note_minor)
                .bind(actor_id)
                .bind(return_id)
                .execute(&mut *tx)
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "sale.return.post",
                    "sales_return",
                    return_id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "return_number": return_number,
                        "sale_id": input.sale_id,
                        "sale_number": sale_number,
                        "total_minor": r_total,
                        "refundable_minor": refundable,
                        "refund_type": input.refund_type,
                        "customer": customer_name,
                    })),
                )
                .await?;
                Ok(return_id)
            })
        })
        .await?;

    sale_return_dto(state, result).await
}

pub async fn void_return(
    state: &AppState,
    principal: &Principal,
    input: ReturnVoidInput,
    correlation_id: &str,
) -> Result<SaleReturnDto, AppError> {
    principal.require("sale.return")?;

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let row = sqlx::query(
                    "SELECT id, status, sale_id, customer_id, location_id, refund_type,
                            total_minor, cash_refund_minor, credit_note_minor,
                            cash_account_id, return_number
                     FROM sales_returns WHERE id = ?",
                )
                .bind(input.return_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("sales return {}", input.return_id)))?;

                let status: String = row.get(1);
                if status != "posted" {
                    return Err(AppError::Conflict(format!(
                        "sales return {} cannot be voided in status '{status}'",
                        input.return_id
                    )));
                }
                let sale_id = row.get::<i64, _>(2);
                let customer_id = row.get::<Option<i64>, _>(3);
                let location_id = row.get::<i64, _>(4);
                let r_total = row.get::<i64, _>(6);
                let cash_refund = row.get::<i64, _>(7);
                let credit_note_minor = row.get::<i64, _>(8);
                let cash_account_id = row.get::<Option<i64>, _>(9);
                let return_number = row.get::<String, _>(10);

                if credit_note_minor > 0 {
                    let applied: String = sqlx::query_scalar(
                        "SELECT status FROM credit_notes WHERE return_id = ?",
                    )
                    .bind(input.return_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    if applied != "open" {
                        return Err(AppError::Conflict(format!(
                            "sales return {} has a credit note that is already '{applied}'",
                            input.return_id
                        )));
                    }
                    sqlx::query(
                        "UPDATE credit_notes SET status = 'voided',
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE return_id = ?",
                    )
                    .bind(input.return_id)
                    .execute(&mut *tx)
                    .await?;
                }

                let items: Vec<(i64, i64, String)> = sqlx::query(
                    "SELECT sale_item_id, quantity, classification
                     FROM sales_return_items WHERE return_id = ? ORDER BY id",
                )
                .bind(input.return_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .collect();

                let reason = input
                    .reason
                    .clone()
                    .unwrap_or_else(|| "return voided".into());
                for (sale_item_id, qty, classification) in &items {
                    let meta = sqlx::query(
                        "SELECT product_id, bundle_id, unit_cost_minor
                         FROM sale_items WHERE id = ?",
                    )
                    .bind(sale_item_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                    let Some(meta) = meta else { continue };
                    let product_id: Option<i64> = meta.get(0);
                    let bundle_id: Option<i64> = meta.get(1);
                    let unit_cost: i64 = meta.get(2);

                    let targets: Vec<(i64, i64, i64)> = if let Some(pid) = product_id {
                        vec![(pid, *qty, unit_cost)]
                    } else if bundle_id.is_some() {
                        sqlx::query(
                            "SELECT product_id, quantity, unit_cost_minor
                             FROM sale_item_components WHERE sale_item_id = ? ORDER BY id",
                        )
                        .bind(sale_item_id)
                        .fetch_all(&mut *tx)
                        .await?
                        .into_iter()
                        .map(|r| (r.get(0), r.get::<i64, _>(1) * qty, r.get::<i64, _>(2)))
                        .collect()
                    } else {
                        Vec::new()
                    };

                    match classification.as_str() {
                        "sellable" => {
                            for (pid, unit_qty, unit_cost) in targets {
                                let seq = next_move_seq(&mut *tx, location_id).await?;
                                let move_number = format!("CSR-{seq:06}");
                                let avg = withdraw_cost_layers(&mut *tx, pid, unit_qty).await?;
                                sqlx::query(
                                    "INSERT INTO stock_movements
                                       (product_id, location_id, movement_type, quantity_delta,
                                        move_number, unit_cost_minor, reference_type, reference_id,
                                        reason, created_by)
                                     VALUES (?, ?, 'cancellation_reversal', ?, ?, ?, 'sales_return', ?, ?, ?)",
                                )
                                .bind(pid)
                                .bind(location_id)
                                .bind(-unit_qty)
                                .bind(&move_number)
                                .bind(avg.unwrap_or(unit_cost))
                                .bind(input.return_id)
                                .bind(classification.as_str())
                                .bind(actor_id)
                                .execute(&mut *tx)
                                .await?;
                                apply_on_hand_delta(&mut *tx, pid, location_id, -unit_qty).await?;
                            }
                        }
                        "damaged" | "repair" => {
                            for (pid, unit_qty, _) in targets {
                                let seq = next_move_seq(&mut *tx, location_id).await?;
                                let move_number = format!("CSR-{seq:06}");
                                sqlx::query(
                                    "INSERT INTO stock_movements
                                       (product_id, location_id, movement_type, quantity_delta,
                                        move_number, unit_cost_minor, reference_type, reference_id,
                                        reason, created_by)
                                     VALUES (?, ?, 'cancellation_reversal', ?, ?, 0, 'sales_return', ?, ?, ?)",
                                )
                                .bind(pid)
                                .bind(location_id)
                                .bind(-unit_qty)
                                .bind(&move_number)
                                .bind(input.return_id)
                                .bind(classification.as_str())
                                .bind(actor_id)
                                .execute(&mut *tx)
                                .await?;
                                apply_damaged_delta(&mut *tx, pid, location_id, unit_qty).await?;
                            }
                        }
                        "disposed" => {
                            for (pid, unit_qty, _) in targets {
                                let seq = next_move_seq(&mut *tx, location_id).await?;
                                let move_number = format!("CSR-{seq:06}");
                                sqlx::query(
                                    "INSERT INTO stock_movements
                                       (product_id, location_id, movement_type, quantity_delta,
                                        move_number, unit_cost_minor, reference_type, reference_id,
                                        reason, created_by)
                                     VALUES (?, ?, 'cancellation_reversal', ?, ?, 0, 'sales_return', ?, ?, ?)",
                                )
                                .bind(pid)
                                .bind(location_id)
                                .bind(unit_qty)
                                .bind(&move_number)
                                .bind(input.return_id)
                                .bind(classification.as_str())
                                .bind(actor_id)
                                .execute(&mut *tx)
                                .await?;
                                apply_damaged_delta(&mut *tx, pid, location_id, -unit_qty).await?;
                            }
                        }
                        _ => {
                            return Err(AppError::Internal(format!(
                                "unsupported classification '{classification}' on return"
                            )));
                        }
                    }
                }

                if let Some(cid) = customer_id {
                    if cash_refund > 0 {
                        record_ledger(
                            &mut *tx,
                            cid,
                            "payment_refund",
                            "sales_return",
                            input.return_id,
                            -cash_refund,
                            &format!("void return {return_number}"),
                            actor_id,
                        )
                        .await?;
                    }
                    record_ledger(
                        &mut *tx,
                        cid,
                        "return_void",
                        "sales_return",
                        input.return_id,
                        r_total,
                        &format!("void return {return_number}"),
                        actor_id,
                    )
                    .await?;

                    let sale_row = sqlx::query(
                        "SELECT total_minor, paid_minor, advance_used_minor, due_minor
                         FROM sales WHERE id = ?",
                    )
                    .bind(sale_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    let total = sale_row.get::<i64, _>(0);
                    let paid = sale_row.get::<i64, _>(1);
                    let advance = sale_row.get::<i64, _>(2);
                    let due_minor = sale_row.get::<i64, _>(3);
                    let restored = (due_minor + r_total).min((total - paid - advance).max(0));
                    sqlx::query(
                        "UPDATE sales SET due_minor = ?,
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
                    )
                    .bind(restored)
                    .bind(sale_id)
                    .execute(&mut *tx)
                    .await?;
                }

                if cash_refund > 0 {
                    let account = cash_account_id.ok_or_else(|| {
                        AppError::Internal("cash return missing cash account".into())
                    })?;
                    record_cash_entry(
                        &mut *tx,
                        account,
                        "return_void",
                        cash_refund,
                        "sales_return",
                        input.return_id,
                        &reason,
                        actor_id,
                    )
                    .await?;
                }

                sqlx::query(
                    "UPDATE sales_returns
                        SET status = 'voided', voided_by = ?, notes = COALESCE(notes, '') ||
                            CASE WHEN ? IS NOT NULL THEN '; ' || ? ELSE '' END,
                            voided_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(actor_id)
                .bind(input.reason.as_deref())
                .bind(input.reason.as_deref())
                .bind(input.return_id)
                .execute(&mut *tx)
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "sale.return.void",
                    "sales_return",
                    input.return_id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "return_number": return_number,
                        "sale_id": sale_id,
                        "reason": reason,
                    })),
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    sale_return_dto(state, input.return_id).await
}

pub async fn list_returns(
    state: &AppState,
    principal: &Principal,
    input: ReturnListInput,
) -> Result<Vec<SaleReturnDto>, AppError> {
    principal.require("sale.return")?;
    let limit = input.limit.unwrap_or(100).min(500);
    let offset = input.offset.unwrap_or(0).max(0);

    let mut predicates = String::from("WHERE 1 = 1");
    if input.sale_id.is_some() {
        predicates.push_str(" AND sale_id = ?");
    }
    if input.customer_id.is_some() {
        predicates.push_str(" AND customer_id = ?");
    }
    let sql =
        format!("SELECT id FROM sales_returns {predicates} ORDER BY id DESC LIMIT ? OFFSET ?");

    let mut query = sqlx::query(&sql);
    if let Some(sale_id) = input.sale_id {
        query = query.bind(sale_id);
    }
    if let Some(customer_id) = input.customer_id {
        query = query.bind(customer_id);
    }
    query = query.bind(limit).bind(offset);

    let ids: Vec<i64> = query
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|r| r.get::<i64, _>(0))
        .collect();

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(sale_return_dto(state, id).await?);
    }
    Ok(out)
}

pub async fn get_return(
    state: &AppState,
    principal: &Principal,
    return_id: i64,
) -> Result<SaleReturnDto, AppError> {
    principal.require("sale.return")?;
    sale_return_dto(state, return_id).await
}

pub async fn list_credit_notes(
    state: &AppState,
    principal: &Principal,
    input: CreditNoteListInput,
) -> Result<Vec<CreditNoteDto>, AppError> {
    principal.require("credit.note.use")?;
    let limit = input.limit.unwrap_or(100).min(500);
    let offset = input.offset.unwrap_or(0).max(0);

    let mut predicates = String::from("WHERE 1 = 1");
    if input.customer_id.is_some() {
        predicates.push_str(" AND customer_id = ?");
    }
    if input.status.is_some() {
        predicates.push_str(" AND status = ?");
    }
    let sql = format!("SELECT id FROM credit_notes {predicates} ORDER BY id DESC LIMIT ? OFFSET ?");

    let mut query = sqlx::query(&sql);
    if let Some(customer_id) = input.customer_id {
        query = query.bind(customer_id);
    }
    if let Some(status) = &input.status {
        query = query.bind(status);
    }
    query = query.bind(limit).bind(offset);

    let ids: Vec<i64> = query
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|r| r.get::<i64, _>(0))
        .collect();

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(credit_note_dto(state, id).await?);
    }
    Ok(out)
}

pub async fn credit_note_pdf(
    state: &AppState,
    principal: &Principal,
    credit_id: i64,
) -> Result<PdfResultDto, AppError> {
    principal.require("credit.note.use")?;

    let dto = credit_note_dto(state, credit_id).await?;
    let customer_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM customers WHERE id = ?")
            .bind(dto.customer_id)
            .fetch_optional(&state.pool)
            .await?;
    let return_number: Option<String> =
        sqlx::query_scalar("SELECT return_number FROM sales_returns WHERE id = ?")
            .bind(dto.return_id)
            .fetch_optional(&state.pool)
            .await?;

    let mut pool_conn = state.pool.acquire().await?;
    let (shop_name, shop_address) = shop_identity(&mut pool_conn).await?;

    let record = CreditNoteRecord {
        number: dto.credit_number.clone().unwrap_or_default(),
        note_date: dto.created_at.clone(),
        customer_name: customer_name.or(dto.customer_id.to_string().into()),
        return_number: return_number
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        amount_minor: dto.amount_minor,
        reason: "credit against customer account".to_string(),
        shop_name,
        shop_address,
    };

    let reports_dir = state.paths.reports_dir.clone();
    let pdf = tokio::task::spawn_blocking(move || generate_credit_note_pdf(&reports_dir, &record))
        .await
        .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    Ok(PdfResultDto {
        report_path: pdf.path,
        pages: pdf.pages,
        bytes: pdf.bytes,
    })
}

// ---------------------------------------------------------------------------
// Damage records
// ---------------------------------------------------------------------------

pub async fn record_damage(
    state: &AppState,
    principal: &Principal,
    input: DamageRecordInput,
    correlation_id: &str,
) -> Result<DamageRecordDto, AppError> {
    principal.require("damage.record")?;
    if input.quantity <= 0 {
        return Err(AppError::Validation(
            "damage quantity must be positive".into(),
        ));
    }
    if !ALLOWED_SOURCES.contains(&input.source.as_str()) {
        return Err(AppError::Validation(format!(
            "unsupported damage source '{}'",
            input.source
        )));
    }
    if input.estimated_loss_minor.unwrap_or(0) < 0 {
        return Err(AppError::Validation(
            "estimated loss cannot be negative".into(),
        ));
    }
    if input.damage_date.trim().is_empty() {
        return Err(AppError::Validation("damage date is required".into()));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                require_positive(&mut *tx, input.product_id).await?;
                require_active_location(&mut *tx, input.location_id).await?;

                let on_hand: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(on_hand, 0) FROM stock_balances
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);
                if input.quantity > on_hand {
                    return Err(AppError::InsufficientStock(format!(
                        "cannot record damage of {}: only {on_hand} on hand",
                        input.quantity
                    )));
                }

                let sold_units: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(SUM(di.quantity), 0)
                     FROM delivery_items di
                     JOIN deliveries d ON d.id = di.delivery_id
                     JOIN sale_items si ON si.id = di.sale_item_id
                     WHERE si.product_id = ? AND d.status != 'cancelled'",
                )
                .bind(input.product_id)
                .fetch_one(&mut *tx)
                .await?;
                let _ = sold_units;

                let seq = next_move_seq(&mut *tx, input.location_id).await?;
                let move_number = format!("DMG-{seq:06}");
                let movement_id = sqlx::query(
                    "INSERT INTO stock_movements (
                        product_id, location_id, movement_type, quantity_delta,
                        move_number, reason, created_by
                     ) VALUES (?, ?, 'damage', ?, ?, ?, ?)",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .bind(-input.quantity)
                .bind(&move_number)
                .bind(input.reason.as_deref())
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                apply_damage_shift(
                    &mut *tx,
                    input.product_id,
                    input.location_id,
                    input.quantity,
                )
                .await?;

                let damage_number = next_document_number(&mut *tx, "damage_record").await?;
                let damage_id = sqlx::query(
                    "INSERT INTO damage_records
                       (damage_number, product_id, location_id, quantity, damage_date,
                        source, reason, estimated_loss_minor, photo_path, created_by)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&damage_number)
                .bind(input.product_id)
                .bind(input.location_id)
                .bind(input.quantity)
                .bind(input.damage_date.as_str())
                .bind(input.source.as_str())
                .bind(input.reason.as_deref())
                .bind(input.estimated_loss_minor.unwrap_or(0))
                .bind(input.photo_path.as_deref())
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "damage.record",
                    "damage_record",
                    damage_id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "damage_number": damage_number,
                        "product_id": input.product_id,
                        "location_id": input.location_id,
                        "quantity": input.quantity,
                        "move_number": move_number,
                        "movement_id": movement_id,
                    })),
                )
                .await?;
                Ok(damage_id)
            })
        })
        .await?;

    damage_dto(state, result).await
}

pub async fn decide_damage(
    state: &AppState,
    principal: &Principal,
    input: DamageDecisionInput,
    correlation_id: &str,
) -> Result<DamageRecordDto, AppError> {
    principal.require("damage.record")?;
    if !ALLOWED_DECISIONS.contains(&input.decision.as_str()) {
        return Err(AppError::Validation(format!(
            "unsupported damage decision '{}'",
            input.decision
        )));
    }
    if input.decision == "damaged_sale" && input.linked_sale_id.is_none() {
        return Err(AppError::Validation(
            "damaged_sale requires a linked sale".into(),
        ));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let row = sqlx::query(
                    "SELECT product_id, location_id, quantity, status, decision
                     FROM damage_records WHERE id = ?",
                )
                .bind(input.damage_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("damage record {}", input.damage_id)))?;

                let status: String = row.get(3);
                if status != "open" {
                    return Err(AppError::Conflict(format!(
                        "damage record {} is already resolved",
                        input.damage_id
                    )));
                }
                let product_id = row.get::<i64, _>(0);
                let location_id = row.get::<i64, _>(1);
                let quantity = row.get::<i64, _>(2);

                if let Some(sale_id) = input.linked_sale_id {
                    let sale_ok: Option<String> =
                        sqlx::query_scalar("SELECT status FROM sales WHERE id = ?")
                            .bind(sale_id)
                            .fetch_optional(&mut *tx)
                            .await?;
                    match sale_ok {
                        Some(st) if st == "confirmed" => {}
                        Some(_) => {
                            return Err(AppError::Validation(format!(
                                "linked sale {sale_id} is not confirmed"
                            )));
                        }
                        None => {
                            return Err(AppError::NotFound(format!("sale {sale_id}")));
                        }
                    }
                }

                let damaged: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(damaged, 0) FROM stock_balances
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(product_id)
                .bind(location_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);
                if quantity > damaged {
                    return Err(AppError::InsufficientStock(format!(
                        "cannot resolve damage of {quantity}: only {damaged} damaged on hand"
                    )));
                }

                let seq = next_move_seq(&mut *tx, location_id).await?;
                let move_number = format!("DMG-{seq:06}");
                let (movement_type, delta, reason): (&str, i64, String) =
                    match input.decision.as_str() {
                        "repair" => (
                            "repair_recovery",
                            quantity,
                            "repair recovery from damage record".into(),
                        ),
                        "supplier_return" => (
                            "supplier_return",
                            -quantity,
                            "returned to supplier from damage record".into(),
                        ),
                        "damaged_sale" => (
                            "adjustment",
                            -quantity,
                            "damaged-sale from damage record".into(),
                        ),
                        "write_off" => (
                            "adjustment",
                            -quantity,
                            format!(
                                "write-off: {}",
                                input.decision_note.as_deref().unwrap_or("damage write-off")
                            ),
                        ),
                        _ => unreachable!(),
                    };

                let movement_id = sqlx::query(
                    "INSERT INTO stock_movements (
                        product_id, location_id, movement_type, quantity_delta,
                        move_number, reference_type, reference_id, reason, created_by
                     ) VALUES (?, ?, ?, ?, ?, 'damage_record', ?, ?, ?)",
                )
                .bind(product_id)
                .bind(location_id)
                .bind(movement_type)
                .bind(delta)
                .bind(&move_number)
                .bind(input.damage_id)
                .bind(&reason)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                match input.decision.as_str() {
                    "repair" => {
                        apply_repair_shift(&mut *tx, product_id, location_id, quantity).await?
                    }
                    _ => apply_damaged_delta(&mut *tx, product_id, location_id, quantity).await?,
                }

                sqlx::query(
                    "UPDATE damage_records
                        SET status = 'resolved', decision = ?, decision_note = ?,
                            linked_sale_id = ?, resolved_by = ?,
                            resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(input.decision.as_str())
                .bind(input.decision_note.as_deref())
                .bind(input.linked_sale_id)
                .bind(actor_id)
                .bind(input.damage_id)
                .execute(&mut *tx)
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "damage.decide",
                    "damage_record",
                    input.damage_id,
                    &correlation,
                    None,
                    Some(serde_json::json!({
                        "damage_id": input.damage_id,
                        "decision": input.decision,
                        "movement_id": movement_id,
                        "move_number": move_number,
                    })),
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    damage_dto(state, input.damage_id).await
}

pub async fn list_damage(
    state: &AppState,
    principal: &Principal,
    input: DamageListInput,
) -> Result<Vec<DamageRecordDto>, AppError> {
    principal.require("damage.record")?;
    let limit = input.limit.unwrap_or(100).min(500);
    let offset = input.offset.unwrap_or(0).max(0);

    let mut predicates = String::from("WHERE 1 = 1");
    if input.product_id.is_some() {
        predicates.push_str(" AND product_id = ?");
    }
    if input.location_id.is_some() {
        predicates.push_str(" AND location_id = ?");
    }
    if input.status.is_some() {
        predicates.push_str(" AND status = ?");
    }
    let sql =
        format!("SELECT id FROM damage_records {predicates} ORDER BY id DESC LIMIT ? OFFSET ?");

    let mut query = sqlx::query(&sql);
    if let Some(product_id) = input.product_id {
        query = query.bind(product_id);
    }
    if let Some(location_id) = input.location_id {
        query = query.bind(location_id);
    }
    if let Some(status) = &input.status {
        query = query.bind(status);
    }
    query = query.bind(limit).bind(offset);

    let ids: Vec<i64> = query
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|r| r.get::<i64, _>(0))
        .collect();

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(damage_dto(state, id).await?);
    }
    Ok(out)
}

pub async fn get_damage(
    state: &AppState,
    principal: &Principal,
    damage_id: i64,
) -> Result<DamageRecordDto, AppError> {
    principal.require("damage.record")?;
    damage_dto(state, damage_id).await
}
