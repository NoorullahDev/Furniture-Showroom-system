use sqlx::sqlite::SqliteConnection;
use sqlx::Row;

use crate::application::auth::Principal;
use crate::dto::inventory::{
    AdjustStockInput, CountLineDto, CountLineInput, CountSessionDto, DamageStockInput, LocationDto,
    LowStockItemDto, OpeningBatchErrorDto, OpeningBatchInput, OpeningBatchResultDto,
    PostCountInput, PostStockInput, ReleaseStockInput, ReverseMovementInput, StartCountInput,
    StockBalanceDto, StockMovementDto, TransferStockInput, ValuationLineDto,
};
use crate::error::AppError;
use crate::infrastructure::AuditInput;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn require_positive(conn: &mut SqliteConnection, product_id: i64) -> Result<(), AppError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM products WHERE id = ? AND archived_at IS NULL")
            .bind(product_id)
            .fetch_optional(conn)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!("product {product_id}")));
    }
    Ok(())
}

async fn require_active_location(
    conn: &mut SqliteConnection,
    location_id: i64,
) -> Result<(), AppError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM locations WHERE id = ? AND is_active = 1")
            .bind(location_id)
            .fetch_optional(conn)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!("location {location_id}")));
    }
    Ok(())
}

async fn allow_negative_stock(conn: &mut SqliteConnection) -> Result<bool, AppError> {
    let val: Option<String> = sqlx::query_scalar(
        "SELECT value_json FROM settings WHERE key = 'inventory.allow_negative'",
    )
    .fetch_optional(conn)
    .await?;
    let Some(raw) = val else {
        return Ok(false);
    };
    let enabled = match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(serde_json::Value::Bool(b)) => b,
        Ok(serde_json::Value::String(s)) => {
            matches!(s.to_ascii_lowercase().as_str(), "true" | "1" | "yes")
        }
        _ => false,
    };
    Ok(enabled)
}

/// Increment the location's move_num_seq and return the new value. Called
/// inside a transaction so no two threads can see the same number.
async fn next_move_seq(conn: &mut SqliteConnection, location_id: i64) -> Result<i64, AppError> {
    sqlx::query(
        "UPDATE locations SET move_num_seq = move_num_seq + 1,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(location_id)
    .execute(&mut *conn)
    .await?;
    let seq: i64 = sqlx::query_scalar("SELECT move_num_seq FROM locations WHERE id = ?")
        .bind(location_id)
        .fetch_one(&mut *conn)
        .await?;
    Ok(seq)
}

/// Ensure a stock_balances row exists, then apply a signed delta to on_hand.
async fn apply_on_hand_delta(
    conn: &mut SqliteConnection,
    product_id: i64,
    location_id: i64,
    delta: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO stock_balances (product_id, location_id, on_hand, reserved, damaged)
         VALUES (?, ?, ?, 0, 0)
         ON CONFLICT (product_id, location_id) DO UPDATE SET
           on_hand = on_hand + excluded.on_hand",
    )
    .bind(product_id)
    .bind(location_id)
    .bind(delta)
    .execute(conn)
    .await?;
    Ok(())
}

/// Move `qty` units from on_hand to damaged for the given product/location.
async fn apply_damage_shift(
    conn: &mut SqliteConnection,
    product_id: i64,
    location_id: i64,
    qty: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE stock_balances
         SET on_hand  = MAX(on_hand - ?, 0),
             damaged  = damaged + ?
         WHERE product_id = ? AND location_id = ?",
    )
    .bind(qty)
    .bind(qty)
    .bind(product_id)
    .bind(location_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Move `qty` units from damaged back to on_hand for the given product/location.
async fn apply_repair_shift(
    conn: &mut SqliteConnection,
    product_id: i64,
    location_id: i64,
    qty: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE stock_balances
         SET damaged  = MAX(damaged - ?, 0),
             on_hand  = on_hand + ?
         WHERE product_id = ? AND location_id = ?",
    )
    .bind(qty)
    .bind(qty)
    .bind(product_id)
    .bind(location_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Post a single FIFO cost layer for a receipt movement.
async fn post_cost_layer(
    conn: &mut SqliteConnection,
    product_id: i64,
    qty: i64,
    unit_cost: i64,
    movement_id: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO inventory_cost_layers (product_id, quantity, unit_cost_minor, movement_id)
         VALUES (?, ?, ?, ?)",
    )
    .bind(product_id)
    .bind(qty)
    .bind(unit_cost)
    .bind(movement_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Withdraw `qty` from the oldest available cost layers (FIFO). Returns the
/// weighted-average unit cost of the withdrawn quantity, used for issue
/// movements.
async fn withdraw_cost_layers(
    conn: &mut SqliteConnection,
    product_id: i64,
    mut qty: i64,
) -> Result<Option<i64>, AppError> {
    let layers: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, quantity FROM inventory_cost_layers
         WHERE product_id = ? AND quantity > 0
         ORDER BY created_at ASC, id ASC",
    )
    .bind(product_id)
    .fetch_all(&mut *conn)
    .await?;

    if layers.is_empty() {
        return Ok(None);
    }

    let mut total_cost: i64 = 0;
    let mut total_qty: i64 = 0;
    for (id, available) in &layers {
        if qty <= 0 {
            break;
        }
        let take = std::cmp::min(*available, qty);
        sqlx::query("UPDATE inventory_cost_layers SET quantity = quantity - ? WHERE id = ?")
            .bind(take)
            .bind(id)
            .execute(&mut *conn)
            .await?;
        // Fetch unit_cost for this layer
        let unit: i64 =
            sqlx::query_scalar("SELECT unit_cost_minor FROM inventory_cost_layers WHERE id = ?")
                .bind(id)
                .fetch_one(&mut *conn)
                .await?;
        total_cost += unit * take;
        total_qty += take;
        qty -= take;
    }
    if total_qty == 0 {
        return Ok(None);
    }
    Ok(Some(total_cost / total_qty))
}

// ---------------------------------------------------------------------------
// Opening stock
// ---------------------------------------------------------------------------

pub async fn post_opening(
    state: &AppState,
    principal: &Principal,
    input: PostStockInput,
    correlation_id: &str,
) -> Result<StockMovementDto, AppError> {
    principal.require("inventory.create")?;
    if input.quantity <= 0 {
        return Err(AppError::Validation(
            "opening stock quantity must be positive".into(),
        ));
    }
    let cost = input.unit_cost_minor.unwrap_or(0);
    if cost < 0 {
        return Err(AppError::Validation("unit cost cannot be negative".into()));
    }
    let reason = input.reason.unwrap_or_default();

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = reason.clone();
            Box::pin(async move {
                require_positive(&mut *tx, input.product_id).await?;
                require_active_location(&mut *tx, input.location_id).await?;

                let seq = next_move_seq(&mut *tx, input.location_id).await?;
                let move_number = format!("OPN-{seq:06}");

                let result = sqlx::query(
                    "INSERT INTO stock_movements (
                        product_id, location_id, movement_type, quantity_delta,
                        move_number, unit_cost_minor, reason, created_by
                     ) VALUES (?, ?, 'opening', ?, ?, ?, ?, ?)",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .bind(input.quantity)
                .bind(&move_number)
                .bind(cost)
                .bind(&reason)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                apply_on_hand_delta(
                    &mut *tx,
                    input.product_id,
                    input.location_id,
                    input.quantity,
                )
                .await?;
                post_cost_layer(&mut *tx, input.product_id, input.quantity, cost, result).await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "inventory.opening".into(),
                            entity_type: Some("stock_movement".into()),
                            entity_id: Some(result.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "product_id": input.product_id,
                                    "location_id": input.location_id,
                                    "quantity": input.quantity,
                                    "unit_cost_minor": cost,
                                    "move_number": move_number,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(result)
            })
        })
        .await?;

    get_movement(state, principal, result).await
}

// ---------------------------------------------------------------------------
// Location transfer
// ---------------------------------------------------------------------------

pub async fn post_transfer(
    state: &AppState,
    principal: &Principal,
    input: TransferStockInput,
    correlation_id: &str,
) -> Result<Vec<StockMovementDto>, AppError> {
    principal.require("inventory.create")?;
    if input.quantity <= 0 {
        return Err(AppError::Validation(
            "transfer quantity must be positive".into(),
        ));
    }
    if input.from_location_id == input.to_location_id {
        return Err(AppError::Validation(
            "source and destination must differ".into(),
        ));
    }
    let reason = input.reason.unwrap_or_default();

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let (out_id, in_id) = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = reason.clone();
            Box::pin(async move {
                require_positive(&mut *tx, input.product_id).await?;
                require_active_location(&mut *tx, input.from_location_id).await?;
                require_active_location(&mut *tx, input.to_location_id).await?;

                // Check available stock at source
                let on_hand: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(on_hand,0) FROM stock_balances
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(input.product_id)
                .bind(input.from_location_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);

                let reserved: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(reserved,0) FROM stock_balances
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(input.product_id)
                .bind(input.from_location_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);

                let damaged: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(damaged,0) FROM stock_balances
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(input.product_id)
                .bind(input.from_location_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);

                let available = on_hand - reserved - damaged;
                if !allow_negative_stock(&mut *tx).await? && input.quantity > available {
                    return Err(AppError::InsufficientStock(format!(
                        "requested {qty} but only {available} available at source",
                        qty = input.quantity
                    )));
                }

                let from_seq = next_move_seq(&mut *tx, input.from_location_id).await?;
                let from_number = format!("TRF-{from_seq:06}");
                let to_seq = next_move_seq(&mut *tx, input.to_location_id).await?;
                let to_number = format!("TRF-{to_seq:06}");

                let out_id = sqlx::query(
                    "INSERT INTO stock_movements (
                        product_id, location_id, movement_type, quantity_delta,
                        move_number, reason, created_by
                     ) VALUES (?, ?, 'transfer_out', ?, ?, ?, ?)",
                )
                .bind(input.product_id)
                .bind(input.from_location_id)
                .bind(-input.quantity)
                .bind(&from_number)
                .bind(&reason)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                let in_id = sqlx::query(
                    "INSERT INTO stock_movements (
                        product_id, location_id, movement_type, quantity_delta,
                        move_number, reason, created_by
                     ) VALUES (?, ?, 'transfer_in', ?, ?, ?, ?)",
                )
                .bind(input.product_id)
                .bind(input.to_location_id)
                .bind(input.quantity)
                .bind(&to_number)
                .bind(&reason)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                apply_on_hand_delta(
                    &mut *tx,
                    input.product_id,
                    input.from_location_id,
                    -input.quantity,
                )
                .await?;
                apply_on_hand_delta(
                    &mut *tx,
                    input.product_id,
                    input.to_location_id,
                    input.quantity,
                )
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "inventory.transfer".into(),
                            entity_type: Some("stock_movement".into()),
                            entity_id: Some(out_id.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "product_id": input.product_id,
                                    "from_location_id": input.from_location_id,
                                    "to_location_id": input.to_location_id,
                                    "quantity": input.quantity,
                                    "from_number": from_number,
                                    "to_number": to_number,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok((out_id, in_id))
            })
        })
        .await?;

    let out = get_movement(state, principal, out_id).await?;
    let r#in = get_movement(state, principal, in_id).await?;
    Ok(vec![out, r#in])
}

// ---------------------------------------------------------------------------
// Internal adjustment (manual)
// ---------------------------------------------------------------------------

pub async fn post_adjust(
    state: &AppState,
    principal: &Principal,
    input: AdjustStockInput,
    correlation_id: &str,
) -> Result<StockMovementDto, AppError> {
    principal.require("inventory.create")?;
    if input.adjustment_qty == 0 {
        return Err(AppError::Validation(
            "adjustment quantity cannot be zero".into(),
        ));
    }
    let reason = input.reason.unwrap_or_default();
    let unit_cost = input.unit_cost_minor.unwrap_or(0);
    if unit_cost < 0 {
        return Err(AppError::Validation("unit cost cannot be negative".into()));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = reason.clone();
            Box::pin(async move {
                require_positive(&mut *tx, input.product_id).await?;
                require_active_location(&mut *tx, input.location_id).await?;

                if input.adjustment_qty < 0 {
                    let on_hand: i64 = sqlx::query_scalar(
                        "SELECT COALESCE(on_hand,0) FROM stock_balances
                         WHERE product_id = ? AND location_id = ?",
                    )
                    .bind(input.product_id)
                    .bind(input.location_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .unwrap_or(0);

                    if !allow_negative_stock(&mut *tx).await?
                        && input.adjustment_qty.unsigned_abs() as i64 > on_hand
                    {
                        return Err(AppError::InsufficientStock(format!(
                            "adjustment of {} would put stock below zero (on hand: {on_hand})",
                            input.adjustment_qty
                        )));
                    }
                }

                let seq = next_move_seq(&mut *tx, input.location_id).await?;
                let move_number = format!("ADJ-{seq:06}");

                let movement_id = sqlx::query(
                    "INSERT INTO stock_movements (
                        product_id, location_id, movement_type, quantity_delta,
                        move_number, unit_cost_minor, reason, created_by
                     ) VALUES (?, ?, 'adjustment', ?, ?, ?, ?, ?)",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .bind(input.adjustment_qty)
                .bind(&move_number)
                .bind(unit_cost)
                .bind(&reason)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                apply_on_hand_delta(
                    &mut *tx,
                    input.product_id,
                    input.location_id,
                    input.adjustment_qty,
                )
                .await?;

                if input.adjustment_qty > 0 && unit_cost > 0 {
                    post_cost_layer(
                        &mut *tx,
                        input.product_id,
                        input.adjustment_qty,
                        unit_cost,
                        movement_id,
                    )
                    .await?;
                } else if input.adjustment_qty < 0 {
                    let _ = withdraw_cost_layers(
                        &mut *tx,
                        input.product_id,
                        input.adjustment_qty.unsigned_abs() as i64,
                    )
                    .await?;
                }

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "inventory.adjust".into(),
                            entity_type: Some("stock_movement".into()),
                            entity_id: Some(movement_id.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "product_id": input.product_id,
                                    "location_id": input.location_id,
                                    "adjustment_qty": input.adjustment_qty,
                                    "unit_cost_minor": unit_cost,
                                    "move_number": move_number,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(movement_id)
            })
        })
        .await?;

    get_movement(state, principal, result).await
}

// ---------------------------------------------------------------------------
// Damage classification
// ---------------------------------------------------------------------------

pub async fn post_damage(
    state: &AppState,
    principal: &Principal,
    input: DamageStockInput,
    correlation_id: &str,
) -> Result<StockMovementDto, AppError> {
    principal.require("inventory.create")?;
    if input.quantity <= 0 {
        return Err(AppError::Validation(
            "damage quantity must be positive".into(),
        ));
    }
    let reason = input.reason.unwrap_or_default();

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = reason.clone();
            Box::pin(async move {
                require_positive(&mut *tx, input.product_id).await?;
                require_active_location(&mut *tx, input.location_id).await?;

                let on_hand: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(on_hand,0) FROM stock_balances
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);

                if input.quantity > on_hand {
                    return Err(AppError::InsufficientStock(format!(
                        "cannot mark {qty} as damaged: only {on_hand} on hand",
                        qty = input.quantity
                    )));
                }

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
                .bind(&reason)
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

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "inventory.damage".into(),
                            entity_type: Some("stock_movement".into()),
                            entity_id: Some(movement_id.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "product_id": input.product_id,
                                    "location_id": input.location_id,
                                    "quantity": input.quantity,
                                    "move_number": move_number,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(movement_id)
            })
        })
        .await?;

    get_movement(state, principal, result).await
}

// ---------------------------------------------------------------------------
// Repair recovery (damaged → sellable)
// ---------------------------------------------------------------------------

pub async fn post_repair(
    state: &AppState,
    principal: &Principal,
    input: DamageStockInput,
    correlation_id: &str,
) -> Result<StockMovementDto, AppError> {
    principal.require("inventory.create")?;
    if input.quantity <= 0 {
        return Err(AppError::Validation(
            "repair quantity must be positive".into(),
        ));
    }
    let reason = input.reason.unwrap_or_default();

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = reason.clone();
            Box::pin(async move {
                require_positive(&mut *tx, input.product_id).await?;
                require_active_location(&mut *tx, input.location_id).await?;

                let damaged: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(damaged,0) FROM stock_balances
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);

                if input.quantity > damaged {
                    return Err(AppError::InsufficientStock(format!(
                        "cannot recover {qty} from damaged: only {damaged} damaged",
                        qty = input.quantity
                    )));
                }

                let seq = next_move_seq(&mut *tx, input.location_id).await?;
                let move_number = format!("RPR-{seq:06}");

                let movement_id = sqlx::query(
                    "INSERT INTO stock_movements (
                        product_id, location_id, movement_type, quantity_delta,
                        move_number, reason, created_by
                     ) VALUES (?, ?, 'repair_recovery', ?, ?, ?, ?)",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .bind(input.quantity)
                .bind(&move_number)
                .bind(&reason)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                apply_repair_shift(
                    &mut *tx,
                    input.product_id,
                    input.location_id,
                    input.quantity,
                )
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "inventory.repair".into(),
                            entity_type: Some("stock_movement".into()),
                            entity_id: Some(movement_id.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "product_id": input.product_id,
                                    "location_id": input.location_id,
                                    "quantity": input.quantity,
                                    "move_number": move_number,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(movement_id)
            })
        })
        .await?;

    get_movement(state, principal, result).await
}

// ---------------------------------------------------------------------------
// Reservation
// ---------------------------------------------------------------------------

pub async fn post_reserve(
    state: &AppState,
    principal: &Principal,
    input: ReleaseStockInput,
    correlation_id: &str,
) -> Result<StockMovementDto, AppError> {
    principal.require("inventory.create")?;
    if input.quantity <= 0 {
        return Err(AppError::Validation(
            "reservation quantity must be positive".into(),
        ));
    }
    let reason = input.reason.unwrap_or_default();

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = reason.clone();
            Box::pin(async move {
                require_positive(&mut *tx, input.product_id).await?;
                require_active_location(&mut *tx, input.location_id).await?;

                let on_hand: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(on_hand,0) FROM stock_balances
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);

                let reserved: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(reserved,0) FROM stock_balances
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);

                let available = on_hand - reserved;
                if !allow_negative_stock(&mut *tx).await? && input.quantity > available {
                    return Err(AppError::InsufficientStock(format!(
                        "cannot reserve {qty}: only {available} available",
                        qty = input.quantity
                    )));
                }

                let seq = next_move_seq(&mut *tx, input.location_id).await?;
                let move_number = format!("RSV-{seq:06}");

                let movement_id = sqlx::query(
                    "INSERT INTO stock_movements (
                        product_id, location_id, movement_type, quantity_delta,
                        move_number, reason, created_by
                     ) VALUES (?, ?, 'reservation', ?, ?, ?, ?)",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .bind(input.quantity)
                .bind(&move_number)
                .bind(&reason)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                sqlx::query(
                    "UPDATE stock_balances SET reserved = reserved + ?
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(input.quantity)
                .bind(input.product_id)
                .bind(input.location_id)
                .execute(&mut *tx)
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "inventory.reserve".into(),
                            entity_type: Some("stock_movement".into()),
                            entity_id: Some(movement_id.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "product_id": input.product_id,
                                    "location_id": input.location_id,
                                    "quantity": input.quantity,
                                    "move_number": move_number,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(movement_id)
            })
        })
        .await?;

    get_movement(state, principal, result).await
}

// ---------------------------------------------------------------------------
// Release (reservation → available)
// ---------------------------------------------------------------------------

pub async fn post_release(
    state: &AppState,
    principal: &Principal,
    input: ReleaseStockInput,
    correlation_id: &str,
) -> Result<StockMovementDto, AppError> {
    principal.require("inventory.create")?;
    if input.quantity <= 0 {
        return Err(AppError::Validation(
            "release quantity must be positive".into(),
        ));
    }
    let reason = input.reason.unwrap_or_default();

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = reason.clone();
            Box::pin(async move {
                require_positive(&mut *tx, input.product_id).await?;
                require_active_location(&mut *tx, input.location_id).await?;

                let reserved: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(reserved,0) FROM stock_balances
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or(0);

                if input.quantity > reserved {
                    return Err(AppError::InsufficientStock(format!(
                        "cannot release {qty}: only {reserved} reserved",
                        qty = input.quantity
                    )));
                }

                let seq = next_move_seq(&mut *tx, input.location_id).await?;
                let move_number = format!("REL-{seq:06}");

                let movement_id = sqlx::query(
                    "INSERT INTO stock_movements (
                        product_id, location_id, movement_type, quantity_delta,
                        move_number, reason, created_by
                     ) VALUES (?, ?, 'release', ?, ?, ?, ?)",
                )
                .bind(input.product_id)
                .bind(input.location_id)
                .bind(-input.quantity)
                .bind(&move_number)
                .bind(&reason)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                sqlx::query(
                    "UPDATE stock_balances SET reserved = MAX(reserved - ?, 0)
                     WHERE product_id = ? AND location_id = ?",
                )
                .bind(input.quantity)
                .bind(input.product_id)
                .bind(input.location_id)
                .execute(&mut *tx)
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "inventory.release".into(),
                            entity_type: Some("stock_movement".into()),
                            entity_id: Some(movement_id.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "product_id": input.product_id,
                                    "location_id": input.location_id,
                                    "quantity": input.quantity,
                                    "move_number": move_number,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(movement_id)
            })
        })
        .await?;

    get_movement(state, principal, result).await
}

// ---------------------------------------------------------------------------
// Read helpers
// ---------------------------------------------------------------------------

pub async fn list_locations(
    state: &AppState,
    _principal: &Principal,
) -> Result<Vec<LocationDto>, AppError> {
    let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(
        "SELECT id, name, type, is_active FROM locations WHERE is_active = 1 ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| LocationDto {
            id: r.try_get(0).unwrap_or_default(),
            name: r.try_get(1).unwrap_or_default(),
            location_type: r.try_get(2).unwrap_or_default(),
            is_active: r.try_get::<i64, _>(3).unwrap_or(1) == 1,
        })
        .collect())
}

pub async fn list_balances(
    state: &AppState,
    _principal: &Principal,
    location_id: Option<i64>,
) -> Result<Vec<StockBalanceDto>, AppError> {
    let rows: Vec<sqlx::sqlite::SqliteRow> = if let Some(loc) = location_id {
        sqlx::query(
            "SELECT b.product_id, p.article_number, p.name, pi.thumbnail_path,
                    b.location_id, l.name, b.on_hand, b.reserved, b.damaged,
                    p.minimum_stock,
                    (b.on_hand - b.reserved - b.damaged) AS available
             FROM stock_balances b
             JOIN products p ON p.id = b.product_id
             JOIN locations l ON l.id = b.location_id
             LEFT JOIN product_images pi ON pi.product_id = p.id AND pi.is_primary = 1
             WHERE b.location_id = ? AND p.archived_at IS NULL
             ORDER BY p.name, p.article_number",
        )
        .bind(loc)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query(
            "SELECT b.product_id, p.article_number, p.name, pi.thumbnail_path,
                    b.location_id, l.name, b.on_hand, b.reserved, b.damaged,
                    p.minimum_stock,
                    (b.on_hand - b.reserved - b.damaged) AS available
             FROM stock_balances b
             JOIN products p ON p.id = b.product_id
             JOIN locations l ON l.id = b.location_id
             LEFT JOIN product_images pi ON pi.product_id = p.id AND pi.is_primary = 1
             WHERE p.archived_at IS NULL
             ORDER BY p.name, p.article_number",
        )
        .fetch_all(&state.pool)
        .await?
    };
    Ok(rows
        .into_iter()
        .map(|r| StockBalanceDto {
            product_id: r.try_get(0).unwrap_or_default(),
            article_number: r.try_get(1).unwrap_or_default(),
            product_name: r.try_get(2).unwrap_or_default(),
            thumbnail_path: r
                .try_get::<Option<String>, _>(3)
                .ok()
                .flatten()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            location_id: r.try_get(4).unwrap_or_default(),
            location_name: r.try_get(5).unwrap_or_default(),
            on_hand: r.try_get(6).unwrap_or_default(),
            reserved: r.try_get(7).unwrap_or_default(),
            damaged: r.try_get(8).unwrap_or_default(),
            minimum_stock: r.try_get(9).unwrap_or_default(),
            available: r.try_get(10).unwrap_or_default(),
        })
        .collect())
}

pub async fn list_movements(
    state: &AppState,
    _principal: &Principal,
    product_id: Option<i64>,
    location_id: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<StockMovementDto>, AppError> {
    let limit = limit.unwrap_or(100).min(500);

    let mut sql = String::from(
        "SELECT m.id, m.product_id, p.article_number, p.name,
                m.location_id, l.name, m.movement_type, m.quantity_delta,
                m.move_number, m.unit_cost_minor, m.reference_type, m.reference_id,
                m.reason, m.reversal_of_id, m.created_by, m.created_at
         FROM stock_movements m
         JOIN products p ON p.id = m.product_id
         JOIN locations l ON l.id = m.location_id
         WHERE 1 = 1",
    );
    if product_id.is_some() {
        sql.push_str(" AND m.product_id = ?");
    }
    if location_id.is_some() {
        sql.push_str(" AND m.location_id = ?");
    }
    sql.push_str(" ORDER BY m.created_at DESC, m.id DESC LIMIT ?");

    let mut query = sqlx::query(&sql);
    if let Some(pid) = product_id {
        query = query.bind(pid);
    }
    if let Some(lid) = location_id {
        query = query.bind(lid);
    }
    query = query.bind(limit);

    let rows: Vec<sqlx::sqlite::SqliteRow> = query.fetch_all(&state.pool).await?;
    Ok(rows
        .into_iter()
        .map(|r| StockMovementDto {
            id: r.try_get(0).unwrap_or_default(),
            product_id: r.try_get(1).unwrap_or_default(),
            article_number: r.try_get(2).ok(),
            product_name: r.try_get(3).ok(),
            location_id: r.try_get(4).unwrap_or_default(),
            location_name: r.try_get(5).ok(),
            movement_type: r.try_get(6).unwrap_or_default(),
            quantity_delta: r.try_get(7).unwrap_or_default(),
            move_number: r.try_get(8).ok(),
            unit_cost_minor: r.try_get(9).ok(),
            reference_type: r.try_get(10).ok(),
            reference_id: r.try_get(11).ok(),
            reason: r.try_get(12).ok(),
            reversal_of_id: r.try_get(13).ok(),
            created_by: r.try_get(14).unwrap_or_default(),
            created_at: r.try_get(15).unwrap_or_default(),
        })
        .collect())
}

async fn get_movement(
    state: &AppState,
    _principal: &Principal,
    movement_id: i64,
) -> Result<StockMovementDto, AppError> {
    let row = sqlx::query(
        "SELECT m.id, m.product_id, p.article_number, p.name,
                m.location_id, l.name, m.movement_type, m.quantity_delta,
                m.move_number, m.unit_cost_minor, m.reference_type, m.reference_id,
                m.reason, m.reversal_of_id, m.created_by, m.created_at
         FROM stock_movements m
         JOIN products p ON p.id = m.product_id
         JOIN locations l ON l.id = m.location_id
         WHERE m.id = ?",
    )
    .bind(movement_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(r) = row else {
        return Err(AppError::NotFound(format!("stock movement {movement_id}")));
    };
    Ok(StockMovementDto {
        id: r.try_get(0).unwrap_or_default(),
        product_id: r.try_get(1).unwrap_or_default(),
        article_number: r.try_get(2).ok(),
        product_name: r.try_get(3).ok(),
        location_id: r.try_get(4).unwrap_or_default(),
        location_name: r.try_get(5).ok(),
        movement_type: r.try_get(6).unwrap_or_default(),
        quantity_delta: r.try_get(7).unwrap_or_default(),
        move_number: r.try_get(8).ok(),
        unit_cost_minor: r.try_get(9).ok(),
        reference_type: r.try_get(10).ok(),
        reference_id: r.try_get(11).ok(),
        reason: r.try_get(12).ok(),
        reversal_of_id: r.try_get(13).ok(),
        created_by: r.try_get(14).unwrap_or_default(),
        created_at: r.try_get(15).unwrap_or_default(),
    })
}

pub async fn valuation(
    state: &AppState,
    _principal: &Principal,
) -> Result<Vec<ValuationLineDto>, AppError> {
    let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(
        "SELECT v.product_id, v.article_number, v.product_name,
                v.unit_cost_minor, v.sellable_qty, v.value_minor
         FROM current_valuation v
         ORDER BY v.product_name",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| ValuationLineDto {
            product_id: r.try_get(0).unwrap_or_default(),
            article_number: r.try_get(1).unwrap_or_default(),
            product_name: r.try_get(2).unwrap_or_default(),
            unit_cost_minor: r.try_get(3).unwrap_or_default(),
            sellable_qty: r.try_get(4).unwrap_or_default(),
            value_minor: r.try_get(5).unwrap_or_default(),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Movement reversal
// ---------------------------------------------------------------------------

pub async fn reverse_movement(
    state: &AppState,
    principal: &Principal,
    input: ReverseMovementInput,
    correlation_id: &str,
) -> Result<StockMovementDto, AppError> {
    principal.require("inventory.create")?;

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = input.reason.clone().unwrap_or_default();
            Box::pin(async move {
                let orig: Option<(i64, i64, i64, String, Option<i64>)> = sqlx::query_as(
                    "SELECT id, product_id, location_id, movement_type, reversal_of_id
                     FROM stock_movements WHERE id = ?",
                )
                .bind(input.movement_id)
                .fetch_optional(&mut *tx)
                .await?;

                let Some((orig_id, product_id, location_id, _orig_type, existing_reversal)) = orig
                else {
                    return Err(AppError::NotFound(format!(
                        "stock movement {}",
                        input.movement_id
                    )));
                };

                if existing_reversal.is_some() {
                    return Err(AppError::Validation(format!(
                        "movement {orig_id} has already been reversed"
                    )));
                }

                // Also guard: a movement can only be reversed once — check if any
                // existing reversal already points to this movement.
                let already_reversed: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM stock_movements WHERE reversal_of_id = ? LIMIT 1",
                )
                .bind(orig_id)
                .fetch_optional(&mut *tx)
                .await?;
                if already_reversed.is_some() {
                    return Err(AppError::Validation(format!(
                        "movement {orig_id} has already been reversed"
                    )));
                }

                let orig_delta: i64 =
                    sqlx::query_scalar("SELECT quantity_delta FROM stock_movements WHERE id = ?")
                        .bind(orig_id)
                        .fetch_one(&mut *tx)
                        .await?;

                let seq = next_move_seq(&mut *tx, location_id).await?;
                let move_number = format!("REV-{seq:06}");
                let reversal_delta = -orig_delta;

                let reversal_id = sqlx::query(
                    "INSERT INTO stock_movements (
                        product_id, location_id, movement_type, quantity_delta,
                        move_number, reason, created_by, reversal_of_id
                     ) VALUES (?, ?, 'cancellation_reversal', ?, ?, ?, ?, ?)",
                )
                .bind(product_id)
                .bind(location_id)
                .bind(reversal_delta)
                .bind(&move_number)
                .bind(&reason)
                .bind(actor_id)
                .bind(orig_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                apply_on_hand_delta(&mut *tx, product_id, location_id, reversal_delta).await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "inventory.reverse".into(),
                            entity_type: Some("stock_movement".into()),
                            entity_id: Some(reversal_id.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "original_movement_id": orig_id,
                                    "reversal_movement_id": reversal_id,
                                    "reversal_delta": reversal_delta,
                                    "move_number": move_number,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(reversal_id)
            })
        })
        .await?;

    get_movement(state, principal, result).await
}

// ---------------------------------------------------------------------------
// Low-stock list
// ---------------------------------------------------------------------------

pub async fn list_low_stock(
    state: &AppState,
    _principal: &Principal,
) -> Result<Vec<LowStockItemDto>, AppError> {
    let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(
        "SELECT p.id, p.article_number, p.name, pi.thumbnail_path,
                p.minimum_stock,
                COALESCE(SUM(b.on_hand - b.reserved - b.damaged), 0) AS total_available,
                COALESCE(SUM(b.on_hand), 0) AS total_on_hand
         FROM products p
         LEFT JOIN stock_balances b ON b.product_id = p.id
         LEFT JOIN product_images pi ON pi.product_id = p.id AND pi.is_primary = 1
         WHERE p.archived_at IS NULL AND p.track_stock = 1
         GROUP BY p.id
         HAVING total_available < p.minimum_stock AND p.minimum_stock > 0
         ORDER BY (p.minimum_stock - total_available) DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| LowStockItemDto {
            product_id: r.try_get(0).unwrap_or_default(),
            article_number: r.try_get(1).unwrap_or_default(),
            product_name: r.try_get(2).unwrap_or_default(),
            thumbnail_path: r
                .try_get::<Option<String>, _>(3)
                .ok()
                .flatten()
                .filter(|s| !s.is_empty()),
            minimum_stock: r.try_get(4).unwrap_or_default(),
            total_available: r.try_get(5).unwrap_or_default(),
            total_on_hand: r.try_get(6).unwrap_or_default(),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Stock-count sessions
// ---------------------------------------------------------------------------

pub async fn start_count(
    state: &AppState,
    principal: &Principal,
    input: StartCountInput,
    correlation_id: &str,
) -> Result<CountSessionDto, AppError> {
    principal.require("inventory.count")?;
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let notes = input.notes.clone().unwrap_or_default();
            Box::pin(async move {
                require_active_location(&mut *tx, input.location_id).await?;

                let open: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM inventory_count_sessions
                     WHERE location_id = ? AND status = 'open'",
                )
                .bind(input.location_id)
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(open_id) = open {
                    return Err(AppError::Validation(format!(
                        "a count session ({open_id}) is already open at this location"
                    )));
                }

                sqlx::query(
                    "UPDATE locations SET session_num_seq = session_num_seq + 1,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
                )
                .bind(input.location_id)
                .execute(&mut *tx)
                .await?;
                let seq: i64 =
                    sqlx::query_scalar("SELECT session_num_seq FROM locations WHERE id = ?")
                        .bind(input.location_id)
                        .fetch_one(&mut *tx)
                        .await?;
                let session_number = format!("CNT-{seq:06}");

                let session_id = sqlx::query(
                    "INSERT INTO inventory_count_sessions (
                        location_id, session_number, notes, created_by
                     ) VALUES (?, ?, ?, ?)",
                )
                .bind(input.location_id)
                .bind(&session_number)
                .bind(&notes)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                sqlx::query(
                    "INSERT INTO inventory_count_lines (session_id, product_id, expected_qty, counted_qty, variance_qty)
                     SELECT ?, b.product_id, (b.on_hand - b.reserved - b.damaged), 0, 0
                     FROM stock_balances b
                     JOIN products p ON p.id = b.product_id
                     WHERE b.location_id = ? AND b.on_hand > 0 AND p.archived_at IS NULL",
                )
                .bind(session_id)
                .bind(input.location_id)
                .execute(&mut *tx)
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "inventory.count_start".into(),
                            entity_type: Some("inventory_count_session".into()),
                            entity_id: Some(session_id.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "session_id": session_id,
                                    "session_number": session_number,
                                    "location_id": input.location_id,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;

                Ok(CountSessionDto {
                    id: session_id,
                    location_id: input.location_id,
                    location_name: None,
                    session_number: Some(session_number),
                    status: "open".into(),
                    notes: Some(notes),
                    created_by: actor_id,
                    created_at: String::new(),
                    posted_at: None,
                })
            })
        })
        .await
}

pub async fn add_count_line(
    state: &AppState,
    principal: &Principal,
    input: CountLineInput,
    _correlation_id: &str,
) -> Result<CountLineDto, AppError> {
    principal.require("inventory.count")?;

    let session: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, location_id FROM inventory_count_sessions WHERE id = ? AND status = 'open'",
    )
    .bind(input.session_id)
    .fetch_optional(&state.pool)
    .await?;
    if session.is_none() {
        return Err(AppError::NotFound(format!(
            "open count session {}",
            input.session_id
        )));
    }

    sqlx::query(
        "INSERT INTO inventory_count_lines (session_id, product_id, expected_qty, counted_qty, variance_qty)
         VALUES (?, ?, 0, ?, ?)
         ON CONFLICT (session_id, product_id) DO UPDATE SET
           counted_qty = excluded.counted_qty,
           variance_qty = excluded.counted_qty - expected_qty",
    )
    .bind(input.session_id)
    .bind(input.product_id)
    .bind(input.counted_qty)
    .bind(input.counted_qty)
    .execute(&state.pool)
    .await?;

    let row: Option<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT id, session_id, product_id, expected_qty, counted_qty
         FROM inventory_count_lines WHERE session_id = ? AND product_id = ?",
    )
    .bind(input.session_id)
    .bind(input.product_id)
    .fetch_optional(&state.pool)
    .await?;

    let (id, session_id, product_id, expected_qty, counted_qty) =
        row.ok_or_else(|| AppError::NotFound("count line".into()))?;
    Ok(CountLineDto {
        id,
        session_id,
        product_id,
        article_number: None,
        product_name: None,
        expected_qty,
        counted_qty,
        variance_qty: counted_qty - expected_qty,
    })
}

pub async fn list_count_lines(
    state: &AppState,
    _principal: &Principal,
    session_id: i64,
) -> Result<Vec<CountLineDto>, AppError> {
    let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(
        "SELECT cl.id, cl.session_id, cl.product_id, p.article_number, p.name,
                cl.expected_qty, cl.counted_qty, cl.variance_qty
         FROM inventory_count_lines cl
         JOIN products p ON p.id = cl.product_id
         WHERE cl.session_id = ?
         ORDER BY p.article_number",
    )
    .bind(session_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| CountLineDto {
            id: r.try_get(0).unwrap_or_default(),
            session_id: r.try_get(1).unwrap_or_default(),
            product_id: r.try_get(2).unwrap_or_default(),
            article_number: r.try_get(3).ok(),
            product_name: r.try_get(4).ok(),
            expected_qty: r.try_get(5).unwrap_or_default(),
            counted_qty: r.try_get(6).unwrap_or_default(),
            variance_qty: r.try_get(7).unwrap_or_default(),
        })
        .collect())
}

pub async fn post_count(
    state: &AppState,
    principal: &Principal,
    input: PostCountInput,
    correlation_id: &str,
) -> Result<Vec<StockMovementDto>, AppError> {
    principal.require("inventory.count")?;

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let posted_ids = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let session_id_for_reason = input.session_id;
            Box::pin(async move {
                let session: Option<(i64, i64, String, Option<String>)> = sqlx::query_as(
                    "SELECT id, location_id, session_number, notes
                     FROM inventory_count_sessions WHERE id = ? AND status = 'open'",
                )
                .bind(input.session_id)
                .fetch_optional(&mut *tx)
                .await?;

                let Some((_sid, location_id, _session_number, _notes)) = session else {
                    return Err(AppError::NotFound(format!(
                        "open count session {}",
                        input.session_id
                    )));
                };

                let lines: Vec<(i64, i64, i64, i64)> = sqlx::query_as(
                    "SELECT product_id, expected_qty, counted_qty, variance_qty
                     FROM inventory_count_lines
                     WHERE session_id = ? AND variance_qty != 0",
                )
                .bind(input.session_id)
                .fetch_all(&mut *tx)
                .await?;

                let mut posted_ids = Vec::new();
                for (product_id, _expected, _counted, variance) in &lines {
                    let seq = next_move_seq(&mut *tx, location_id).await?;
                    let move_number = format!("ADJ-{seq:06}");

                    let movement_id = sqlx::query(
                        "INSERT INTO stock_movements (
                            product_id, location_id, movement_type, quantity_delta,
                            move_number, reason, created_by
                         ) VALUES (?, ?, 'stock_count_correction', ?, ?, ?, ?)",
                    )
                    .bind(product_id)
                    .bind(location_id)
                    .bind(variance)
                    .bind(&move_number)
                    .bind(format!(
                        "count correction for session {session_id_for_reason}"
                    ))
                    .bind(actor_id)
                    .execute(&mut *tx)
                    .await?
                    .last_insert_rowid();

                    apply_on_hand_delta(&mut *tx, *product_id, location_id, *variance).await?;

                    if *variance < 0 {
                        let _ = withdraw_cost_layers(
                            &mut *tx,
                            *product_id,
                            variance.unsigned_abs() as i64,
                        )
                        .await?;
                    }

                    posted_ids.push(movement_id);
                }

                sqlx::query(
                    "UPDATE inventory_count_sessions SET status = 'posted',
                     posted_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                     WHERE id = ?",
                )
                .bind(session_id_for_reason)
                .execute(&mut *tx)
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session),
                            action: "inventory.count_post".into(),
                            entity_type: Some("inventory_count_session".into()),
                            entity_id: Some(session_id_for_reason.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "session_id": session_id_for_reason,
                                    "corrections_posted": posted_ids.len(),
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;

                Ok(posted_ids)
            })
        })
        .await?;

    let mut results = Vec::with_capacity(posted_ids.len());
    for mv_id in &posted_ids {
        let mv = get_movement(state, principal, *mv_id).await?;
        results.push(mv);
    }
    Ok(results)
}

pub async fn list_count_sessions(
    state: &AppState,
    _principal: &Principal,
    location_id: Option<i64>,
) -> Result<Vec<CountSessionDto>, AppError> {
    let mut sql = String::from(
        "SELECT cs.id, cs.location_id, l.name, cs.session_number, cs.status,
                cs.notes, cs.created_by, cs.created_at, cs.posted_at
         FROM inventory_count_sessions cs
         JOIN locations l ON l.id = cs.location_id",
    );
    if location_id.is_some() {
        sql.push_str(" WHERE cs.location_id = ?");
    }
    sql.push_str(" ORDER BY cs.created_at DESC LIMIT 50");

    let mut query = sqlx::query(&sql);
    if let Some(lid) = location_id {
        query = query.bind(lid);
    }
    let rows: Vec<sqlx::sqlite::SqliteRow> = query.fetch_all(&state.pool).await?;
    Ok(rows
        .into_iter()
        .map(|r| CountSessionDto {
            id: r.try_get(0).unwrap_or_default(),
            location_id: r.try_get(1).unwrap_or_default(),
            location_name: r.try_get(2).ok(),
            session_number: r.try_get(3).ok(),
            status: r.try_get(4).unwrap_or_default(),
            notes: r.try_get(5).ok(),
            created_by: r.try_get(6).unwrap_or_default(),
            created_at: r.try_get(7).unwrap_or_default(),
            posted_at: r.try_get(8).ok(),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Opening-stock batch import
// ---------------------------------------------------------------------------

pub async fn post_opening_batch(
    state: &AppState,
    principal: &Principal,
    input: OpeningBatchInput,
    correlation_id: &str,
) -> Result<OpeningBatchResultDto, AppError> {
    principal.require("inventory.create")?;

    // Resolve article numbers to product IDs first so article mismatches are
    // reported without posting anything.
    let mut resolved = Vec::with_capacity(input.rows.len());
    let mut errors = Vec::new();

    for (idx, row) in input.rows.iter().enumerate() {
        if row.quantity <= 0 {
            errors.push(OpeningBatchErrorDto {
                row_index: idx,
                article_number: row.article_number.clone(),
                error: "quantity must be positive".into(),
            });
            continue;
        }
        let norm = row.article_number.trim().to_uppercase();
        if norm.is_empty() {
            errors.push(OpeningBatchErrorDto {
                row_index: idx,
                article_number: row.article_number.clone(),
                error: "missing article number".into(),
            });
            continue;
        }
        let product_id: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM products WHERE article_number_norm = ? AND archived_at IS NULL",
        )
        .bind(&norm)
        .fetch_optional(&state.pool)
        .await?;
        let Some(pid) = product_id else {
            errors.push(OpeningBatchErrorDto {
                row_index: idx,
                article_number: row.article_number.clone(),
                error: format!("no active product with article '{}'", row.article_number),
            });
            continue;
        };
        resolved.push((idx, pid, row));
    }

    let mut posted_count: i64 = 0;
    for (idx, pid, row) in resolved {
        let result = post_opening(
            state,
            principal,
            PostStockInput {
                product_id: pid,
                location_id: row.location_id,
                quantity: row.quantity,
                unit_cost_minor: row.unit_cost_minor,
                reason: row.reason.clone(),
            },
            correlation_id,
        )
        .await;
        match result {
            Ok(_) => {
                posted_count += 1;
            }
            Err(e) => {
                errors.push(OpeningBatchErrorDto {
                    row_index: idx,
                    article_number: row.article_number.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(OpeningBatchResultDto {
        posted_count,
        error_count: errors.len() as i64,
        errors,
    })
}
