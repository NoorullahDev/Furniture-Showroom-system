use sqlx::sqlite::SqliteConnection;
use sqlx::Row;

use crate::application::auth::Principal;
use crate::application::suppliers::state_audit;
use crate::dto::sales::{BundleAvailabilityDto, BundleDto, BundleInput, BundleItemDto};
use crate::error::AppError;
use crate::state::AppState;

pub(crate) async fn require_bundle(
    conn: &mut SqliteConnection,
    bundle_id: i64,
) -> Result<(), AppError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM bundles WHERE id = ? AND is_active = 1")
            .bind(bundle_id)
            .fetch_optional(conn)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!("bundle {bundle_id}")));
    }
    Ok(())
}

/// FIFO weighted-average unit cost estimate from the open cost layers.
pub(crate) async fn product_cost_estimate(
    conn: &mut SqliteConnection,
    product_id: i64,
) -> Result<i64, AppError> {
    let cost: Option<i64> = sqlx::query_scalar(
        "SELECT CAST(SUM(unit_cost_minor * quantity) AS INTEGER) / SUM(quantity)
           FROM inventory_cost_layers WHERE product_id = ? AND quantity > 0",
    )
    .bind(product_id)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(cost.unwrap_or(0))
}

pub(crate) async fn bundle_cost_estimate(
    conn: &mut SqliteConnection,
    bundle_id: i64,
) -> Result<i64, AppError> {
    let costs: Vec<i64> = sqlx::query_scalar(
        "SELECT bi.quantity * COALESCE(
                 (SELECT CAST(SUM(unit_cost_minor * quantity) AS INTEGER) / SUM(quantity)
                    FROM inventory_cost_layers l WHERE l.product_id = bi.product_id AND l.quantity > 0),
                 0)
           FROM bundle_items bi WHERE bi.bundle_id = ?",
    )
    .bind(bundle_id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(costs.iter().sum())
}

fn map_bundle_item(r: &sqlx::sqlite::SqliteRow) -> (i64, String, String, i64, i64) {
    (r.get(0), r.get(1), r.get(2), r.get(3), r.get(4))
}

pub async fn create(
    state: &AppState,
    principal: &Principal,
    input: BundleInput,
    correlation_id: &str,
) -> Result<BundleDto, AppError> {
    principal.require("bundle.create")?;

    let code = input.code.trim().to_uppercase();
    if code.is_empty() {
        return Err(AppError::Validation("bundle code is required".into()));
    }
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("bundle name is required".into()));
    }
    if input.items.is_empty() {
        return Err(AppError::Validation(
            "bundle must have at least one item".into(),
        ));
    }
    for item in &input.items {
        if item.quantity <= 0 {
            return Err(AppError::Validation(
                "bundle item quantities must be positive".into(),
            ));
        }
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
            let description = input.description.clone();
            let cover = input.cover_image_path.clone();
            Box::pin(async move {
                let existing: Option<i64> =
                    sqlx::query_scalar("SELECT 1 FROM bundles WHERE code = ?")
                        .bind(&code)
                        .fetch_optional(&mut *tx)
                        .await?;
                if existing.is_some() {
                    return Err(AppError::Conflict(format!(
                        "bundle code '{code}' already exists"
                    )));
                }

                let id = sqlx::query(
                    "INSERT INTO bundles (code, name, description, cover_image_path,
                                          default_price_minor, created_by)
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(&code)
                .bind(&name)
                .bind(description.as_deref())
                .bind(cover.as_deref())
                .bind(input.default_price_minor.unwrap_or(0))
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                insert_bundle_items(&mut *tx, id, &input.items).await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "bundle.create",
                    "bundle",
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

    bundle_dto(state, id).await
}

pub async fn update(
    state: &AppState,
    principal: &Principal,
    bundle_id: i64,
    input: BundleInput,
    correlation_id: &str,
) -> Result<BundleDto, AppError> {
    principal.require("bundle.create")?;

    let code = input.code.trim().to_uppercase();
    if code.is_empty() {
        return Err(AppError::Validation("bundle code is required".into()));
    }
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("bundle name is required".into()));
    }
    if input.items.is_empty() {
        return Err(AppError::Validation(
            "bundle must have at least one item".into(),
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
            let description = input.description.clone();
            let cover = input.cover_image_path.clone();
            let is_active = input.is_active.unwrap_or(true);
            Box::pin(async move {
                let conflict: Option<i64> =
                    sqlx::query_scalar("SELECT 1 FROM bundles WHERE code = ? AND id != ?")
                        .bind(&code)
                        .bind(bundle_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                if conflict.is_some() {
                    return Err(AppError::Conflict(format!(
                        "bundle code '{code}' already exists"
                    )));
                }
                let updated = sqlx::query(
                    "UPDATE bundles
                        SET code = ?, name = ?, description = ?, cover_image_path = ?,
                            default_price_minor = ?, is_active = ?,
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                      WHERE id = ?",
                )
                .bind(&code)
                .bind(&name)
                .bind(description.as_deref())
                .bind(cover.as_deref())
                .bind(input.default_price_minor.unwrap_or(0))
                .bind(if is_active { 1 } else { 0 })
                .bind(bundle_id)
                .execute(&mut *tx)
                .await?;
                if updated.rows_affected() == 0 {
                    return Err(AppError::NotFound(format!("bundle {bundle_id}")));
                }

                sqlx::query("DELETE FROM bundle_items WHERE bundle_id = ?")
                    .bind(bundle_id)
                    .execute(&mut *tx)
                    .await?;
                insert_bundle_items(&mut *tx, bundle_id, &input.items).await?;

                state_audit(
                    &mut *tx,
                    &audits,
                    actor_id,
                    &actor_session,
                    "bundle.update",
                    "bundle",
                    bundle_id,
                    &correlation,
                    None,
                    None,
                )
                .await?;
                Ok(())
            })
        })
        .await?;

    bundle_dto(state, bundle_id).await
}

async fn insert_bundle_items(
    tx: &mut SqliteConnection,
    bundle_id: i64,
    items: &[crate::dto::sales::BundleItemInput],
) -> Result<(), AppError> {
    for (index, item) in items.iter().enumerate() {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM products WHERE id = ? AND archived_at IS NULL")
                .bind(item.product_id)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(AppError::NotFound(format!("product {}", item.product_id)));
        }
        sqlx::query(
            "INSERT INTO bundle_items (bundle_id, product_id, quantity, sort_order)
             VALUES (?, ?, ?, ?)",
        )
        .bind(bundle_id)
        .bind(item.product_id)
        .bind(item.quantity)
        .bind(index as i64)
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}

pub async fn bundle_dto(state: &AppState, bundle_id: i64) -> Result<BundleDto, AppError> {
    let row = sqlx::query(
        "SELECT id, code, name, description, cover_image_path,
                default_price_minor, is_active, created_at
         FROM bundles WHERE id = ?",
    )
    .bind(bundle_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("bundle {bundle_id}")))?;

    let item_rows = sqlx::query(
        "SELECT bi.product_id, p.article_number, p.name, bi.quantity, bi.sort_order
         FROM bundle_items bi
         JOIN products p ON p.id = bi.product_id
         WHERE bi.bundle_id = ? ORDER BY bi.sort_order, bi.id",
    )
    .bind(bundle_id)
    .fetch_all(&state.pool)
    .await?;

    let mut conn = state.pool.acquire().await?;
    let mut items = Vec::with_capacity(item_rows.len());
    for r in &item_rows {
        let (product_id, article, name, qty, _sort) = map_bundle_item(r);
        let unit_cost = product_cost_estimate(&mut conn, product_id).await?;
        items.push(BundleItemDto {
            product_id,
            article_number: article,
            product_name: name,
            quantity: qty,
            sort_order: _sort,
            unit_cost_estimate_minor: unit_cost,
            line_cost_estimate_minor: unit_cost * qty,
        });
    }
    drop(conn);

    let cost_estimate = items.iter().map(|i| i.line_cost_estimate_minor).sum();

    Ok(BundleDto {
        id: row.get(0),
        code: row.get(1),
        name: row.get(2),
        description: row.try_get(3).ok(),
        cover_image_path: row.try_get(4).ok(),
        default_price_minor: row.get(5),
        is_active: row.get::<i64, _>(6) != 0,
        items,
        cost_estimate_minor: cost_estimate,
        created_at: row.get(7),
    })
}

pub async fn list(state: &AppState, principal: &Principal) -> Result<Vec<BundleDto>, AppError> {
    principal.require("bundle.view")?;
    let rows: Vec<i64> = sqlx::query_scalar("SELECT id FROM bundles ORDER BY is_active DESC, name")
        .fetch_all(&state.pool)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for id in rows {
        out.push(bundle_dto(state, id).await?);
    }
    Ok(out)
}

pub async fn get(
    state: &AppState,
    principal: &Principal,
    bundle_id: i64,
) -> Result<BundleDto, AppError> {
    principal.require("bundle.view")?;
    bundle_dto(state, bundle_id).await
}

/// How many complete sets can be assembled right now at a location. The limiting
/// component (lowest floor(available / needed)) drives the answer.
pub async fn availability(
    state: &AppState,
    principal: &Principal,
    bundle_id: i64,
    location_id: i64,
) -> Result<BundleAvailabilityDto, AppError> {
    principal.require("bundle.view")?;
    let mut conn = state.pool.acquire().await?;
    require_bundle(&mut conn, bundle_id).await?;

    let comps: Vec<(i64, String, i64, i64)> = sqlx::query_as(
        "SELECT bi.product_id, p.name, bi.quantity,
                COALESCE((SELECT on_hand - reserved - damaged FROM stock_balances
                           WHERE product_id = bi.product_id AND location_id = ?), 0)
           FROM bundle_items bi
           JOIN products p ON p.id = bi.product_id
          WHERE bi.bundle_id = ?",
    )
    .bind(location_id)
    .bind(bundle_id)
    .fetch_all(&mut *conn)
    .await?;

    let (
        mut best_count,
        mut limiting_id,
        mut limiting_name,
        mut limiting_avail,
        mut limiting_needed,
    ) = (i64::MAX, None, None, None, None);
    for (product_id, name, needed, available) in &comps {
        let count = if *needed > 0 { available / needed } else { 0 };
        if count < best_count {
            best_count = count;
            limiting_id = Some(*product_id);
            limiting_name = Some(name.clone());
            limiting_avail = Some(*available);
            limiting_needed = Some(*needed);
        }
    }

    let bundle_name: String = sqlx::query_scalar("SELECT name FROM bundles WHERE id = ?")
        .bind(bundle_id)
        .fetch_one(&mut *conn)
        .await?;

    Ok(BundleAvailabilityDto {
        bundle_id,
        bundle_name,
        location_id,
        available_count: if comps.is_empty() { 0 } else { best_count },
        limiting_product_id: limiting_id,
        limiting_product_name: limiting_name,
        limiting_available: limiting_avail,
        limiting_needed_per_set: limiting_needed,
    })
}
