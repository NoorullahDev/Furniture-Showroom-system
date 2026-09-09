use crate::application::auth::Principal;
use crate::dto::catalogue::{CategoryDto, ProductTypeDto, UnitDto};
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::infrastructure::AuditInput;
use crate::state::AppState;

const MAX_NAME_LEN: usize = 120;

fn validate_name(label: &str, value: &str) -> Result<String, AppError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_NAME_LEN {
        return Err(AppError::Validation(format!(
            "{label} is required and must be {MAX_NAME_LEN} characters or fewer"
        )));
    }
    Ok(trimmed)
}

/// Flat list of categories so the frontend can render the tree itself. Every
/// authenticated role may read the catalogue.
pub async fn list_categories(
    state: &AppState,
    _principal: &Principal,
) -> Result<Vec<CategoryDto>, AppError> {
    let rows: Vec<(i64, String, Option<i64>, i64, i64, i64)> = sqlx::query_as(
        "SELECT c.id, c.name, c.parent_id, c.sort_order, c.is_active,
                (SELECT COUNT(*) FROM products p WHERE p.category_id = c.id) AS product_count
         FROM categories c
         ORDER BY c.sort_order, c.name COLLATE NOCASE",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, name, parent_id, sort_order, is_active, product_count)| CategoryDto {
                id,
                name,
                parent_id,
                sort_order,
                is_active: is_active != 0,
                product_count,
            },
        )
        .collect())
}

pub async fn create_category(
    state: &AppState,
    principal: &Principal,
    name: &str,
    parent_id: Option<i64>,
    sort_order: i64,
    correlation_id: &str,
) -> Result<CategoryDto, AppError> {
    principal.require("product.create")?;
    let name = validate_name("category name", name)?;
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let dto_name = name.clone();

    let created = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let name = name.clone();
            Box::pin(async move {
                if let Some(parent) = parent_id {
                    let exists: Option<i64> =
                        sqlx::query_scalar("SELECT 1 FROM categories WHERE id = ?")
                            .bind(parent)
                            .fetch_optional(&mut *tx)
                            .await?;
                    if exists.is_none() {
                        return Err(AppError::Validation(
                            "parent category does not exist".into(),
                        ));
                    }
                }
                let duplicate: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM categories
                     WHERE name COLLATE NOCASE = ?
                       AND COALESCE(parent_id, -1) = COALESCE(?, -1)",
                )
                .bind(&name)
                .bind(parent_id)
                .fetch_optional(&mut *tx)
                .await?;
                if duplicate.is_some() {
                    return Err(AppError::Conflict(format!(
                        "a sibling category named `{name}` already exists"
                    )));
                }

                let id = sqlx::query(
                    "INSERT INTO categories (name, parent_id, sort_order, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind(&name)
                .bind(parent_id)
                .bind(sort_order)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "category.create".into(),
                            entity_type: Some("category".into()),
                            entity_id: Some(id.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "name": name,
                                    "parent_id": parent_id,
                                    "sort_order": sort_order,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(id)
            })
        })
        .await?;

    Ok(CategoryDto {
        id: created,
        name: dto_name,
        parent_id,
        sort_order,
        is_active: true,
        product_count: 0,
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn update_category(
    state: &AppState,
    principal: &Principal,
    category_id: i64,
    name: Option<String>,
    parent_id: Option<Option<i64>>,
    sort_order: Option<i64>,
    is_active: Option<bool>,
    correlation_id: &str,
) -> Result<CategoryDto, AppError> {
    principal.require("product.create")?;
    let current = load_category(state, category_id).await?;
    let name = match &name {
        Some(value) => validate_name("category name", value)?,
        None => current.0.clone(),
    };
    let new_parent = parent_id.unwrap_or(current.1);
    let new_sort = sort_order.unwrap_or(current.2);
    let new_active = is_active.unwrap_or(current.3);
    let result_name = name.clone();
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let before_json = serde_json::json!({
        "name": current.0,
        "parent_id": current.1,
        "sort_order": current.2,
        "is_active": current.3,
    })
    .to_string();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let name = name.clone();
            Box::pin(async move {
                if let Some(parent) = new_parent {
                    if parent == category_id {
                        return Err(AppError::Validation(
                            "a category cannot be its own parent".into(),
                        ));
                    }
                    let exists: Option<i64> =
                        sqlx::query_scalar("SELECT 1 FROM categories WHERE id = ?")
                            .bind(parent)
                            .fetch_optional(&mut *tx)
                            .await?;
                    if exists.is_none() {
                        return Err(AppError::Validation("parent category does not exist".into()));
                    }
                }
                let duplicate: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM categories
                     WHERE name COLLATE NOCASE = ?
                       AND COALESCE(parent_id, -1) = COALESCE(?, -1)
                       AND id != ?",
                )
                .bind(&name)
                .bind(new_parent)
                .bind(category_id)
                .fetch_optional(&mut *tx)
                .await?;
                if duplicate.is_some() {
                    return Err(AppError::Conflict(format!(
                        "a sibling category named `{name}` already exists"
                    )));
                }

                sqlx::query(
                    "UPDATE categories SET name = ?, parent_id = ?, sort_order = ?, is_active = ?, updated_at = ?
                     WHERE id = ?",
                )
                .bind(&name)
                .bind(new_parent)
                .bind(new_sort)
                .bind(new_active as i64)
                .bind(&now)
                .bind(category_id)
                .execute(&mut *tx)
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "category.update".into(),
                            entity_type: Some("category".into()),
                            entity_id: Some(category_id.to_string()),
                            before_json: Some(before_json),
                            after_json: Some(
                                serde_json::json!({
                                    "name": name,
                                    "parent_id": new_parent,
                                    "sort_order": new_sort,
                                    "is_active": new_active,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await?;

    Ok(CategoryDto {
        id: category_id,
        name: result_name,
        parent_id: new_parent,
        sort_order: new_sort,
        is_active: new_active,
        product_count: load_category_count(state, category_id).await?,
    })
}

pub async fn archive_category(
    state: &AppState,
    principal: &Principal,
    category_id: i64,
    reason: &str,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("product.create")?;
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let reason_owned = reason.to_string();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let before: Option<(String, i64)> =
                    sqlx::query_as("SELECT name, is_active FROM categories WHERE id = ?")
                        .bind(category_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                let Some((name, was_active)) = before else {
                    return Err(AppError::NotFound(format!("category {category_id}")));
                };
                if was_active == 0 {
                    return Err(AppError::Conflict("category is already archived".into()));
                }

                sqlx::query("UPDATE categories SET is_active = 0, updated_at = ? WHERE id = ?")
                    .bind(&now)
                    .bind(category_id)
                    .execute(&mut *tx)
                    .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "category.archive".into(),
                            entity_type: Some("category".into()),
                            entity_id: Some(category_id.to_string()),
                            before_json: Some(format!("{{\"name\":\"{name}\"}}")),
                            reason: Some(reason_owned),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await
}

pub async fn create_product_type(
    state: &AppState,
    principal: &Principal,
    category_id: i64,
    name: &str,
    correlation_id: &str,
) -> Result<ProductTypeDto, AppError> {
    principal.require("product.create")?;
    let name = validate_name("product type name", name)?;
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let dto_name = name.clone();

    let created = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let name = name.clone();
            Box::pin(async move {
                let category_exists: Option<i64> =
                    sqlx::query_scalar("SELECT 1 FROM categories WHERE id = ? AND is_active = 1")
                        .bind(category_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                if category_exists.is_none() {
                    return Err(AppError::Validation(
                        "category does not exist or is not active".into(),
                    ));
                }
                let duplicate: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM product_types
                     WHERE category_id = ? AND name COLLATE NOCASE = ?",
                )
                .bind(category_id)
                .bind(&name)
                .fetch_optional(&mut *tx)
                .await?;
                if duplicate.is_some() {
                    return Err(AppError::Conflict(format!(
                        "a product type named `{name}` already exists in this category"
                    )));
                }

                let id = sqlx::query(
                    "INSERT INTO product_types (category_id, name, created_at, updated_at)
                     VALUES (?, ?, ?, ?)",
                )
                .bind(category_id)
                .bind(&name)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product_type.create".into(),
                            entity_type: Some("product_type".into()),
                            entity_id: Some(id.to_string()),
                            after_json: Some(
                                serde_json::json!({"category_id": category_id, "name": name})
                                    .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(id)
            })
        })
        .await?;

    Ok(ProductTypeDto {
        id: created,
        category_id,
        name: dto_name,
        is_active: true,
        product_count: 0,
    })
}

pub async fn update_product_type(
    state: &AppState,
    principal: &Principal,
    product_type_id: i64,
    name: Option<String>,
    is_active: Option<bool>,
    correlation_id: &str,
) -> Result<ProductTypeDto, AppError> {
    principal.require("product.create")?;
    let current = load_product_type(state, product_type_id).await?;
    let name = match &name {
        Some(value) => validate_name("product type name", value)?,
        None => current.1.clone(),
    };
    let new_active = is_active.unwrap_or(current.2);
    let result_name = name.clone();
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let category_id = current.0;
    let before_json =
        serde_json::json!({"category_id": category_id, "name": current.1, "is_active": current.2})
            .to_string();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let name = name.clone();
            Box::pin(async move {
                let duplicate: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM product_types
                     WHERE category_id = ? AND name COLLATE NOCASE = ? AND id != ?",
                )
                .bind(category_id)
                .bind(&name)
                .bind(product_type_id)
                .fetch_optional(&mut *tx)
                .await?;
                if duplicate.is_some() {
                    return Err(AppError::Conflict(format!(
                        "a product type named `{name}` already exists in this category"
                    )));
                }

                sqlx::query(
                    "UPDATE product_types SET name = ?, is_active = ?, updated_at = ? WHERE id = ?",
                )
                .bind(&name)
                .bind(new_active as i64)
                .bind(&now)
                .bind(product_type_id)
                .execute(&mut *tx)
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product_type.update".into(),
                            entity_type: Some("product_type".into()),
                            entity_id: Some(product_type_id.to_string()),
                            before_json: Some(before_json),
                            after_json: Some(
                                serde_json::json!({
                                    "category_id": category_id,
                                    "name": name,
                                    "is_active": new_active,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await?;

    Ok(ProductTypeDto {
        id: product_type_id,
        category_id,
        name: result_name,
        is_active: new_active,
        product_count: load_type_count(state, product_type_id).await?,
    })
}

pub async fn archive_product_type(
    state: &AppState,
    principal: &Principal,
    product_type_id: i64,
    reason: &str,
    correlation_id: &str,
) -> Result<(), AppError> {
    principal.require("product.create")?;
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let reason_owned = reason.to_string();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let before: Option<(String, i64)> =
                    sqlx::query_as("SELECT name, is_active FROM product_types WHERE id = ?")
                        .bind(product_type_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                let Some((name, was_active)) = before else {
                    return Err(AppError::NotFound(format!(
                        "product type {product_type_id}"
                    )));
                };
                if was_active == 0 {
                    return Err(AppError::Conflict(
                        "product type is already archived".into(),
                    ));
                }

                sqlx::query("UPDATE product_types SET is_active = 0, updated_at = ? WHERE id = ?")
                    .bind(&now)
                    .bind(product_type_id)
                    .execute(&mut *tx)
                    .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product_type.archive".into(),
                            entity_type: Some("product_type".into()),
                            entity_id: Some(product_type_id.to_string()),
                            before_json: Some(format!("{{\"name\":\"{name}\"}}")),
                            reason: Some(reason_owned),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await
}

pub async fn list_product_types(
    state: &AppState,
    _principal: &Principal,
    category_id: Option<i64>,
) -> Result<Vec<ProductTypeDto>, AppError> {
    let rows: Vec<(i64, i64, String, i64, i64)> = sqlx::query_as(
        "SELECT t.id, t.category_id, t.name, t.is_active,
                (SELECT COUNT(*) FROM products p WHERE p.product_type_id = t.id) AS product_count
         FROM product_types t
         WHERE (?1 IS NULL OR t.category_id = ?1)
         ORDER BY t.name COLLATE NOCASE",
    )
    .bind(category_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, cat_id, name, is_active, product_count)| ProductTypeDto {
                id,
                category_id: cat_id,
                name,
                is_active: is_active != 0,
                product_count,
            },
        )
        .collect())
}

pub async fn list_units(
    state: &AppState,
    _principal: &Principal,
) -> Result<Vec<UnitDto>, AppError> {
    let rows: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT id, name, code FROM units WHERE is_active = 1 ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, name, code)| UnitDto { id, name, code })
        .collect())
}

type CategoryRow = (String, Option<i64>, i64, bool);

async fn load_category(state: &AppState, category_id: i64) -> Result<CategoryRow, AppError> {
    let row: Option<(String, Option<i64>, i64, i64)> = sqlx::query_as(
        "SELECT name, parent_id, sort_order, is_active FROM categories WHERE id = ?",
    )
    .bind(category_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((name, parent_id, sort_order, is_active)) = row else {
        return Err(AppError::NotFound(format!("category {category_id}")));
    };
    Ok((name, parent_id, sort_order, is_active != 0))
}

async fn load_category_count(state: &AppState, category_id: i64) -> Result<i64, AppError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products WHERE category_id = ?")
        .bind(category_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(count)
}

type TypeRow = (i64, String, bool);

async fn load_product_type(state: &AppState, product_type_id: i64) -> Result<TypeRow, AppError> {
    let row: Option<(i64, String, i64)> =
        sqlx::query_as("SELECT category_id, name, is_active FROM product_types WHERE id = ?")
            .bind(product_type_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some((category_id, name, is_active)) = row else {
        return Err(AppError::NotFound(format!(
            "product type {product_type_id}"
        )));
    };
    Ok((category_id, name, is_active != 0))
}

async fn load_type_count(state: &AppState, product_type_id: i64) -> Result<i64, AppError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products WHERE product_type_id = ?")
        .bind(product_type_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(count)
}
