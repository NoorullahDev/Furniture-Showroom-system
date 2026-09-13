use sqlx::sqlite::SqliteConnection;
use sqlx::Row;

use crate::application::auth::Principal;
use crate::application::cash::{record_cash_entry, require_cash_balance};
use crate::application::customers::{customer_balance, record_ledger, require_customer};
use crate::application::documents::{next_document_number, replay_guard};
use crate::application::inventory::{
    apply_on_hand_delta, next_move_seq, post_cost_layer, require_active_location,
    withdraw_cost_layers,
};
use crate::application::sets::{bundle_cost_estimate, require_bundle};
use crate::application::suppliers::state_audit;
use crate::dto::sales::{
    SaleCancelInput, SaleComponentDto, SaleConfirmInput, SaleCreateInput, SaleDeleteInput,
    SaleDto, SaleEditInput, SaleItemDto,
};
use crate::dto::PdfResultDto;
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::infrastructure::{generate_invoice_pdf, InvoiceLine, InvoiceRecord};
use crate::state::AppState;

async fn sale_dto(state: &AppState, sale_id: i64) -> Result<SaleDto, AppError> {
    let row = sqlx::query(
        "SELECT id, sale_number, kind, customer_id, customer_name, location_id,
                sale_date, status, subtotal_minor, discount_minor, delivery_charge_minor,
                tax_minor, total_minor, paid_minor, advance_used_minor, due_minor,
                cost_minor, notes, created_at, confirmed_at, confirmed_by
         FROM sales WHERE id = ?",
    )
    .bind(sale_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("sale {sale_id}")))?;

    let items: Vec<SaleItemDto> = sqlx::query(
        "SELECT id, product_id, bundle_id, article_number, product_name,
                quantity, unit_price_minor, line_total_minor, unit_cost_minor, line_cost_minor
         FROM sale_items WHERE sale_id = ? ORDER BY sort_order, id",
    )
    .bind(sale_id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| {
        let id = r.get::<i64, _>(0);
        SaleItemDto {
            id,
            product_id: r.get(1),
            bundle_id: r.get(2),
            article_number: r.get(3),
            product_name: r.get(4),
            quantity: r.get(5),
            unit_price_minor: r.get(6),
            line_total_minor: r.get(7),
            unit_cost_minor: r.get(8),
            line_cost_minor: r.get(9),
            components: Vec::new(),
        }
    })
    .collect();

    let mut out = SaleDto {
        id: row.get(0),
        sale_number: row
            .try_get::<Option<String>, _>(1)
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()),
        kind: row.get(2),
        customer_id: row.try_get(3).ok(),
        customer_name: row.try_get(4).ok(),
        location_id: row.get(5),
        sale_date: row.get(6),
        status: row.get(7),
        subtotal_minor: row.get(8),
        discount_minor: row.get(9),
        delivery_charge_minor: row.get(10),
        tax_minor: row.get(11),
        total_minor: row.get(12),
        paid_minor: row.get(13),
        advance_used_minor: row.get(14),
        due_minor: row.get(15),
        cost_minor: row.get(16),
        notes: row.try_get(17).ok(),
        items,
        created_at: row.get(18),
        confirmed_at: row.try_get(19).ok(),
        confirmed_by: row.try_get(20).ok(),
    };

    let comps: Vec<(i64, i64, String, String, i64, i64, i64)> = sqlx::query(
        "SELECT c.sale_item_id, c.product_id, c.article_number, c.product_name,
                c.quantity, c.unit_cost_minor, c.line_cost_minor
         FROM sale_item_components c
         JOIN sale_items i ON i.id = c.sale_item_id
         WHERE i.sale_id = ? ORDER BY c.id",
    )
    .bind(sale_id)
    .fetch_all(&state.pool)
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
        )
    })
    .collect();

    for item in &mut out.items {
        item.components = comps
            .iter()
            .filter(|(sid, _, _, _, _, _, _)| *sid == item.id)
            .map(|(_, pid, a, n, q, uc, lc)| SaleComponentDto {
                product_id: *pid,
                article_number: a.clone(),
                product_name: n.clone(),
                quantity: *q,
                unit_cost_minor: *uc,
                line_cost_minor: *lc,
            })
            .collect();
    }

    Ok(out)
}

async fn issue_product(
    tx: &mut SqliteConnection,
    location_id: i64,
    product_id: i64,
    qty: i64,
    reference_id: i64,
    reason: &str,
    actor_id: i64,
) -> Result<Option<i64>, AppError> {
    let seq = next_move_seq(tx, location_id).await?;
    let move_number = format!("SAL-{seq:06}");
    let avg = withdraw_cost_layers(tx, product_id, qty).await?;
    sqlx::query(
        "INSERT INTO stock_movements
           (product_id, location_id, movement_type, quantity_delta, move_number,
            unit_cost_minor, reference_type, reference_id, reason, created_by)
         VALUES (?, ?, 'sale_issue', ?, ?, ?, 'sale', ?, ?, ?)",
    )
    .bind(product_id)
    .bind(location_id)
    .bind(-qty)
    .bind(&move_number)
    .bind(avg.unwrap_or(0))
    .bind(reference_id)
    .bind(reason)
    .bind(actor_id)
    .execute(&mut *tx)
    .await?;
    apply_on_hand_delta(&mut *tx, product_id, location_id, -qty).await?;
    Ok(avg)
}

async fn available_product(
    tx: &mut SqliteConnection,
    product_id: i64,
    location_id: i64,
) -> Result<i64, AppError> {
    let avail: Option<i64> = sqlx::query_scalar(
        "SELECT on_hand - reserved - damaged FROM stock_balances
         WHERE product_id = ? AND location_id = ?",
    )
    .bind(product_id)
    .bind(location_id)
    .fetch_optional(&mut *tx)
    .await?;
    Ok(avail.unwrap_or(0))
}

#[allow(clippy::too_many_arguments)]
async fn reverse_product(
    tx: &mut SqliteConnection,
    location_id: i64,
    product_id: i64,
    qty: i64,
    unit_cost: i64,
    reference_id: i64,
    reason: &str,
    actor_id: i64,
) -> Result<(), AppError> {
    let seq = next_move_seq(tx, location_id).await?;
    let move_number = format!("SAL-{seq:06}");
    let movement_id = sqlx::query(
        "INSERT INTO stock_movements
           (product_id, location_id, movement_type, quantity_delta, move_number,
            unit_cost_minor, reference_type, reference_id, reason, created_by)
         VALUES (?, ?, 'cancellation_reversal', ?, ?, ?, 'sale', ?, ?, ?)",
    )
    .bind(product_id)
    .bind(location_id)
    .bind(qty)
    .bind(&move_number)
    .bind(unit_cost)
    .bind(reference_id)
    .bind(reason)
    .bind(actor_id)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    apply_on_hand_delta(&mut *tx, product_id, location_id, qty).await?;
    if unit_cost > 0 {
        post_cost_layer(&mut *tx, product_id, qty, unit_cost, movement_id).await?;
    }
    Ok(())
}

pub async fn create_sale(
    state: &AppState,
    principal: &Principal,
    input: SaleCreateInput,
    correlation_id: &str,
) -> Result<SaleDto, AppError> {
    principal.require("sale.create")?;

    if input.items.is_empty() {
        return Err(AppError::Validation(
            "sale must have at least one item".into(),
        ));
    }
    for item in &input.items {
        if item.quantity <= 0 {
            return Err(AppError::Validation("quantities must be positive".into()));
        }
        if item.product_id.is_none() && item.bundle_id.is_none() {
            return Err(AppError::Validation(
                "each item must have a product_id or bundle_id".into(),
            ));
        }
        if item.product_id.is_some() && item.bundle_id.is_some() {
            return Err(AppError::Validation(
                "each item must have exactly one of product_id or bundle_id".into(),
            ));
        }
    }

    let kind = input.kind.unwrap_or_else(|| "sale".into());
    if kind != "sale" && kind != "quote" {
        return Err(AppError::Validation("kind must be sale or quote".into()));
    }
    let discount = input.discount_minor.unwrap_or(0);
    if discount < 0 {
        return Err(AppError::Validation("discount cannot be negative".into()));
    }
    let delivery = input.delivery_charge_minor.unwrap_or(0);
    if delivery < 0 {
        return Err(AppError::Validation(
            "delivery charge cannot be negative".into(),
        ));
    }
    let notes = input.notes.clone();
    let sale_date = input
        .sale_date
        .clone()
        .unwrap_or_else(|| state.clock.now_iso());

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let sale_id = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                require_active_location(&mut *tx, input.location_id).await?;

                if let Some(cid) = input.customer_id {
                    require_customer(&mut *tx, cid).await?;
                }

                let status = if kind == "quote" {
                    "quotation"
                } else {
                    "draft"
                };

                let customer_name = if let Some(cid) = input.customer_id {
                    let name: Option<String> =
                        sqlx::query_scalar("SELECT name FROM customers WHERE id = ?")
                            .bind(cid)
                            .fetch_optional(&mut *tx)
                            .await?;
                    name.unwrap_or_else(|| "Walk-in".into())
                } else {
                    "Walk-in".into()
                };

                let id = sqlx::query(
                    "INSERT INTO sales
                       (kind, customer_id, customer_name, location_id, sale_date,
                        status, discount_minor, delivery_charge_minor, notes, created_by)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&kind)
                .bind(input.customer_id)
                .bind(&customer_name)
                .bind(input.location_id)
                .bind(&sale_date)
                .bind(status)
                .bind(discount)
                .bind(delivery)
                .bind(notes.as_deref())
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                let mut subtotal = 0i64;

                for (index, item) in input.items.iter().enumerate() {
                    if let Some(product_id) = item.product_id {
                        crate::application::inventory::require_positive(&mut *tx, product_id)
                            .await?;
                        let row = sqlx::query(
                            "SELECT article_number, name, sale_price_minor FROM products WHERE id = ?",
                        )
                        .bind(product_id)
                        .fetch_one(&mut *tx)
                        .await?;
                        let article: String = row.get(0);
                        let name: String = row.get(1);
                        let price: i64 = row.get::<Option<i64>, _>(2).unwrap_or(0);
                        let total = price * item.quantity;
                        subtotal += total;
                        sqlx::query(
                            "INSERT INTO sale_items
                               (sale_id, sort_order, product_id, article_number, product_name,
                                quantity, unit_price_minor, line_total_minor, unit_cost_minor, line_cost_minor)
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, 0)",
                        )
                        .bind(id)
                        .bind(index as i64)
                        .bind(product_id)
                        .bind(&article)
                        .bind(&name)
                        .bind(item.quantity)
                        .bind(price)
                        .bind(total)
                        .execute(&mut *tx)
                        .await?;
                    } else if let Some(bundle_id) = item.bundle_id {
                        require_bundle(&mut *tx, bundle_id).await?;
                        let row = sqlx::query(
                            "SELECT name, code, default_price_minor FROM bundles WHERE id = ?",
                        )
                        .bind(bundle_id)
                        .fetch_one(&mut *tx)
                        .await?;
                        let name: String = row.get(0);
                        let code: String = row.get(1);
                        let price: i64 = row.get(2);
                        let total = price * item.quantity;
                        subtotal += total;
                        let cost_estimate = bundle_cost_estimate(&mut *tx, bundle_id).await?;
                        let item_id = sqlx::query(
                            "INSERT INTO sale_items
                               (sale_id, sort_order, bundle_id, article_number, product_name,
                                quantity, unit_price_minor, line_total_minor, unit_cost_minor, line_cost_minor)
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        )
                        .bind(id)
                        .bind(index as i64)
                        .bind(bundle_id)
                        .bind(&code)
                        .bind(&name)
                        .bind(item.quantity)
                        .bind(price)
                        .bind(total)
                        .bind(cost_estimate)
                        .bind(cost_estimate * item.quantity)
                        .execute(&mut *tx)
                        .await?
                        .last_insert_rowid();

                        let components = sqlx::query(
                            "SELECT bi.product_id, p.article_number, p.name, bi.quantity
                             FROM bundle_items bi
                             JOIN products p ON p.id = bi.product_id
                             WHERE bi.bundle_id = ? ORDER BY bi.sort_order, bi.id",
                        )
                        .bind(bundle_id)
                        .fetch_all(&mut *tx)
                        .await?;
                        for comp in &components {
                            let cpid = comp.get::<i64, _>(0);
                            let ca: String = comp.get(1);
                            let cn: String = comp.get(2);
                            let cqty = comp.get::<i64, _>(3);
                            sqlx::query(
                                "INSERT INTO sale_item_components
                                   (sale_item_id, product_id, article_number, product_name,
                                    quantity, unit_cost_minor, line_cost_minor)
                                 VALUES (?, ?, ?, ?, ?, 0, 0)",
                            )
                            .bind(item_id)
                            .bind(cpid)
                            .bind(&ca)
                            .bind(&cn)
                            .bind(cqty * item.quantity)
                            .execute(&mut *tx)
                            .await?;
                        }
                    }
                }

                let total = subtotal - discount + delivery;
                sqlx::query(
                    "UPDATE sales SET subtotal_minor = ?, total_minor = ?,
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(subtotal)
                .bind(total)
                .bind(id)
                .execute(&mut *tx)
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "sale.create",
                    "sale",
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

    sale_dto(state, sale_id).await
}

pub async fn confirm_sale(
    state: &AppState,
    principal: &Principal,
    input: SaleConfirmInput,
    correlation_id: &str,
) -> Result<SaleDto, AppError> {
    principal.require("sale.create")?;

    let paid = input.paid_minor.unwrap_or(0);
    let advance = input.advance_used_minor.unwrap_or(0);
    if paid < 0 {
        return Err(AppError::Validation("paid cannot be negative".into()));
    }
    if advance < 0 {
        return Err(AppError::Validation("advance cannot be negative".into()));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let has_credit = principal.permissions.iter().any(|p| p == "sale.credit");

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let row = sqlx::query(
                    "SELECT id, kind, customer_id, customer_name, location_id, sale_date,
                            status, discount_minor, delivery_charge_minor, total_minor,
                            paid_minor, advance_used_minor, due_minor, notes
                     FROM sales WHERE id = ?",
                )
                .bind(input.sale_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("sale {}", input.sale_id)))?;

                let replay = replay_guard(
                    &mut *tx,
                    "sales",
                    input.sale_id,
                    &input.idempotency_key,
                )
                .await?;
                if replay {
                    return Ok(());
                }

                let status: String = row.get(6);
                if status != "draft" && status != "quotation" {
                    return Err(AppError::Conflict(format!(
                        "sale {} cannot be confirmed in status '{status}'",
                        input.sale_id
                    )));
                }

                let location_id = row.get::<i64, _>(4);
                require_active_location(&mut *tx, location_id).await?;

                let customer_id: Option<i64> = row.try_get(2).ok().filter(|c| *c > 0);
                if let Some(cid) = customer_id {
                    require_customer(&mut *tx, cid).await?;

                    if advance > 0 {
                        let bal = customer_balance(&mut *tx, cid).await?;
                        let available = i64::max(0, -bal);
                        if advance > available {
                            return Err(AppError::Validation(format!(
                                "advance {advance} exceeds available {available}"
                            )));
                        }
                    }

                    if let Some(cn) = input.credit_note_id {
                        let cn_row = sqlx::query(
                            "SELECT customer_id, amount_minor, status FROM credit_notes WHERE id = ?",
                        )
                        .bind(cn)
                        .fetch_optional(&mut *tx)
                        .await?
                        .ok_or_else(|| AppError::NotFound(format!("credit note {cn}")))?;
                        let cn_customer: i64 = cn_row.get(0);
                        let cn_amount: i64 = cn_row.get(1);
                        let cn_status: String = cn_row.get(2);
                        if cn_customer != cid {
                            return Err(AppError::Validation(format!(
                                "credit note {cn} belongs to a different customer"
                            )));
                        }
                        if cn_status != "open" {
                            return Err(AppError::Conflict(format!(
                                "credit note {cn} is not open (status '{cn_status}')"
                            )));
                        }
                        if cn_amount != advance {
                            return Err(AppError::Validation(format!(
                                "credit note amount {cn_amount} does not match requested advance {advance}"
                            )));
                        }
                        sqlx::query(
                            "UPDATE credit_notes SET status = 'applied', sale_id = ?,
                             applied_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                             WHERE id = ?",
                        )
                        .bind(input.sale_id)
                        .bind(cn)
                        .execute(&mut *tx)
                        .await?;
                    }
                }

                if paid > 0 {
                    let account: Option<i64> = sqlx::query_scalar(
                        "SELECT 1 FROM cash_accounts WHERE id = ? AND is_active = 1",
                    )
                    .bind(input.cash_account_id.unwrap_or(0))
                    .fetch_optional(&mut *tx)
                    .await?;
                    if account.is_none() {
                        return Err(AppError::NotFound("cash account".into()));
                    }
                    let method: Option<i64> = sqlx::query_scalar(
                        "SELECT 1 FROM payment_methods WHERE id = ? AND is_active = 1",
                    )
                    .bind(input.payment_method_id.unwrap_or(0))
                    .fetch_optional(&mut *tx)
                    .await?;
                    if method.is_none() {
                        return Err(AppError::NotFound("payment method".into()));
                    }
                }

                if customer_id.is_none() && advance > 0 {
                    return Err(AppError::Validation(
                        "advance payments require a customer".into(),
                    ));
                }

                let discount = row.get::<i64, _>(7);

                let mut subtotal = 0i64;
                #[allow(clippy::type_complexity)]
                let items: Vec<(i64, Option<i64>, Option<i64>, i64, i64)> = sqlx::query(
                    "SELECT id, product_id, bundle_id, quantity, unit_price_minor
                     FROM sale_items WHERE sale_id = ? ORDER BY id",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3), r.get(4)))
                .collect();

                if items.is_empty() {
                    return Err(AppError::Validation("sale has no items".into()));
                }

                let comps: Vec<(i64, i64, i64)> = sqlx::query(
                    "SELECT c.sale_item_id, c.product_id, c.quantity
                     FROM sale_item_components c
                     JOIN sale_items i ON i.id = c.sale_item_id
                     WHERE i.sale_id = ?",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .collect();

                // Bundle-component stock check — only for products that track stock.
                for (_sid, pid, qty) in &comps {
                    let tracks: Option<bool> = sqlx::query_scalar(
                        "SELECT track_stock FROM products WHERE id = ?",
                    )
                    .bind(*pid)
                    .fetch_optional(&mut *tx)
                    .await?
                    .map(|v: i64| v != 0);
                    if tracks.unwrap_or(true) {
                        let avail = available_product(&mut *tx, *pid, location_id).await?;
                        if avail < *qty {
                            return Err(AppError::Validation(format!(
                                "insufficient component stock product {pid}: need {qty}, available {avail}"
                            )));
                        }
                    }
                }
                // Direct-item stock check — only for products that track stock.
                for (sale_item_id, pid, _bid, qty, _price) in &items {
                    if let Some(product_id) = pid {
                        let tracks: Option<bool> = sqlx::query_scalar(
                            "SELECT track_stock FROM products WHERE id = ?",
                        )
                        .bind(*product_id)
                        .fetch_optional(&mut *tx)
                        .await?
                        .map(|v: i64| v != 0);
                        if tracks.unwrap_or(true) {
                            let avail = available_product(&mut *tx, *product_id, location_id).await?;
                            if avail < *qty {
                                return Err(AppError::Validation(format!(
                                    "insufficient stock for sale item {sale_item_id}: need {qty}, available {avail}"
                                )));
                            }
                        }
                    }
                }

                let delivery = row.get::<i64, _>(8);
                let mut total_cost: i64 = 0;

                for (sale_item_id, pid, _bid, qty, unit_price) in &items {
                    let mut line_cost: i64 = 0;

                    if let Some(product_id) = pid {
                        let avg = issue_product(
                            &mut *tx,
                            location_id,
                            *product_id,
                            *qty,
                            input.sale_id,
                            &format!("sale {}", input.sale_id),
                            actor_id,
                        )
                        .await?;
                        let unit_cost = avg.unwrap_or(0);
                        line_cost = unit_cost * qty;

                        sqlx::query(
                            "UPDATE sale_items SET unit_cost_minor = ?, line_cost_minor = ?
                              WHERE id = ?",
                        )
                        .bind(unit_cost)
                        .bind(line_cost)
                        .bind(sale_item_id)
                        .execute(&mut *tx)
                        .await?;
                    } else {
                        let item_comps: Vec<(i64, i64)> = comps
                            .iter()
                            .filter(|(sid, _, _)| sid == sale_item_id)
                            .map(|(_, pid, qty)| (*pid, *qty))
                            .collect();
                        for (cpid, cqty) in item_comps {
                            let avg = issue_product(
                                &mut *tx,
                                location_id,
                                cpid,
                                cqty,
                                input.sale_id,
                                &format!("sale {}", input.sale_id),
                                actor_id,
                            )
                            .await?;
                            let unit_cost = avg.unwrap_or(0);
                            let comp_cost = unit_cost * cqty;
                            line_cost += comp_cost;

                            sqlx::query(
                                "UPDATE sale_item_components
                                    SET unit_cost_minor = ?, line_cost_minor = ?
                                  WHERE sale_item_id = ? AND product_id = ?",
                            )
                            .bind(unit_cost)
                            .bind(comp_cost)
                            .bind(sale_item_id)
                            .bind(cpid)
                            .execute(&mut *tx)
                            .await?;
                        }

                        let unit_cost_avg = if *qty > 0 {
                            line_cost / qty
                        } else {
                            0
                        };

                        sqlx::query(
                            "UPDATE sale_items SET unit_cost_minor = ?, line_cost_minor = ?
                              WHERE id = ?",
                        )
                        .bind(unit_cost_avg)
                        .bind(line_cost)
                        .bind(sale_item_id)
                        .execute(&mut *tx)
                        .await?;
                    }

                    total_cost += line_cost;
                    subtotal += unit_price * qty;
                }

                let total = subtotal - discount + delivery;

                if paid + advance > total {
                    return Err(AppError::Validation(
                        "paid + advance cannot exceed total".into(),
                    ));
                }
                if total - paid - advance > 0 && !has_credit {
                    return Err(AppError::Unauthorized(
                        "credit sales require sale.credit permission".into(),
                    ));
                }

                let sale_number = if let Some(ref num) = input.override_sale_number {
                    num.clone()
                } else {
                    next_document_number(&mut *tx, "sale").await?
                };

                let sale_date = row.get::<String, _>(5);
                let due_date = match customer_id {
                    Some(cid) => {
                        let credit_days: i64 = sqlx::query_scalar(
                            "SELECT credit_days FROM customers WHERE id = ?",
                        )
                        .bind(cid)
                        .fetch_one(&mut *tx)
                        .await?;
                        sqlx::query_scalar::<_, String>(
                            "SELECT date(?1, '+' || ?2 || ' days')",
                        )
                        .bind(&sale_date)
                        .bind(credit_days)
                        .fetch_one(&mut *tx)
                        .await?
                    }
                    None => sale_date.clone(),
                };

                sqlx::query(
                    "UPDATE sales
                        SET sale_number = ?, status = 'confirmed',
                            idempotency_key = ?,
                            subtotal_minor = ?, total_minor = ?,
                            paid_minor = ?, advance_used_minor = ?,
                            due_minor = ?, cost_minor = ?, due_date = ?,
                            payment_cash_account_id = ?,
                            confirmed_by = ?, confirmed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(&sale_number)
                .bind(&input.idempotency_key)
                .bind(subtotal)
                .bind(total)
                .bind(paid)
                .bind(advance)
                .bind(total - paid - advance)
                .bind(total_cost)
                .bind(&due_date)
                .bind(if paid > 0 { input.cash_account_id } else { None })
                .bind(actor_id)
                .bind(input.sale_id)
                .execute(&mut *tx)
                .await?;

                if let Some(cid) = customer_id {
                    record_ledger(
                        &mut *tx,
                        cid,
                        "sale",
                        "sale",
                        input.sale_id,
                        total,
                        &format!("sale {sale_number}"),
                        actor_id,
                    )
                    .await?;

                    if paid > 0 {
                        let payment_row = next_document_number(&mut *tx, "customer_payment")
                            .await
                            .unwrap_or_default();
                        let pay_id = sqlx::query(
                            "INSERT INTO customer_payments
                               (receipt_number, customer_id, sale_id, payment_method_id,
                                cash_account_id, payment_date, amount_minor, advance_alloc_minor,
                                status, idempotency_key, created_by)
                             VALUES (?, ?, ?, ?, ?, ?, ?, 0, 'posted', ?, ?)",
                        )
                        .bind(&payment_row)
                        .bind(cid)
                        .bind(input.sale_id)
                        .bind(input.payment_method_id.unwrap_or(0))
                        .bind(input.cash_account_id.unwrap_or(0))
                        .bind(row.get::<String, _>(5).clone())
                        .bind(paid)
                        .bind(input.idempotency_key.as_deref())
                        .bind(actor_id)
                        .execute(&mut *tx)
                        .await?
                        .last_insert_rowid();

                        sqlx::query(
                            "INSERT INTO customer_payment_allocations
                               (payment_id, sale_id, amount_minor)
                             VALUES (?, ?, ?)",
                        )
                        .bind(pay_id)
                        .bind(input.sale_id)
                        .bind(paid)
                        .execute(&mut *tx)
                        .await?;

                        record_ledger(
                            &mut *tx,
                            cid,
                            "payment",
                            "customer_payment",
                            pay_id,
                            -paid,
                            &format!("sale {sale_number}"),
                            actor_id,
                        )
                        .await?;
                    }

                    if advance > 0 {
                        // Reclassify the prepayment from the advance bucket to
                        // this receivable: the pair is balance-neutral because the
                        // receipt already reduced the balance when it arrived.
                        record_ledger(
                            &mut *tx,
                            cid,
                            "advance_used",
                            "sale",
                            input.sale_id,
                            advance,
                            &format!("sale {sale_number}"),
                            actor_id,
                        )
                        .await?;
                        record_ledger(
                            &mut *tx,
                            cid,
                            "payment",
                            "sale",
                            input.sale_id,
                            -advance,
                            &format!("advance applied to {sale_number}"),
                            actor_id,
                        )
                        .await?;
                    }
                }

                if paid > 0 {
                    record_cash_entry(
                        &mut *tx,
                        input.cash_account_id.unwrap_or(0),
                        "sale_payment",
                        paid,
                        "sale",
                        input.sale_id,
                        &format!("sale {sale_number}"),
                        actor_id,
                    )
                    .await?;
                }

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "sale.confirm",
                    "sale",
                    input.sale_id,
                    &correlation,
                    None,
                    None,
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    sale_dto(state, input.sale_id).await
}

pub async fn cancel_sale(
    state: &AppState,
    principal: &Principal,
    input: crate::dto::sales::SaleCancelInput,
    correlation_id: &str,
) -> Result<SaleDto, AppError> {
    principal.require("sale.cancel")?;

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = input
                .reason
                .clone()
                .unwrap_or_else(|| "cancellation".into());
            Box::pin(async move {
                let row = sqlx::query(
                    "SELECT id, customer_id, location_id, sale_date, status, total_minor,
                        paid_minor, advance_used_minor, sale_number, payment_cash_account_id
                 FROM sales WHERE id = ?",
                )
                .bind(input.sale_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("sale {}", input.sale_id)))?;

                let status: String = row.get(4);
                if status != "confirmed" {
                    return Err(AppError::Conflict(format!(
                        "sale {} cannot be cancelled in status '{status}'",
                        input.sale_id
                    )));
                }

                let location_id = row.get::<i64, _>(2);
                let total = row.get::<i64, _>(5);
                let paid_minor = row.get::<i64, _>(6);
                let advance = row.get::<i64, _>(7);
                let sale_number = row.get::<String, _>(8);
                let payment_cash_account_id = row.get::<Option<i64>, _>(9);

                let items: Vec<(Option<i64>, Option<i64>, i64, i64)> = sqlx::query(
                    "SELECT product_id, bundle_id, quantity, unit_cost_minor
                     FROM sale_items WHERE sale_id = ? ORDER BY id",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3)))
                .collect();

                let comps: Vec<(i64, i64, i64)> = sqlx::query(
                    "SELECT c.product_id, c.quantity, c.unit_cost_minor
                     FROM sale_item_components c
                     JOIN sale_items i ON i.id = c.sale_item_id
                     WHERE i.sale_id = ?",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .collect();

                // Reverse bundle component stock first, then standalone product stock.
                for (cpid, cqty, cunit) in &comps {
                    reverse_product(
                        &mut *tx,
                        location_id,
                        *cpid,
                        *cqty,
                        *cunit,
                        input.sale_id,
                        &reason,
                        actor_id,
                    )
                    .await?;
                }

                // Reverse standalone products; bundle component stock was already reversed above.
                for (pid, bid, qty, unit_cost) in &items {
                    if pid.is_some() && bid.is_none() {
                        reverse_product(
                            &mut *tx,
                            location_id,
                            pid.unwrap(),
                            *qty,
                            *unit_cost,
                            input.sale_id,
                            &reason,
                            actor_id,
                        )
                        .await?;
                    }
                }

                let customer_id: Option<i64> = row.try_get(1).ok().filter(|c| *c > 0);

                let payments: Vec<(i64, i64, i64)> = sqlx::query_as(
                    "SELECT p.id, p.cash_account_id,
                            COALESCE((SELECT a.amount_minor
                                      FROM customer_payment_allocations a
                                      WHERE a.payment_id = p.id AND a.sale_id = ?),
                                     p.amount_minor) AS refund_amount
                     FROM customer_payments p
                     WHERE p.status = 'posted'
                       AND (p.sale_id = ? OR EXISTS (
                             SELECT 1 FROM customer_payment_allocations a
                             WHERE a.payment_id = p.id AND a.sale_id = ?))
                     ORDER BY p.id",
                )
                .bind(input.sale_id)
                .bind(input.sale_id)
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?;

                for (pay_id, account_id, amount) in &payments {
                    crate::application::cash::require_cash_balance(
                        &mut *tx,
                        *account_id,
                        -amount,
                        "sale cancellation refund",
                    )
                    .await?;

                    record_cash_entry(
                        &mut *tx,
                        *account_id,
                        "sale_refund",
                        -amount,
                        "sale",
                        input.sale_id,
                        &format!("cancel sale {sale_number}"),
                        actor_id,
                    )
                    .await?;

                    sqlx::query(
                        "UPDATE customer_payments
                            SET status = 'voided',
                                voided_by = ?,
                                voided_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                          WHERE id = ?",
                    )
                    .bind(actor_id)
                    .bind(pay_id)
                    .execute(&mut *tx)
                    .await?;
                }

                // Walk-in sales (no customer) have no customer_payments row, so
                // refund the cash account the sale was paid from directly.
                if payments.is_empty() && customer_id.is_none() {
                    if let Some(account_id) = payment_cash_account_id {
                        if paid_minor > 0 {
                            crate::application::cash::require_cash_balance(
                                &mut *tx,
                                account_id,
                                -paid_minor,
                                "sale cancellation refund",
                            )
                            .await?;
                            record_cash_entry(
                                &mut *tx,
                                account_id,
                                "sale_refund",
                                -paid_minor,
                                "sale",
                                input.sale_id,
                                &format!("cancel sale {sale_number}"),
                                actor_id,
                            )
                            .await?;
                        }
                    }
                }

                if let Some(cid) = customer_id {
                    record_ledger(
                        &mut *tx,
                        cid,
                        "sale_cancellation",
                        "sale",
                        input.sale_id,
                        -total,
                        &format!("cancel sale {sale_number}"),
                        actor_id,
                    )
                    .await?;
                    if advance > 0 {
                        // Reverse the balance-neutral advance reclass recorded at
                        // confirmation: restore the advance bucket.
                        record_ledger(
                            &mut *tx,
                            cid,
                            "advance_restore",
                            "sale",
                            input.sale_id,
                            -advance,
                            &format!("cancel sale {sale_number}"),
                            actor_id,
                        )
                        .await?;
                        record_ledger(
                            &mut *tx,
                            cid,
                            "payment_refund",
                            "sale",
                            input.sale_id,
                            advance,
                            &format!("advance restored on {sale_number}"),
                            actor_id,
                        )
                        .await?;
                    }
                }

                sqlx::query(
                    "UPDATE sales
                        SET status = 'cancelled', paid_minor = 0, advance_used_minor = 0,
                            due_minor = 0, cost_minor = 0,
                            cancelled_by = ?,
                            cancelled_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(actor_id)
                .bind(input.sale_id)
                .execute(&mut *tx)
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "sale.cancel",
                    "sale",
                    input.sale_id,
                    &correlation,
                    None,
                    None,
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    sale_dto(state, input.sale_id).await
}

pub async fn sale_update(
    state: &AppState,
    principal: &Principal,
    input: crate::dto::sales::SaleUpdateInput,
) -> Result<SaleDto, AppError> {
    let actor_id = principal.require_any(&["sale.create"])?;
    let actor_session = principal.session_id.clone();
    
    // 1. Fetch current status
    let row = sqlx::query(
        "SELECT status, sale_number FROM sales WHERE id = ?"
    )
    .bind(input.sale_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("sale {}", input.sale_id)))?;
    
    let status: String = row.get(0);
    let old_sale_number: Option<String> = row.get(1);
    
    if status == "cancelled" {
        return Err(AppError::Conflict("Cannot edit a cancelled sale".into()));
    }
    
    let applied_cn: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM credit_notes WHERE sale_id = ?")
        .bind(input.sale_id)
        .fetch_one(&state.pool)
        .await?;
    if applied_cn > 0 {
        return Err(AppError::Conflict("Cannot edit a sale that has credit notes applied".into()));
    }

    // 2. If confirmed, reverse effects by calling sale_cancel internally
    if status == "confirmed" {
        cancel_sale(state, principal, SaleCancelInput {
            sale_id: input.sale_id,
            reason: Some("Sale updated".into()),
        }, "").await?;
        
        // Reset status to draft to allow confirm again
        sqlx::query(
            "UPDATE sales SET status = 'draft', cancelled_by = NULL, cancelled_at = NULL WHERE id = ?"
        )
        .bind(input.sale_id)
        .execute(&state.pool)
        .await?;
    }
    
    // 3. Clear old items
    sqlx::query("DELETE FROM sale_item_components WHERE sale_id = ?")
        .bind(input.sale_id).execute(&state.pool).await?;
    sqlx::query("DELETE FROM sale_items WHERE sale_id = ?")
        .bind(input.sale_id).execute(&state.pool).await?;
        
    // 4. Update sales core info
    let (customer_name, _): (Option<String>, Option<i64>) = if let Some(cid) = input.customer_id {
        let cr = sqlx::query("SELECT name, credit_limit_minor FROM customers WHERE id = ?")
            .bind(cid).fetch_optional(&state.pool).await?
            .ok_or_else(|| AppError::Validation(format!("customer {cid} not found")))?;
        (Some(cr.get(0)), Some(cr.get(1)))
    } else {
        (None, None)
    };
    
    sqlx::query(
        "UPDATE sales SET customer_id = ?, customer_name = ?, location_id = ?, 
            discount_minor = ?, delivery_charge_minor = ?, notes = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?"
    )
    .bind(input.customer_id)
    .bind(customer_name)
    .bind(input.location_id)
    .bind(input.discount_minor.unwrap_or(0))
    .bind(input.delivery_charge_minor.unwrap_or(0))
    .bind(&input.notes)
    .bind(input.sale_id)
    .execute(&state.pool)
    .await?;

    // 5. Insert new items
    let mut tx = state.pool.begin().await?;
    let mut sort_order = 0;
    let mut subtotal = 0;
    
    for item in &input.items {
        let (article_number, product_name, unit_price): (String, String, i64) = if let Some(pid) = item.product_id {
            sqlx::query("SELECT article_number, name, default_price_minor FROM products WHERE id = ?")
                .bind(pid).fetch_optional(&mut *tx).await?
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .ok_or_else(|| AppError::Validation(format!("product {pid} not found")))?
        } else {
            let bid = item.bundle_id.unwrap();
            sqlx::query("SELECT code, name, default_price_minor FROM bundles WHERE id = ?")
                .bind(bid).fetch_optional(&mut *tx).await?
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .ok_or_else(|| AppError::Validation(format!("bundle {bid} not found")))?
        };
        
        let line_total = unit_price * item.quantity;
        subtotal += line_total;
        
        let sale_item_id = sqlx::query(
            "INSERT INTO sale_items (sale_id, product_id, bundle_id, article_number, product_name, quantity, unit_price_minor, line_total_minor, sort_order)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(input.sale_id)
        .bind(item.product_id)
        .bind(item.bundle_id)
        .bind(&article_number)
        .bind(&product_name)
        .bind(item.quantity)
        .bind(unit_price)
        .bind(line_total)
        .bind(sort_order)
        .execute(&mut *tx).await?.last_insert_rowid();
        
        if let Some(bid) = item.bundle_id {
            let components: Vec<(i64, String, String, i64)> = sqlx::query(
                "SELECT p.id, p.article_number, p.name, bi.quantity
                 FROM bundle_items bi JOIN products p ON bi.product_id = p.id
                 WHERE bi.bundle_id = ?"
            ).bind(bid).fetch_all(&mut *tx).await?
            .into_iter().map(|r| (r.get(0), r.get(1), r.get(2), r.get(3))).collect();
            
            for (cid, cart, cname, cqty) in components {
                sqlx::query(
                    "INSERT INTO sale_item_components (sale_id, sale_item_id, product_id, article_number, product_name, quantity)
                     VALUES (?, ?, ?, ?, ?, ?)"
                )
                .bind(input.sale_id).bind(sale_item_id).bind(cid)
                .bind(cart).bind(cname).bind(cqty * item.quantity)
                .execute(&mut *tx).await?;
            }
        }
        sort_order += 1;
    }
    
    let total = subtotal - input.discount_minor.unwrap_or(0) + input.delivery_charge_minor.unwrap_or(0);
    sqlx::query("UPDATE sales SET subtotal_minor = ?, total_minor = ? WHERE id = ?")
        .bind(subtotal).bind(total).bind(input.sale_id).execute(&mut *tx).await?;
        
    tx.commit().await?;
    
    // 6. If originally confirmed, re-confirm
    if status == "confirmed" {
        confirm_sale(state, principal, SaleConfirmInput {
            sale_id: input.sale_id,
            override_sale_number: old_sale_number,
            idempotency_key: None,
            paid_minor: input.paid_minor,
            cash_account_id: input.cash_account_id,
            payment_method_id: input.payment_method_id,
            advance_used_minor: Some(0),
            credit_note_id: None,
        }, "").await?;
    }
    
    sale_dto(state, input.sale_id).await
}
pub async fn list_sales(state: &AppState, principal: &Principal) -> Result<Vec<SaleDto>, AppError> {
    principal.require_any(&["sale.create", "invoice.print"])?;
    let rows: Vec<i64> = sqlx::query_scalar("SELECT id FROM sales ORDER BY id DESC LIMIT 500")
        .fetch_all(&state.pool)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for id in rows {
        out.push(sale_dto(state, id).await?);
    }
    Ok(out)
}

pub async fn get_sale(
    state: &AppState,
    principal: &Principal,
    sale_id: i64,
) -> Result<SaleDto, AppError> {
    principal.require_any(&["sale.create", "invoice.print"])?;
    sale_dto(state, sale_id).await
}

pub async fn invoice_pdf(
    state: &AppState,
    principal: &Principal,
    sale_id: i64,
) -> Result<PdfResultDto, AppError> {
    principal.require("invoice.print")?;

    let row = sqlx::query(
        "SELECT id, sale_number, sale_date, customer_name, kind, status,
                subtotal_minor, discount_minor, delivery_charge_minor, tax_minor,
                total_minor, paid_minor, advance_used_minor, due_minor, customer_id
         FROM sales WHERE id = ?",
    )
    .bind(sale_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("sale {sale_id}")))?;

    let status: String = row.get(5);
    if status != "confirmed" {
        return Err(AppError::Validation(
            "only confirmed sales can generate invoices".into(),
        ));
    }

    let customer_id: Option<i64> = row.try_get(14).ok().flatten();

    let items: Vec<(String, String, i64, i64, i64)> = sqlx::query(
        "SELECT article_number, product_name, quantity, unit_price_minor, line_total_minor
         FROM sale_items WHERE sale_id = ? ORDER BY sort_order, id",
    )
    .bind(sale_id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3), r.get(4)))
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

    let sale_number = row.get::<Option<String>, _>(1).unwrap_or_default();

    let (customer_phone, customer_address) = if let Some(cid) = customer_id {
        let phone: Option<String> = sqlx::query_scalar("SELECT phone FROM customers WHERE id = ?")
            .bind(cid)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();
        let addr: Option<String> = sqlx::query_scalar("SELECT address FROM customers WHERE id = ?")
            .bind(cid)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();
        (phone, addr)
    } else {
        (None, None)
    };

    let record = InvoiceRecord {
        number: sale_number.clone(),
        sale_date: row.get(2),
        customer_name: row.try_get(3).ok(),
        customer_phone,
        customer_address,
        notes: None,
        footer_text: None,
        shop_name,
        shop_address,
        items: items
            .into_iter()
            .map(|(article, name, qty, price, total)| InvoiceLine {
                article,
                name,
                quantity: qty,
                price_minor: price,
                total_minor: total,
            })
            .collect(),
        subtotal_minor: row.get(6),
        discount_minor: row.get(7),
        delivery_charge_minor: row.get(8),
        tax_minor: row.get(9),
        total_minor: row.get(10),
        paid_minor: row.get(11),
        advance_used_minor: row.get(12),
        due_minor: row.get(13),
    };

    let reports_dir = state.paths.reports_dir.clone();
    let pdf = tokio::task::spawn_blocking(move || generate_invoice_pdf(&reports_dir, &record))
        .await
        .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    Ok(PdfResultDto {
        report_path: pdf.path,
        pages: pdf.pages,
        bytes: pdf.bytes,
    })
}

pub async fn delete_sale(
    state: &AppState,
    principal: &Principal,
    input: SaleDeleteInput,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("sale.create")?;

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = input
                .reason
                .clone()
                .unwrap_or_else(|| "deletion".into());
            Box::pin(async move {
                let row = sqlx::query(
                    "SELECT id, customer_id, location_id, sale_date, status, total_minor,
                        paid_minor, advance_used_minor, sale_number, payment_cash_account_id
                 FROM sales WHERE id = ?",
                )
                .bind(input.sale_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("sale {}", input.sale_id)))?;

                let status: String = row.get(4);
                if status != "confirmed" {
                    return Err(AppError::Conflict(format!(
                        "sale {} cannot be deleted in status '{status}'",
                        input.sale_id
                    )));
                }

                let location_id = row.get::<i64, _>(2);
                let total = row.get::<i64, _>(5);
                let paid_minor = row.get::<i64, _>(6);
                let advance = row.get::<i64, _>(7);
                let sale_number = row.get::<String, _>(8);
                let payment_cash_account_id = row.get::<Option<i64>, _>(9);

                let items: Vec<(Option<i64>, Option<i64>, i64, i64)> = sqlx::query(
                    "SELECT product_id, bundle_id, quantity, unit_cost_minor
                     FROM sale_items WHERE sale_id = ? ORDER BY id",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3)))
                .collect();

                let comps: Vec<(i64, i64, i64)> = sqlx::query(
                    "SELECT c.product_id, c.quantity, c.unit_cost_minor
                     FROM sale_item_components c
                     JOIN sale_items i ON i.id = c.sale_item_id
                     WHERE i.sale_id = ?",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .collect();

                for (cpid, cqty, cunit) in &comps {
                    reverse_product(
                        &mut *tx,
                        location_id,
                        *cpid,
                        *cqty,
                        *cunit,
                        input.sale_id,
                        &reason,
                        actor_id,
                    )
                    .await?;
                }

                for (pid, bid, qty, unit_cost) in &items {
                    if pid.is_some() && bid.is_none() {
                        reverse_product(
                            &mut *tx,
                            location_id,
                            pid.unwrap(),
                            *qty,
                            *unit_cost,
                            input.sale_id,
                            &reason,
                            actor_id,
                        )
                        .await?;
                    }
                }

                let customer_id: Option<i64> = row.try_get(1).ok().filter(|c| *c > 0);

                let payments: Vec<(i64, i64, i64)> = sqlx::query_as(
                    "SELECT p.id, p.cash_account_id,
                            COALESCE((SELECT a.amount_minor
                                      FROM customer_payment_allocations a
                                      WHERE a.payment_id = p.id AND a.sale_id = ?),
                                     p.amount_minor) AS refund_amount
                     FROM customer_payments p
                     WHERE p.status = 'posted'
                       AND (p.sale_id = ? OR EXISTS (
                             SELECT 1 FROM customer_payment_allocations a
                             WHERE a.payment_id = p.id AND a.sale_id = ?))
                     ORDER BY p.id",
                )
                .bind(input.sale_id)
                .bind(input.sale_id)
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?;

                for (pay_id, account_id, amount) in &payments {
                    require_cash_balance(
                        &mut *tx,
                        *account_id,
                        -amount,
                        "sale deletion refund",
                    )
                    .await?;

                    record_cash_entry(
                        &mut *tx,
                        *account_id,
                        "sale_refund",
                        -amount,
                        "sale",
                        input.sale_id,
                        &format!("delete sale {sale_number}"),
                        actor_id,
                    )
                    .await?;

                    sqlx::query(
                        "UPDATE customer_payments
                            SET status = 'voided',
                                voided_by = ?,
                                voided_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                          WHERE id = ?",
                    )
                    .bind(actor_id)
                    .bind(pay_id)
                    .execute(&mut *tx)
                    .await?;
                }

                if payments.is_empty() && customer_id.is_none() {
                    if let Some(account_id) = payment_cash_account_id {
                        if paid_minor > 0 {
                            require_cash_balance(
                                &mut *tx,
                                account_id,
                                -paid_minor,
                                "sale deletion refund",
                            )
                            .await?;
                            record_cash_entry(
                                &mut *tx,
                                account_id,
                                "sale_refund",
                                -paid_minor,
                                "sale",
                                input.sale_id,
                                &format!("delete sale {sale_number}"),
                                actor_id,
                            )
                            .await?;
                        }
                    }
                }

                if let Some(cid) = customer_id {
                    record_ledger(
                        &mut *tx,
                        cid,
                        "sale_cancellation",
                        "sale",
                        input.sale_id,
                        -total,
                        &format!("delete sale {sale_number}"),
                        actor_id,
                    )
                    .await?;
                    if advance > 0 {
                        record_ledger(
                            &mut *tx,
                            cid,
                            "advance_restore",
                            "sale",
                            input.sale_id,
                            -advance,
                            &format!("delete sale {sale_number}"),
                            actor_id,
                        )
                        .await?;
                        record_ledger(
                            &mut *tx,
                            cid,
                            "payment_refund",
                            "sale",
                            input.sale_id,
                            advance,
                            &format!("advance restored on {sale_number}"),
                            actor_id,
                        )
                        .await?;
                    }
                }

                sqlx::query(
                    "DELETE FROM customer_payment_allocations WHERE sale_id = ?",
                )
                .bind(input.sale_id)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "DELETE FROM customer_payments WHERE sale_id = ? AND status = 'voided'",
                )
                .bind(input.sale_id)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "DELETE FROM sale_item_components
                     WHERE sale_item_id IN (SELECT id FROM sale_items WHERE sale_id = ?)",
                )
                .bind(input.sale_id)
                .execute(&mut *tx)
                .await?;

                sqlx::query("DELETE FROM sale_items WHERE sale_id = ?")
                    .bind(input.sale_id)
                    .execute(&mut *tx)
                    .await?;

                sqlx::query("DELETE FROM sales WHERE id = ?")
                    .bind(input.sale_id)
                    .execute(&mut *tx)
                    .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "sale.delete",
                    "sale",
                    input.sale_id,
                    &correlation,
                    None,
                    None,
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    Ok(())
}

pub async fn draft_delete(
    state: &AppState,
    principal: &Principal,
    input: SaleDeleteInput,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("sale.create")?;

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let reason = input
                .reason
                .clone()
                .unwrap_or_else(|| "draft deletion".into());
            Box::pin(async move {
                let row = sqlx::query(
                    "SELECT id, status, sale_number FROM sales WHERE id = ?",
                )
                .bind(input.sale_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("sale {}", input.sale_id)))?;

                let status: String = row.get(1);
                if status != "draft" {
                    return Err(AppError::Conflict(format!(
                        "sale {} cannot be deleted as draft in status '{status}'",
                        input.sale_id
                    )));
                }

                let sale_number: Option<String> = row.get(2);

                sqlx::query("DELETE FROM sale_item_components WHERE sale_item_id IN (SELECT id FROM sale_items WHERE sale_id = ?)")
                    .bind(input.sale_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM sale_items WHERE sale_id = ?")
                    .bind(input.sale_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM sales WHERE id = ?")
                    .bind(input.sale_id)
                    .execute(&mut *tx)
                    .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "sale.draft_delete",
                    "sale",
                    input.sale_id,
                    &correlation,
                    Some(serde_json::json!({"sale_number": sale_number, "reason": reason})),
                    None,
                ).await;

                Ok(())
            })
        })
        .await
}

pub async fn edit_sale(
    state: &AppState,
    principal: &Principal,
    input: SaleEditInput,
    correlation_id: &str,
) -> Result<SaleDto, AppError> {
    principal.require("sale.create")?;

    if input.items.is_empty() {
        return Err(AppError::Validation(
            "sale must have at least one item".into(),
        ));
    }
    for item in &input.items {
        if item.quantity <= 0 {
            return Err(AppError::Validation("quantities must be positive".into()));
        }
        if item.product_id.is_none() && item.bundle_id.is_none() {
            return Err(AppError::Validation(
                "each item must have a product_id or bundle_id".into(),
            ));
        }
        if item.product_id.is_some() && item.bundle_id.is_some() {
            return Err(AppError::Validation(
                "each item must have exactly one of product_id or bundle_id".into(),
            ));
        }
    }

    let discount = input.discount_minor.unwrap_or(0);
    if discount < 0 {
        return Err(AppError::Validation("discount cannot be negative".into()));
    }
    let delivery = input.delivery_charge_minor.unwrap_or(0);
    if delivery < 0 {
        return Err(AppError::Validation(
            "delivery charge cannot be negative".into(),
        ));
    }
    let paid = input.paid_minor.unwrap_or(0);
    if paid < 0 {
        return Err(AppError::Validation("paid cannot be negative".into()));
    }

    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let has_credit = principal.permissions.iter().any(|p| p == "sale.credit");
    let now = state.clock.now_iso();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let row = sqlx::query(
                    "SELECT id, customer_id, location_id, sale_date, status, total_minor,
                        paid_minor, advance_used_minor, sale_number, payment_cash_account_id
                 FROM sales WHERE id = ?",
                )
                .bind(input.sale_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("sale {}", input.sale_id)))?;

                let status: String = row.get(4);
                if status != "confirmed" {
                    return Err(AppError::Conflict(format!(
                        "sale {} cannot be edited in status '{status}'",
                        input.sale_id
                    )));
                }

                let location_id = row.get::<i64, _>(2);
                let total = row.get::<i64, _>(5);
                let paid_minor = row.get::<i64, _>(6);
                let advance = row.get::<i64, _>(7);
                let sale_number = row.get::<String, _>(8);
                let payment_cash_account_id = row.get::<Option<i64>, _>(9);
                let existing_customer_id: Option<i64> = row.try_get(1).ok().filter(|c| *c > 0);

                require_active_location(&mut *tx, location_id).await?;

                if let Some(cid) = input.customer_id {
                    require_customer(&mut *tx, cid).await?;
                }

                // ── Reverse existing effects ──────────────────────────────

                let items: Vec<(Option<i64>, Option<i64>, i64, i64)> = sqlx::query(
                    "SELECT product_id, bundle_id, quantity, unit_cost_minor
                     FROM sale_items WHERE sale_id = ? ORDER BY id",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3)))
                .collect();

                let comps: Vec<(i64, i64, i64)> = sqlx::query(
                    "SELECT c.product_id, c.quantity, c.unit_cost_minor
                     FROM sale_item_components c
                     JOIN sale_items i ON i.id = c.sale_item_id
                     WHERE i.sale_id = ?",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .collect();

                for (cpid, cqty, cunit) in &comps {
                    reverse_product(
                        &mut *tx,
                        location_id,
                        *cpid,
                        *cqty,
                        *cunit,
                        input.sale_id,
                        "sale edit reversal",
                        actor_id,
                    )
                    .await?;
                }

                for (pid, bid, qty, unit_cost) in &items {
                    if pid.is_some() && bid.is_none() {
                        reverse_product(
                            &mut *tx,
                            location_id,
                            pid.unwrap(),
                            *qty,
                            *unit_cost,
                            input.sale_id,
                            "sale edit reversal",
                            actor_id,
                        )
                        .await?;
                    }
                }

                let payments: Vec<(i64, i64, i64)> = sqlx::query_as(
                    "SELECT p.id, p.cash_account_id,
                            COALESCE((SELECT a.amount_minor
                                      FROM customer_payment_allocations a
                                      WHERE a.payment_id = p.id AND a.sale_id = ?),
                                     p.amount_minor) AS refund_amount
                     FROM customer_payments p
                     WHERE p.status = 'posted'
                       AND (p.sale_id = ? OR EXISTS (
                             SELECT 1 FROM customer_payment_allocations a
                             WHERE a.payment_id = p.id AND a.sale_id = ?))
                     ORDER BY p.id",
                )
                .bind(input.sale_id)
                .bind(input.sale_id)
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?;

                for (pay_id, account_id, amount) in &payments {
                    require_cash_balance(
                        &mut *tx,
                        *account_id,
                        -amount,
                        "sale edit refund",
                    )
                    .await?;

                    record_cash_entry(
                        &mut *tx,
                        *account_id,
                        "sale_refund",
                        -amount,
                        "sale",
                        input.sale_id,
                        &format!("edit sale {sale_number}"),
                        actor_id,
                    )
                    .await?;

                    sqlx::query(
                        "UPDATE customer_payments
                            SET status = 'voided',
                                voided_by = ?,
                                voided_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                          WHERE id = ?",
                    )
                    .bind(actor_id)
                    .bind(pay_id)
                    .execute(&mut *tx)
                    .await?;
                }

                if payments.is_empty() && existing_customer_id.is_none() {
                    if let Some(account_id) = payment_cash_account_id {
                        if paid_minor > 0 {
                            require_cash_balance(
                                &mut *tx,
                                account_id,
                                -paid_minor,
                                "sale edit refund",
                            )
                            .await?;
                            record_cash_entry(
                                &mut *tx,
                                account_id,
                                "sale_refund",
                                -paid_minor,
                                "sale",
                                input.sale_id,
                                &format!("edit sale {sale_number}"),
                                actor_id,
                            )
                            .await?;
                        }
                    }
                }

                if let Some(cid) = existing_customer_id {
                    record_ledger(
                        &mut *tx,
                        cid,
                        "sale_cancellation",
                        "sale",
                        input.sale_id,
                        -total,
                        &format!("edit sale {sale_number}"),
                        actor_id,
                    )
                    .await?;
                    if advance > 0 {
                        record_ledger(
                            &mut *tx,
                            cid,
                            "advance_restore",
                            "sale",
                            input.sale_id,
                            -advance,
                            &format!("edit sale {sale_number}"),
                            actor_id,
                        )
                        .await?;
                        record_ledger(
                            &mut *tx,
                            cid,
                            "payment_refund",
                            "sale",
                            input.sale_id,
                            advance,
                            &format!("advance restored on {sale_number}"),
                            actor_id,
                        )
                        .await?;
                    }
                }

                // ── Update sale header ───────────────────────────────────

                let customer_name = if let Some(cid) = input.customer_id {
                    let name: Option<String> =
                        sqlx::query_scalar("SELECT name FROM customers WHERE id = ?")
                            .bind(cid)
                            .fetch_optional(&mut *tx)
                            .await?;
                    name.unwrap_or_else(|| "Walk-in".into())
                } else {
                    "Walk-in".into()
                };

                sqlx::query(
                    "UPDATE sales
                        SET customer_id = ?, customer_name = ?,
                            discount_minor = ?, delivery_charge_minor = ?, notes = ?,
                            subtotal_minor = 0, total_minor = 0, paid_minor = 0,
                            advance_used_minor = 0, due_minor = 0, cost_minor = 0,
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(input.customer_id)
                .bind(&customer_name)
                .bind(discount)
                .bind(delivery)
                .bind(&input.notes)
                .bind(input.sale_id)
                .execute(&mut *tx)
                .await?;

                // ── Delete old items ─────────────────────────────────────

                sqlx::query(
                    "DELETE FROM sale_item_components
                     WHERE sale_item_id IN (SELECT id FROM sale_items WHERE sale_id = ?)",
                )
                .bind(input.sale_id)
                .execute(&mut *tx)
                .await?;

                sqlx::query("DELETE FROM sale_items WHERE sale_id = ?")
                    .bind(input.sale_id)
                    .execute(&mut *tx)
                    .await?;

                // ── Re-insert sale items ─────────────────────────────────

                let mut subtotal = 0i64;

                for (index, item) in input.items.iter().enumerate() {
                    if let Some(product_id) = item.product_id {
                        crate::application::inventory::require_positive(&mut *tx, product_id)
                            .await?;
                        let row = sqlx::query(
                            "SELECT article_number, name, sale_price_minor FROM products WHERE id = ?",
                        )
                        .bind(product_id)
                        .fetch_one(&mut *tx)
                        .await?;
                        let article: String = row.get(0);
                        let name: String = row.get(1);
                        let price: i64 = row.get::<Option<i64>, _>(2).unwrap_or(0);
                        let total_line = price * item.quantity;
                        subtotal += total_line;
                        sqlx::query(
                            "INSERT INTO sale_items
                               (sale_id, sort_order, product_id, article_number, product_name,
                                quantity, unit_price_minor, line_total_minor, unit_cost_minor, line_cost_minor)
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, 0)",
                        )
                        .bind(input.sale_id)
                        .bind(index as i64)
                        .bind(product_id)
                        .bind(&article)
                        .bind(&name)
                        .bind(item.quantity)
                        .bind(price)
                        .bind(total_line)
                        .execute(&mut *tx)
                        .await?;
                    } else if let Some(bundle_id) = item.bundle_id {
                        require_bundle(&mut *tx, bundle_id).await?;
                        let row = sqlx::query(
                            "SELECT name, code, default_price_minor FROM bundles WHERE id = ?",
                        )
                        .bind(bundle_id)
                        .fetch_one(&mut *tx)
                        .await?;
                        let name: String = row.get(0);
                        let code: String = row.get(1);
                        let price: i64 = row.get(2);
                        let total_line = price * item.quantity;
                        subtotal += total_line;
                        let cost_estimate = bundle_cost_estimate(&mut *tx, bundle_id).await?;
                        let item_id = sqlx::query(
                            "INSERT INTO sale_items
                               (sale_id, sort_order, bundle_id, article_number, product_name,
                                quantity, unit_price_minor, line_total_minor, unit_cost_minor, line_cost_minor)
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        )
                        .bind(input.sale_id)
                        .bind(index as i64)
                        .bind(bundle_id)
                        .bind(&code)
                        .bind(&name)
                        .bind(item.quantity)
                        .bind(price)
                        .bind(total_line)
                        .bind(cost_estimate)
                        .bind(cost_estimate * item.quantity)
                        .execute(&mut *tx)
                        .await?
                        .last_insert_rowid();

                        let components = sqlx::query(
                            "SELECT bi.product_id, p.article_number, p.name, bi.quantity
                             FROM bundle_items bi
                             JOIN products p ON p.id = bi.product_id
                             WHERE bi.bundle_id = ? ORDER BY bi.sort_order, bi.id",
                        )
                        .bind(bundle_id)
                        .fetch_all(&mut *tx)
                        .await?;
                        for comp in &components {
                            let cpid = comp.get::<i64, _>(0);
                            let ca: String = comp.get(1);
                            let cn: String = comp.get(2);
                            let cqty = comp.get::<i64, _>(3);
                            sqlx::query(
                                "INSERT INTO sale_item_components
                                   (sale_item_id, product_id, article_number, product_name,
                                    quantity, unit_cost_minor, line_cost_minor)
                                 VALUES (?, ?, ?, ?, ?, 0, 0)",
                            )
                            .bind(item_id)
                            .bind(cpid)
                            .bind(&ca)
                            .bind(&cn)
                            .bind(cqty * item.quantity)
                            .execute(&mut *tx)
                            .await?;
                        }
                    }
                }

                // ── Re-check stock availability ──────────────────────────

                #[allow(clippy::type_complexity)]
                let new_items: Vec<(i64, Option<i64>, Option<i64>, i64, i64)> = sqlx::query(
                    "SELECT id, product_id, bundle_id, quantity, unit_price_minor
                     FROM sale_items WHERE sale_id = ? ORDER BY id",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3), r.get(4)))
                .collect();

                let new_comps: Vec<(i64, i64, i64)> = sqlx::query(
                    "SELECT c.sale_item_id, c.product_id, c.quantity
                     FROM sale_item_components c
                     JOIN sale_items i ON i.id = c.sale_item_id
                     WHERE i.sale_id = ?",
                )
                .bind(input.sale_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .collect();

                for (_sid, pid, qty) in &new_comps {
                    let tracks: Option<bool> = sqlx::query_scalar(
                        "SELECT track_stock FROM products WHERE id = ?",
                    )
                    .bind(*pid)
                    .fetch_optional(&mut *tx)
                    .await?
                    .map(|v: i64| v != 0);
                    if tracks.unwrap_or(true) {
                        let avail = available_product(&mut *tx, *pid, location_id).await?;
                        if avail < *qty {
                            return Err(AppError::Validation(format!(
                                "insufficient component stock product {pid}: need {qty}, available {avail}"
                            )));
                        }
                    }
                }

                for (sale_item_id, pid, _bid, qty, _price) in &new_items {
                    if let Some(product_id) = pid {
                        let tracks: Option<bool> = sqlx::query_scalar(
                            "SELECT track_stock FROM products WHERE id = ?",
                        )
                        .bind(*product_id)
                        .fetch_optional(&mut *tx)
                        .await?
                        .map(|v: i64| v != 0);
                        if tracks.unwrap_or(true) {
                            let avail =
                                available_product(&mut *tx, *product_id, location_id).await?;
                            if avail < *qty {
                                return Err(AppError::Validation(format!(
                                    "insufficient stock for sale item {sale_item_id}: need {qty}, available {avail}"
                                )));
                            }
                        }
                    }
                }

                // ── Re-withdraw stock ────────────────────────────────────

                let mut total_cost: i64 = 0;

                for (sale_item_id, pid, _bid, qty, unit_price) in &new_items {
                    let mut line_cost: i64 = 0;

                    if let Some(product_id) = pid {
                        let avg = issue_product(
                            &mut *tx,
                            location_id,
                            *product_id,
                            *qty,
                            input.sale_id,
                            &format!("sale {}", input.sale_id),
                            actor_id,
                        )
                        .await?;
                        let unit_cost = avg.unwrap_or(0);
                        line_cost = unit_cost * qty;

                        sqlx::query(
                            "UPDATE sale_items SET unit_cost_minor = ?, line_cost_minor = ?
                              WHERE id = ?",
                        )
                        .bind(unit_cost)
                        .bind(line_cost)
                        .bind(sale_item_id)
                        .execute(&mut *tx)
                        .await?;
                    } else {
                        let item_comps: Vec<(i64, i64)> = new_comps
                            .iter()
                            .filter(|(sid, _, _)| sid == sale_item_id)
                            .map(|(_, pid, qty)| (*pid, *qty))
                            .collect();
                        for (cpid, cqty) in item_comps {
                            let avg = issue_product(
                                &mut *tx,
                                location_id,
                                cpid,
                                cqty,
                                input.sale_id,
                                &format!("sale {}", input.sale_id),
                                actor_id,
                            )
                            .await?;
                            let unit_cost = avg.unwrap_or(0);
                            let comp_cost = unit_cost * cqty;
                            line_cost += comp_cost;

                            sqlx::query(
                                "UPDATE sale_item_components
                                    SET unit_cost_minor = ?, line_cost_minor = ?
                                  WHERE sale_item_id = ? AND product_id = ?",
                            )
                            .bind(unit_cost)
                            .bind(comp_cost)
                            .bind(sale_item_id)
                            .bind(cpid)
                            .execute(&mut *tx)
                            .await?;
                        }

                        let unit_cost_avg = if *qty > 0 {
                            line_cost / qty
                        } else {
                            0
                        };

                        sqlx::query(
                            "UPDATE sale_items SET unit_cost_minor = ?, line_cost_minor = ?
                              WHERE id = ?",
                        )
                        .bind(unit_cost_avg)
                        .bind(line_cost)
                        .bind(sale_item_id)
                        .execute(&mut *tx)
                        .await?;
                    }

                    total_cost += line_cost;
                    subtotal += unit_price * qty;
                }

                let total = subtotal - discount + delivery;

                if paid > total {
                    return Err(AppError::Validation(
                        "paid cannot exceed total".into(),
                    ));
                }
                if total - paid > 0 && !has_credit {
                    return Err(AppError::Unauthorized(
                        "credit sales require sale.credit permission".into(),
                    ));
                }

                let sale_number_clone = sale_number.clone();

                // ── Record new payments ──────────────────────────────────

                if let Some(cid) = input.customer_id {
                    record_ledger(
                        &mut *tx,
                        cid,
                        "sale",
                        "sale",
                        input.sale_id,
                        total,
                        &format!("sale {sale_number_clone}"),
                        actor_id,
                    )
                    .await?;

                    if paid > 0 {
                        let payment_row = next_document_number(&mut *tx, "customer_payment")
                            .await
                            .unwrap_or_default();
                        let pay_id = sqlx::query(
                            "INSERT INTO customer_payments
                               (receipt_number, customer_id, sale_id, payment_method_id,
                                cash_account_id, payment_date, amount_minor, advance_alloc_minor,
                                status, idempotency_key, created_by)
                             VALUES (?, ?, ?, ?, ?, ?, ?, 0, 'posted', NULL, ?)",
                        )
                        .bind(&payment_row)
                        .bind(cid)
                        .bind(input.sale_id)
                        .bind(input.payment_method_id.unwrap_or(0))
                        .bind(payment_cash_account_id.unwrap_or(0))
                        .bind(now.clone())
                        .bind(paid)
                        .bind(actor_id)
                        .execute(&mut *tx)
                        .await?
                        .last_insert_rowid();

                        sqlx::query(
                            "INSERT INTO customer_payment_allocations
                               (payment_id, sale_id, amount_minor)
                             VALUES (?, ?, ?)",
                        )
                        .bind(pay_id)
                        .bind(input.sale_id)
                        .bind(paid)
                        .execute(&mut *tx)
                        .await?;

                        record_ledger(
                            &mut *tx,
                            cid,
                            "payment",
                            "customer_payment",
                            pay_id,
                            -paid,
                            &format!("sale {sale_number_clone}"),
                            actor_id,
                        )
                        .await?;
                    }
                }

                if paid > 0 {
                    if let Some(account_id) = payment_cash_account_id {
                        record_cash_entry(
                            &mut *tx,
                            account_id,
                            "sale_payment",
                            paid,
                            "sale",
                            input.sale_id,
                            &format!("sale {sale_number_clone}"),
                            actor_id,
                        )
                        .await?;
                    }
                }

                // ── Finalize sale ────────────────────────────────────────

                sqlx::query(
                    "UPDATE sales
                        SET subtotal_minor = ?, total_minor = ?,
                            paid_minor = ?, due_minor = ?, cost_minor = ?,
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(subtotal)
                .bind(total)
                .bind(paid)
                .bind(total - paid)
                .bind(total_cost)
                .bind(input.sale_id)
                .execute(&mut *tx)
                .await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "sale.edit",
                    "sale",
                    input.sale_id,
                    &correlation,
                    None,
                    None,
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    sale_dto(state, input.sale_id).await
}
