use std::path::PathBuf;

use sqlx::sqlite::SqliteConnection;
use sqlx::Row;

use crate::application::auth::Principal;
use crate::dto::catalogue::{
    AttributeInputDto, CreateProductInput, ProductDetailDto, ProductImageDto, ProductListItemDto,
    UpdateProductInput,
};
use crate::error::AppError;
use crate::infrastructure::{self, clock::Clock, AuditInput, ImportedImage};
use crate::state::AppState;

const MAX_ARTICLE_LEN: usize = 64;
const MAX_NAME_LEN: usize = 200;
const MAX_LONG_TEXT_LEN: usize = 4000;
const MAX_ATTR_NAME_LEN: usize = 60;
const MAX_ATTR_VALUE_LEN: usize = 200;
const MAX_ATTRIBUTES: usize = 60;
const SEARCH_LIMIT: i64 = 300;

/// Return the display form (trimmed, collapsed whitespace) and the normalized
/// form (upper-cased) used for case-insensitive uniqueness and lookup.
fn normalize_article_number(raw: &str) -> Result<(String, String), AppError> {
    let display = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if display.is_empty() || display.len() > MAX_ARTICLE_LEN {
        return Err(AppError::Validation(format!(
            "article number is required and must be {MAX_ARTICLE_LEN} characters or fewer"
        )));
    }
    if display.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "article number contains invalid characters".into(),
        ));
    }
    Ok((display.clone(), display.to_uppercase()))
}

fn required_text(label: &str, value: &str, max: usize) -> Result<String, AppError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() || trimmed.chars().count() > max {
        return Err(AppError::Validation(format!(
            "{label} is required and must be {max} characters or fewer"
        )));
    }
    Ok(trimmed)
}

/// Optional free text: None keeps the current value, an empty string clears it
/// (stored as NULL).
fn optional_text(
    value: &Option<String>,
    label: &str,
    max: usize,
) -> Result<Option<String>, AppError> {
    match value {
        None => Ok(None),
        Some(raw) => {
            let trimmed = raw.trim().to_string();
            if trimmed.is_empty() {
                return Ok(None);
            }
            if trimmed.chars().count() > max {
                return Err(AppError::Validation(format!(
                    "{label} must be {max} characters or fewer"
                )));
            }
            Ok(Some(trimmed))
        }
    }
}

fn validate_minor(label: &str, value: i64) -> Result<(), AppError> {
    if value < 0 {
        return Err(AppError::Validation(format!("{label} cannot be negative")));
    }
    Ok(())
}

fn normalize_attributes(
    attributes: &[AttributeInputDto],
) -> Result<Vec<(String, String)>, AppError> {
    if attributes.len() > MAX_ATTRIBUTES {
        return Err(AppError::Validation(format!(
            "at most {MAX_ATTRIBUTES} attributes are allowed"
        )));
    }
    let mut out = Vec::with_capacity(attributes.len());
    for attr in attributes {
        let name = attr.name.trim().to_string();
        let value = attr.value.trim().to_string();
        if name.is_empty() || name.chars().count() > MAX_ATTR_NAME_LEN {
            return Err(AppError::Validation(format!(
                "attribute names must be non-empty and {MAX_ATTR_NAME_LEN} characters or fewer"
            )));
        }
        if value.is_empty() || value.chars().count() > MAX_ATTR_VALUE_LEN {
            return Err(AppError::Validation(format!(
                "attribute values must be non-empty and {MAX_ATTR_VALUE_LEN} characters or fewer"
            )));
        }
        out.push((name, value));
    }
    Ok(out)
}

fn safe_relative_filename(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\'])
}

/// Import image files into the images directory, staging them under generated
/// names. If any file fails, the already-imported files are removed so a failed
/// product create never leaves orphaned images behind.
async fn import_images(state: &AppState, paths: &[String]) -> Result<Vec<ImportedImage>, AppError> {
    let images_dir = state.paths.images_dir.clone();
    let mut imported: Vec<ImportedImage> = Vec::new();
    for path in paths {
        if path.trim().is_empty() {
            continue;
        }
        let source = PathBuf::from(path);
        let dir = images_dir.clone();
        let result =
            tokio::task::spawn_blocking(move || infrastructure::import_image(&source, &dir))
                .await
                .map_err(|e| AppError::Internal(format!("background task failed: {e}")))?;
        match result {
            Ok(img) => imported.push(img),
            Err(err) => {
                discard_imported(&images_dir, &imported);
                return Err(err);
            }
        }
    }
    Ok(imported)
}

fn discard_imported(images_dir: &std::path::Path, imported: &[ImportedImage]) {
    let names: Vec<&str> = imported
        .iter()
        .flat_map(|img| [img.stored_name.as_str(), img.thumbnail_name.as_str()])
        .collect();
    if !names.is_empty() {
        let _ = infrastructure::discard_generated_images(images_dir, &names);
    }
}

async fn ensure_category_active(
    conn: &mut SqliteConnection,
    category_id: i64,
) -> Result<(), AppError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM categories WHERE id = ? AND is_active = 1")
            .bind(category_id)
            .fetch_optional(&mut *conn)
            .await?;
    if exists.is_none() {
        return Err(AppError::Validation(
            "category does not exist or is not active".into(),
        ));
    }
    Ok(())
}

async fn ensure_type_belongs(
    conn: &mut SqliteConnection,
    category_id: i64,
    product_type_id: i64,
) -> Result<(), AppError> {
    let exists: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM product_types WHERE id = ? AND category_id = ? AND is_active = 1",
    )
    .bind(product_type_id)
    .bind(category_id)
    .fetch_optional(&mut *conn)
    .await?;
    if exists.is_none() {
        return Err(AppError::Validation(
            "product type does not exist, is not active, or does not belong to this category"
                .into(),
        ));
    }
    Ok(())
}

async fn ensure_unit_exists(conn: &mut SqliteConnection, unit_id: i64) -> Result<(), AppError> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM units WHERE id = ?")
        .bind(unit_id)
        .fetch_optional(&mut *conn)
        .await?;
    if exists.is_none() {
        return Err(AppError::Validation("unit does not exist".into()));
    }
    Ok(())
}

async fn article_in_use(
    conn: &mut SqliteConnection,
    norm: &str,
    exclude_product_id: Option<i64>,
) -> Result<bool, AppError> {
    let found: Option<i64> = if let Some(exclude) = exclude_product_id {
        sqlx::query_scalar(
            "SELECT 1 FROM products
             WHERE article_number_norm = ? AND archived_at IS NULL AND id != ?",
        )
        .bind(norm)
        .bind(exclude)
        .fetch_optional(&mut *conn)
        .await?
    } else {
        sqlx::query_scalar(
            "SELECT 1 FROM products WHERE article_number_norm = ? AND archived_at IS NULL",
        )
        .bind(norm)
        .fetch_optional(&mut *conn)
        .await?
    };
    Ok(found.is_some())
}

pub async fn create_product(
    state: &AppState,
    principal: &Principal,
    input: CreateProductInput,
    correlation_id: &str,
) -> Result<ProductDetailDto, AppError> {
    principal.require("product.create")?;
    let (display_article, norm_article) = normalize_article_number(&input.article_number)?;
    let name = required_text("product name", &input.name, MAX_NAME_LEN)?;
    let description = optional_text(&input.description, "description", MAX_LONG_TEXT_LEN)?;
    let material = optional_text(&input.material, "material", 200)?;
    let color = optional_text(&input.color, "color", 200)?;
    let dimensions_text = optional_text(&input.dimensions_text, "dimensions", 200)?;
    let brand = optional_text(&input.brand, "brand", 200)?;
    let barcode = optional_text(&input.barcode, "barcode", 64)?;
    let notes = optional_text(&input.notes, "notes", MAX_LONG_TEXT_LEN)?;
    validate_minor("purchase cost", input.cost_minor)?;
    validate_minor("sale price", input.sale_price_minor)?;
    validate_minor("minimum stock", input.minimum_stock)?;
    let attributes = normalize_attributes(&input.attributes)?;
    let image_paths: Vec<String> = input
        .image_paths
        .iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if image_paths.len() > infrastructure::MAX_PRODUCT_IMAGES as usize {
        return Err(AppError::Validation(format!(
            "at most {} images per product",
            infrastructure::MAX_PRODUCT_IMAGES
        )));
    }

    let imported = import_images(state, &image_paths).await?;
    let staged_names: Vec<String> = imported
        .iter()
        .flat_map(|img| vec![img.stored_name.clone(), img.thumbnail_name.clone()])
        .collect();

    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let category_id = input.category_id;
    let product_type_id = input.product_type_id;
    let unit_id = input.unit_id;
    let warranty_months = input.warranty_months;
    let cost_minor = input.cost_minor;
    let sale_price_minor = input.sale_price_minor;
    let minimum_stock = input.minimum_stock;
    let track_stock = input.track_stock;
    let attr_count = attributes.len();
    let image_count = imported.len();
    let description = description.clone();
    let notes = notes.clone();
    let created_at = now.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let display_article = display_article.clone();
            let norm_article = norm_article.clone();
            let name = name.clone();
            let description = description.clone();
            let notes = notes.clone();
            let material = material.clone();
            let color = color.clone();
            let dimensions_text = dimensions_text.clone();
            let brand = brand.clone();
            let barcode = barcode.clone();
            Box::pin(async move {
                if article_in_use(tx, &norm_article, None).await? {
                    return Err(AppError::Conflict(format!(
                        "article number `{display_article}` is already in use by a live product"
                    )));
                }
                ensure_category_active(tx, category_id).await?;
                if let Some(tid) = product_type_id {
                    ensure_type_belongs(tx, category_id, tid).await?;
                }
                if let Some(uid) = unit_id {
                    ensure_unit_exists(tx, uid).await?;
                }

                let product_id = sqlx::query(
                    "INSERT INTO products (
                        article_number, article_number_norm, name, category_id, product_type_id,
                        unit_id, description, material, color, dimensions_text, brand, barcode,
                        warranty_months, notes, cost_minor, sale_price_minor, minimum_stock,
                        track_stock, created_at, updated_at
                     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&display_article)
                .bind(&norm_article)
                .bind(&name)
                .bind(category_id)
                .bind(product_type_id)
                .bind(unit_id)
                .bind(&description)
                .bind(&material)
                .bind(&color)
                .bind(&dimensions_text)
                .bind(&brand)
                .bind(&barcode)
                .bind(warranty_months)
                .bind(&notes)
                .bind(cost_minor)
                .bind(sale_price_minor)
                .bind(minimum_stock)
                .bind(track_stock as i64)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                insert_images(tx, product_id, &imported, &created_at).await?;
                insert_attributes(tx, product_id, &attributes).await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product.create".into(),
                            entity_type: Some("product".into()),
                            entity_id: Some(product_id.to_string()),
                            after_json: Some(
                                serde_json::json!({
                                    "article_number": display_article,
                                    "name": name,
                                    "category_id": category_id,
                                    "product_type_id": product_type_id,
                                    "unit_id": unit_id,
                                    "sale_price_minor": sale_price_minor,
                                    "cost_minor": cost_minor,
                                    "track_stock": track_stock,
                                    "attribute_count": attr_count,
                                    "image_count": image_count,
                                })
                                .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(product_id)
            })
        })
        .await;

    let product_id = match result {
        Ok(id) => id,
        Err(err) => {
            discard_generated(&state.paths.images_dir, &staged_names);
            return Err(err);
        }
    };

    get_product_inner(state, principal, product_id).await
}

async fn insert_images(
    tx: &mut SqliteConnection,
    product_id: i64,
    images: &[ImportedImage],
    created_at: &str,
) -> Result<(), AppError> {
    if images.len() as i64 > infrastructure::MAX_PRODUCT_IMAGES {
        return Err(AppError::Validation(format!(
            "at most {} images per product",
            infrastructure::MAX_PRODUCT_IMAGES
        )));
    }
    for (idx, img) in images.iter().enumerate() {
        sqlx::query(
            "INSERT INTO product_images (
                product_id, relative_path, thumbnail_path, sort_order, is_primary,
                sha256, width, height, mime_type, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(product_id)
        .bind(&img.stored_name)
        .bind(&img.thumbnail_name)
        .bind(idx as i64)
        .bind(if idx == 0 { 1 } else { 0 })
        .bind(&img.sha256)
        .bind(img.width as i64)
        .bind(img.height as i64)
        .bind(&img.mime_type)
        .bind(created_at)
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}

async fn insert_attributes(
    tx: &mut SqliteConnection,
    product_id: i64,
    attributes: &[(String, String)],
) -> Result<(), AppError> {
    for (idx, (name, value)) in attributes.iter().enumerate() {
        sqlx::query(
            "INSERT INTO product_attributes (product_id, attribute_name, attribute_value, sort_order)
             VALUES (?, ?, ?, ?)",
        )
        .bind(product_id)
        .bind(name)
        .bind(value)
        .bind(idx as i64)
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}

pub async fn update_product(
    state: &AppState,
    principal: &Principal,
    product_id: i64,
    input: UpdateProductInput,
    correlation_id: &str,
) -> Result<ProductDetailDto, AppError> {
    principal.require("product.create")?;
    let current = load_product_snapshot(state, product_id).await?;

    let (display_article, norm_article) = match &input.article_number {
        Some(raw) => {
            let (d, n) = normalize_article_number(raw)?;
            (Some(d), Some(n))
        }
        None => (None, None),
    };
    let name = match &input.name {
        Some(raw) => Some(required_text("product name", raw, MAX_NAME_LEN)?),
        None => None,
    };
    let category_id = input.category_id;
    let product_type_id = input.product_type_id;
    let unit_id = input.unit_id;
    if let Some(raw) = &input.description {
        optional_text(&Some(raw.clone()), "description", MAX_LONG_TEXT_LEN)?;
    }
    let description = input.description.clone();
    let notes = input.notes.clone();
    validate_minor("sale price", input.sale_price_minor.unwrap_or(0))?;
    validate_minor("minimum stock", input.minimum_stock.unwrap_or(0))?;
    if let Some(cost) = input.cost_minor {
        validate_minor("purchase cost", cost)?;
    }
    let attributes = match &input.attributes {
        Some(items) => Some(normalize_attributes(items)?),
        None => None,
    };

    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let before_json = serde_json::to_string(&current)
        .map_err(|e| AppError::Internal(format!("serialize: {e}")))?;

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                if let Some(norm) = &norm_article {
                    if article_in_use(tx, norm, Some(product_id)).await? {
                        return Err(AppError::Conflict(format!(
                            "article number `{}` is already in use by a live product",
                            display_article.as_ref().unwrap()
                        )));
                    }
                }
                if let Some(cat) = category_id {
                    ensure_category_active(tx, cat).await?;
                }
                if let (Some(cat), Some(Some(tid))) = (category_id, product_type_id) {
                    ensure_type_belongs(tx, cat, tid).await?;
                }
                // Moving a product to a new category while leaving its type
                // unchanged must not leave a type that belongs to the old
                // category.
                if let (Some(cat), None) = (category_id, product_type_id) {
                    if let Some(tid) = current.product_type_id {
                        ensure_type_belongs(tx, cat, tid).await?;
                    }
                }
                if let Some(uid) = unit_id.flatten() {
                    ensure_unit_exists(tx, uid).await?;
                }

                let article_changed = display_article.is_some();
                let article = display_article.unwrap_or(current.article_number);
                let article_norm = norm_article.unwrap_or(current.article_number_norm);
                let name = name.unwrap_or(current.name);
                let category_id = category_id.unwrap_or(current.category_id);
                let product_type_id = match product_type_id {
                    Some(opt) => opt,
                    None => current.product_type_id,
                };
                let unit_id = match unit_id {
                    Some(opt) => opt,
                    None => current.unit_id,
                };
                let description = match &description {
                    Some(raw) => {
                        optional_text(&Some(raw.clone()), "description", MAX_LONG_TEXT_LEN)?
                    }
                    None => current.description.clone(),
                };
                let notes = match &notes {
                    Some(raw) => optional_text(&Some(raw.clone()), "notes", MAX_LONG_TEXT_LEN)?,
                    None => current.notes.clone(),
                };

                sqlx::query(
                    "UPDATE products SET
                        article_number = ?, article_number_norm = ?, name = ?, category_id = ?,
                        product_type_id = ?, unit_id = ?, description = ?, material = ?, color = ?,
                        dimensions_text = ?, brand = ?, barcode = ?, warranty_months = ?,
                        notes = ?, cost_minor = ?, sale_price_minor = ?, minimum_stock = ?,
                        track_stock = ?, updated_at = ?
                     WHERE id = ?",
                )
                .bind(&article)
                .bind(&article_norm)
                .bind(&name)
                .bind(category_id)
                .bind(product_type_id)
                .bind(unit_id)
                .bind(&description)
                .bind(input.material.or_else(|| current.material.clone()))
                .bind(input.color.or_else(|| current.color.clone()))
                .bind(
                    input
                        .dimensions_text
                        .or_else(|| current.dimensions_text.clone()),
                )
                .bind(input.brand.or_else(|| current.brand.clone()))
                .bind(input.barcode.or_else(|| current.barcode.clone()))
                .bind(input.warranty_months.or(current.warranty_months))
                .bind(&notes)
                .bind(input.cost_minor.unwrap_or(current.cost_minor))
                .bind(input.sale_price_minor.unwrap_or(current.sale_price_minor))
                .bind(input.minimum_stock.unwrap_or(current.minimum_stock))
                .bind(input.track_stock.unwrap_or(current.track_stock) as i64)
                .bind(&now)
                .bind(product_id)
                .execute(&mut *tx)
                .await?;

                if let Some(attrs) = &attributes {
                    sqlx::query("DELETE FROM product_attributes WHERE product_id = ?")
                        .bind(product_id)
                        .execute(&mut *tx)
                        .await?;
                    insert_attributes(tx, product_id, attrs).await?;
                }

                let after = serde_json::json!({
                    "article_number": article,
                    "name": name,
                    "category_id": category_id,
                    "product_type_id": product_type_id,
                    "unit_id": unit_id,
                    "article_changed": article_changed,
                    "attributes_replaced": attributes.is_some(),
                });
                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product.update".into(),
                            entity_type: Some("product".into()),
                            entity_id: Some(product_id.to_string()),
                            before_json: Some(before_json),
                            after_json: Some(after.to_string()),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await?;

    get_product_inner(state, principal, product_id).await
}

#[derive(serde::Serialize, Clone)]
struct ProductSnapshot {
    article_number: String,
    article_number_norm: String,
    name: String,
    category_id: i64,
    product_type_id: Option<i64>,
    unit_id: Option<i64>,
    description: Option<String>,
    material: Option<String>,
    color: Option<String>,
    dimensions_text: Option<String>,
    brand: Option<String>,
    barcode: Option<String>,
    warranty_months: Option<i64>,
    notes: Option<String>,
    cost_minor: i64,
    sale_price_minor: i64,
    minimum_stock: i64,
    track_stock: bool,
    is_active: bool,
}

async fn load_product_snapshot(
    state: &AppState,
    product_id: i64,
) -> Result<ProductSnapshot, AppError> {
    let row = sqlx::query(
        "SELECT article_number, article_number_norm, name, category_id, product_type_id, unit_id,
                description, material, color, dimensions_text, brand, barcode, notes,
                warranty_months, cost_minor, sale_price_minor, minimum_stock, track_stock,
                is_active
         FROM products WHERE id = ?",
    )
    .bind(product_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(r) = row else {
        return Err(AppError::NotFound(format!("product {product_id}")));
    };
    Ok(ProductSnapshot {
        article_number: r.try_get(0)?,
        article_number_norm: r.try_get(1)?,
        name: r.try_get(2)?,
        category_id: r.try_get(3)?,
        product_type_id: r.try_get(4)?,
        unit_id: r.try_get(5)?,
        description: r.try_get(6)?,
        material: r.try_get(7)?,
        color: r.try_get(8)?,
        dimensions_text: r.try_get(9)?,
        brand: r.try_get(10)?,
        barcode: r.try_get(11)?,
        warranty_months: r.try_get(13)?,
        notes: r.try_get(12)?,
        cost_minor: r.try_get(14)?,
        sale_price_minor: r.try_get(15)?,
        minimum_stock: r.try_get(16)?,
        track_stock: r.try_get(17)?,
        is_active: r.try_get(18)?,
    })
}

pub async fn archive_product(
    state: &AppState,
    principal: &Principal,
    product_id: i64,
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
                let before: Option<(String, Option<String>)> = sqlx::query_as(
                    "SELECT name, archived_at FROM products WHERE id = ?",
                )
                .bind(product_id)
                .fetch_optional(&mut *tx)
                .await?;
                let Some((name, archived_at)) = before else {
                    return Err(AppError::NotFound(format!("product {product_id}")));
                };
                if archived_at.is_some() {
                    return Err(AppError::Conflict("product is already archived".into()));
                }

                sqlx::query(
                    "UPDATE products SET is_active = 0, archived_at = ?, updated_at = ? WHERE id = ?",
                )
                .bind(&now)
                .bind(&now)
                .bind(product_id)
                .execute(&mut *tx)
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product.archive".into(),
                            entity_type: Some("product".into()),
                            entity_id: Some(product_id.to_string()),
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

pub async fn unarchive_product(
    state: &AppState,
    principal: &Principal,
    product_id: i64,
    correlation_id: &str,
) -> Result<ProductDetailDto, AppError> {
    principal.require("product.create")?;
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let before: Option<(String, String, Option<String>)> = sqlx::query_as(
                    "SELECT article_number_norm, name, archived_at FROM products WHERE id = ?",
                )
                .bind(product_id)
                .fetch_optional(&mut *tx)
                .await?;
                let Some((norm, name, archived_at)) = before else {
                    return Err(AppError::NotFound(format!("product {product_id}")));
                };
                if archived_at.is_none() {
                    return Err(AppError::Conflict("product is not archived".into()));
                }
                if article_in_use(tx, &norm, Some(product_id)).await? {
                    return Err(AppError::Conflict(format!(
                        "cannot restore: article number `{name}` is already in use by another live product"
                    )));
                }

                sqlx::query(
                    "UPDATE products SET is_active = 1, archived_at = NULL, updated_at = ? WHERE id = ?",
                )
                .bind(&now)
                .bind(product_id)
                .execute(&mut *tx)
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product.unarchive".into(),
                            entity_type: Some("product".into()),
                            entity_id: Some(product_id.to_string()),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await?;

    get_product_inner(state, principal, product_id).await
}

pub async fn duplicate_product(
    state: &AppState,
    principal: &Principal,
    source_id: i64,
    new_article: &str,
    correlation_id: &str,
) -> Result<ProductDetailDto, AppError> {
    principal.require("product.create")?;
    let (display_article, norm_article) = normalize_article_number(new_article)?;
    let source = load_product_snapshot(state, source_id).await?;
    let source_attrs: Vec<(String, String)> = sqlx::query_as(
        "SELECT attribute_name, attribute_value FROM product_attributes
         WHERE product_id = ? ORDER BY sort_order, id",
    )
    .bind(source_id)
    .fetch_all(&state.pool)
    .await?;
    let source_images: Vec<SourceImage> = sqlx::query_as(
        "SELECT relative_path, thumbnail_path, sha256, width, height FROM product_images
         WHERE product_id = ? ORDER BY sort_order, id",
    )
    .bind(source_id)
    .fetch_all(&state.pool)
    .await?;

    // Re-copy every image file to new generated names so both products own
    // independent files.
    let images_dir = state.paths.images_dir.clone();
    let copied =
        tokio::task::spawn_blocking(move || copy_product_images(&images_dir, &source_images))
            .await
            .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    let staged_names: Vec<String> = copied
        .iter()
        .flat_map(|c| vec![c.1.clone(), c.2.clone()])
        .collect();

    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let display_article = display_article.clone();
            Box::pin(async move {
                if article_in_use(tx, &norm_article, None).await? {
                    return Err(AppError::Conflict(format!(
                        "article number `{display_article}` is already in use by a live product"
                    )));
                }

                let product_id = sqlx::query(
                    "INSERT INTO products (
                        article_number, article_number_norm, name, category_id, product_type_id,
                        unit_id, description, material, color, dimensions_text, brand, barcode,
                        warranty_months, notes, cost_minor, sale_price_minor, minimum_stock,
                        track_stock, created_at, updated_at
                     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&display_article)
                .bind(&norm_article)
                .bind(&source.name)
                .bind(source.category_id)
                .bind(source.product_type_id)
                .bind(source.unit_id)
                .bind(&source.description)
                .bind(&source.material)
                .bind(&source.color)
                .bind(&source.dimensions_text)
                .bind(&source.brand)
                .bind(&source.barcode)
                .bind(source.warranty_months)
                .bind(&source.notes)
                .bind(source.cost_minor)
                .bind(source.sale_price_minor)
                .bind(source.minimum_stock)
                .bind(source.track_stock as i64)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();

                for (idx, attr) in source_attrs.iter().enumerate() {
                    sqlx::query(
                        "INSERT INTO product_attributes (product_id, attribute_name, attribute_value, sort_order)
                         VALUES (?, ?, ?, ?)",
                    )
                    .bind(product_id)
                    .bind(&attr.0)
                    .bind(&attr.1)
                    .bind(idx as i64)
                    .execute(&mut *tx)
                    .await?;
                }
                for (idx, c) in copied.iter().enumerate() {
                    sqlx::query(
                        "INSERT INTO product_images (
                            product_id, relative_path, thumbnail_path, sort_order, is_primary,
                            sha256, width, height, mime_type, created_at
                         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(product_id)
                    .bind(&c.1)
                    .bind(&c.2)
                    .bind(idx as i64)
                    .bind(if idx == 0 { 1 } else { 0 })
                    .bind(&c.0)
                    .bind(c.3)
                    .bind(c.4)
                    .bind("image/webp")
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product.duplicate".into(),
                            entity_type: Some("product".into()),
                            entity_id: Some(product_id.to_string()),
                            before_json: Some(
                                serde_json::json!({"source_product_id": source_id}).to_string(),
                            ),
                            after_json: Some(
                                serde_json::json!({"article_number": display_article})
                                    .to_string(),
                            ),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(product_id)
            })
        })
        .await;

    let product_id = match result {
        Ok(id) => id,
        Err(err) => {
            discard_generated(&state.paths.images_dir, &staged_names);
            return Err(err);
        }
    };

    get_product_inner(state, principal, product_id).await
}

type CopiedImage = (String, String, String, Option<i64>, Option<i64>);
type SourceImage = (String, String, String, Option<i64>, Option<i64>);
type ProductImageRow = (
    i64,
    String,
    String,
    i64,
    i64,
    String,
    Option<i64>,
    Option<i64>,
    String,
);

fn copy_product_images(
    images_dir: &std::path::Path,
    images: &[SourceImage],
) -> Result<Vec<CopiedImage>, AppError> {
    let mut out = Vec::with_capacity(images.len());
    for (rel, thumb_rel, sha, width, height) in images {
        let stem = uuid::Uuid::now_v7().to_string();
        let new_rel = format!("{stem}.webp");
        let new_thumb = format!("{stem}-thumb.webp");
        if safe_relative_filename(rel) && safe_relative_filename(thumb_rel) {
            std::fs::copy(images_dir.join(rel), images_dir.join(&new_rel))?;
            std::fs::copy(images_dir.join(thumb_rel), images_dir.join(&new_thumb))?;
        }
        out.push((sha.clone(), new_rel, new_thumb, *width, *height));
    }
    Ok(out)
}

fn discard_generated(images_dir: &std::path::Path, names: &[String]) {
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    if !refs.is_empty() {
        let _ = infrastructure::discard_generated_images(images_dir, &refs);
    }
}

pub async fn get_product(
    state: &AppState,
    principal: &Principal,
    product_id: i64,
) -> Result<ProductDetailDto, AppError> {
    get_product_inner(state, principal, product_id).await
}

async fn get_product_inner(
    state: &AppState,
    principal: &Principal,
    product_id: i64,
) -> Result<ProductDetailDto, AppError> {
    let can_view_cost = principal
        .permissions
        .iter()
        .any(|p| p == "product.cost.view");
    let images_dir = state.paths.images_dir.clone();
    let row = sqlx::query(
        "SELECT p.article_number, p.name, p.category_id, c.name, p.product_type_id, t.name,
                p.unit_id, u.name, p.description, p.material, p.color, p.dimensions_text,
                p.brand, p.barcode, p.warranty_months, p.notes, p.cost_minor, p.sale_price_minor,
                p.minimum_stock, p.track_stock, p.is_active, p.archived_at, p.created_at, p.updated_at
         FROM products p
         JOIN categories c ON c.id = p.category_id
         LEFT JOIN product_types t ON t.id = p.product_type_id
         LEFT JOIN units u ON u.id = p.unit_id
         WHERE p.id = ?",
    )
    .bind(product_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(r) = row else {
        return Err(AppError::NotFound(format!("product {product_id}")));
    };

    let images: Vec<ProductImageRow> =
        sqlx::query_as(
            "SELECT id, relative_path, thumbnail_path, sort_order, is_primary, sha256, width, height, mime_type
             FROM product_images WHERE product_id = ? ORDER BY sort_order, id",
        )
        .bind(product_id)
        .fetch_all(&state.pool)
        .await?;
    let attributes: Vec<(String, String)> = sqlx::query_as(
        "SELECT attribute_name, attribute_value FROM product_attributes
         WHERE product_id = ? ORDER BY sort_order, id",
    )
    .bind(product_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(ProductDetailDto {
        id: product_id,
        article_number: r.try_get(0)?,
        name: r.try_get(1)?,
        category_id: r.try_get(2)?,
        category: r.try_get(3)?,
        product_type_id: r.try_get(4)?,
        product_type: r.try_get(5)?,
        unit_id: r.try_get(6)?,
        unit: r.try_get(7)?,
        description: r.try_get(8)?,
        material: r.try_get(9)?,
        color: r.try_get(10)?,
        dimensions_text: r.try_get(11)?,
        brand: r.try_get(12)?,
        barcode: r.try_get(13)?,
        warranty_months: r.try_get(14)?,
        notes: r.try_get(15)?,
        cost_minor: if can_view_cost {
            Some(r.try_get(16)?)
        } else {
            None
        },
        sale_price_minor: r.try_get(17)?,
        minimum_stock: r.try_get(18)?,
        track_stock: r.try_get(19)?,
        is_active: r.try_get(20)?,
        archived_at: r.try_get(21)?,
        created_at: r.try_get(22)?,
        updated_at: r.try_get(23)?,
        images: images
            .into_iter()
            .map(
                |(id, rel, thumb, sort, primary, sha, width, height, mime)| ProductImageDto {
                    id,
                    image_path: abs_path(&images_dir, &rel),
                    thumbnail_path: abs_path(&images_dir, &thumb),
                    relative_path: rel,
                    thumbnail_relative_path: thumb,
                    sort_order: sort,
                    is_primary: primary != 0,
                    sha256: sha,
                    width,
                    height,
                    mime_type: mime,
                },
            )
            .collect(),
        attributes: attributes
            .into_iter()
            .map(|(n, v)| AttributeInputDto { name: n, value: v })
            .collect(),
    })
}

fn abs_path(images_dir: &std::path::Path, relative: &str) -> String {
    if safe_relative_filename(relative) {
        images_dir.join(relative).to_string_lossy().into_owned()
    } else {
        String::new()
    }
}

/// Serve a stored product image to the current session as a `data:` URL.
///
/// Any authenticated session may view product images; the path must resolve
/// inside the approved images directory (symlinks are resolved before the
/// containment check).
pub async fn read_product_image(
    state: &AppState,
    principal: &Principal,
    path: &str,
) -> Result<String, AppError> {
    let _ = principal;
    const MAX_BYTES: u64 = 25 * 1024 * 1024;
    let base = std::fs::canonicalize(&state.paths.images_dir)
        .map_err(|_| AppError::Internal("images directory missing".into()))?;
    let candidate =
        std::fs::canonicalize(path).map_err(|_| AppError::NotFound(format!("image {path}")))?;
    if !candidate.is_file() || !candidate.starts_with(&base) {
        return Err(AppError::Validation(
            "path escapes the approved images directory".into(),
        ));
    }
    let meta = std::fs::metadata(&candidate)?;
    if meta.len() > MAX_BYTES {
        return Err(AppError::Validation("image file too large to serve".into()));
    }
    let bytes = tokio::fs::read(&candidate).await?;
    let mime = infrastructure::base64::image_mime(&bytes);
    Ok(format!(
        "data:{mime};base64,{}",
        infrastructure::base64::encode(&bytes)
    ))
}

#[allow(clippy::too_many_arguments)]
pub async fn list_products(
    state: &AppState,
    principal: &Principal,
    scope: Option<String>,
    q: Option<String>,
    category_id: Option<i64>,
    product_type_id: Option<i64>,
    price_min: Option<i64>,
    price_max: Option<i64>,
    attribute_q: Option<String>,
) -> Result<Vec<ProductListItemDto>, AppError> {
    let can_view_cost = principal
        .permissions
        .iter()
        .any(|p| p == "product.cost.view");
    let images_dir = state.paths.images_dir.clone();

    let scope = scope
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("active");
    let scope_sql = match scope {
        "archived" => "p.archived_at IS NOT NULL",
        "all" => "1 = 1",
        _ => "p.archived_at IS NULL",
    };

    let qraw = q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_uppercase()
        });
    let attr_raw = attribute_q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_uppercase());

    let select = format!(
        "SELECT p.id, p.article_number, p.name, p.category_id, c.name,
                p.product_type_id, t.name, u.name, p.sale_price_minor, {},
                p.minimum_stock, p.track_stock, p.is_active, p.archived_at, pi.thumbnail_path
         FROM products p
         JOIN categories c ON c.id = p.category_id
         LEFT JOIN product_types t ON t.id = p.product_type_id
         LEFT JOIN units u ON u.id = p.unit_id
         LEFT JOIN product_images pi ON pi.product_id = p.id AND pi.is_primary = 1
         WHERE {scope_sql}
         AND (?1 IS NULL OR p.category_id IN (WITH RECURSIVE tree(id) AS (
             SELECT ?1 UNION ALL
             SELECT c.id FROM categories c JOIN tree ON c.parent_id = tree.id
         ) SELECT id FROM tree))
         AND (?2 IS NULL OR p.product_type_id = ?2)
         AND (?3 IS NULL OR p.sale_price_minor >= ?3)
         AND (?4 IS NULL OR p.sale_price_minor <= ?4)
         AND (?5 IS NULL OR EXISTS (
             SELECT 1 FROM product_attributes pa
             WHERE pa.product_id = p.id
               AND (UPPER(pa.attribute_name) LIKE ?6 OR UPPER(pa.attribute_value) LIKE ?6)
         ))",
        if can_view_cost {
            "p.cost_minor"
        } else {
            "NULL"
        }
    );

    let search_sql = "\n AND (?7 IS NULL OR p.article_number_norm = ?7
            OR p.article_number_norm LIKE ?8
            OR UPPER(p.name) LIKE ?8
            OR UPPER(p.name) LIKE ?9
            OR UPPER(c.name) LIKE ?9
            OR (t.name IS NOT NULL AND UPPER(t.name) LIKE ?9)
            OR EXISTS (
                SELECT 1 FROM product_attributes sa
                WHERE sa.product_id = p.id AND UPPER(sa.attribute_value) LIKE ?9
            ))";

    let order_sql = "\n ORDER BY CASE
         WHEN ?7 IS NULL THEN 0
         WHEN p.article_number_norm = ?7 THEN 1
         WHEN p.article_number_norm LIKE ?8 THEN 2
         WHEN UPPER(p.name) LIKE ?8 THEN 3
         WHEN UPPER(p.name) LIKE ?9 THEN 4
         ELSE 5
     END, p.name COLLATE NOCASE, p.article_number";

    let qn = qraw.clone().unwrap_or_default();
    let has_search = qraw.is_some();
    let prefix = format!("{qn}%");
    let contains = format!("%{qn}%");
    let attr_q_some = attr_raw.is_some();
    let attr_contains = format!("%{}%", attr_raw.clone().unwrap_or_default());

    let sql = if has_search {
        format!("{select}{search_sql}{order_sql} LIMIT {SEARCH_LIMIT}")
    } else {
        format!("{select}{order_sql}")
    };

    let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(&sql)
        .bind(category_id)
        .bind(product_type_id)
        .bind(price_min)
        .bind(price_max)
        .bind(if attr_q_some { attr_raw } else { None })
        .bind(if attr_q_some {
            Some(attr_contains)
        } else {
            None
        })
        .bind(qraw)
        .bind(if has_search { Some(prefix) } else { None })
        .bind(if has_search { Some(contains) } else { None })
        .fetch_all(&state.pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let thumb: Option<String> = r
                .try_get::<Option<String>, _>(14)
                .unwrap_or_default()
                .filter(|s| !s.is_empty())
                .and_then(|rel| {
                    if safe_relative_filename(&rel) {
                        Some(images_dir.join(rel).to_string_lossy().into_owned())
                    } else {
                        None
                    }
                });
            ProductListItemDto {
                id: r.try_get(0).unwrap_or_default(),
                article_number: r.try_get(1).unwrap_or_default(),
                name: r.try_get(2).unwrap_or_default(),
                category_id: r.try_get(3).unwrap_or_default(),
                category: r.try_get(4).unwrap_or_default(),
                product_type_id: r.try_get(5).unwrap_or_default(),
                product_type: r.try_get(6).unwrap_or_default(),
                unit: r.try_get(7).unwrap_or_default(),
                sale_price_minor: r.try_get(8).unwrap_or_default(),
                cost_minor: if can_view_cost {
                    r.try_get(9).ok()
                } else {
                    None
                },
                minimum_stock: r.try_get(10).unwrap_or_default(),
                track_stock: r.try_get(11).unwrap_or_default(),
                is_active: r.try_get(12).unwrap_or_default(),
                archived_at: r.try_get(13).unwrap_or_default(),
                primary_thumbnail_path: thumb,
            }
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Image lifecycle (staged writes with cleanup on failure)
// ---------------------------------------------------------------------------

pub async fn add_product_image(
    state: &AppState,
    principal: &Principal,
    product_id: i64,
    source_path: &str,
    correlation_id: &str,
) -> Result<ProductDetailDto, AppError> {
    principal.require("product.create")?;
    if source_path.trim().is_empty() {
        return Err(AppError::Validation("no file selected".into()));
    }

    let images_dir = state.paths.images_dir.clone();
    let source = PathBuf::from(source_path);
    let imported =
        tokio::task::spawn_blocking(move || infrastructure::import_image(&source, &images_dir))
            .await
            .map_err(|e| AppError::Internal(format!("background task failed: {e}")))??;

    let staged_names = vec![
        imported.stored_name.clone(),
        imported.thumbnail_name.clone(),
    ];
    let now = state.clock.now_iso();
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM products WHERE id = ?")
                    .bind(product_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                if exists.is_none() {
                    return Err(AppError::NotFound(format!("product {product_id}")));
                }
                let count: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM product_images WHERE product_id = ?")
                        .bind(product_id)
                        .fetch_one(&mut *tx)
                        .await?;
                if count >= infrastructure::MAX_PRODUCT_IMAGES {
                    return Err(AppError::Validation(format!(
                        "at most {} images per product",
                        infrastructure::MAX_PRODUCT_IMAGES
                    )));
                }

                sqlx::query(
                    "INSERT INTO product_images (
                        product_id, relative_path, thumbnail_path, sort_order, is_primary,
                        sha256, width, height, mime_type, created_at
                     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(product_id)
                .bind(&imported.stored_name)
                .bind(&imported.thumbnail_name)
                .bind(count)
                .bind(if count == 0 { 1 } else { 0 })
                .bind(&imported.sha256)
                .bind(imported.width as i64)
                .bind(imported.height as i64)
                .bind(&imported.mime_type)
                .bind(&now)
                .execute(&mut *tx)
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product.image.add".into(),
                            entity_type: Some("product".into()),
                            entity_id: Some(product_id.to_string()),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await;

    if let Err(err) = result {
        discard_generated(&state.paths.images_dir, &staged_names);
        return Err(err);
    }

    get_product_inner(state, principal, product_id).await
}

pub async fn remove_product_image(
    state: &AppState,
    principal: &Principal,
    product_id: i64,
    image_id: i64,
    correlation_id: &str,
) -> Result<ProductDetailDto, AppError> {
    principal.require("product.create")?;
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();
    let removed_files: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let removed_files_arc = removed_files.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            let removed_files_arc = removed_files_arc.clone();
            Box::pin(async move {
                let before: Option<(String, String, i64)> = sqlx::query_as(
                    "SELECT relative_path, thumbnail_path, is_primary FROM product_images
                     WHERE id = ? AND product_id = ?",
                )
                .bind(image_id)
                .bind(product_id)
                .fetch_optional(&mut *tx)
                .await?;
                let Some((rel, thumb_rel, is_primary)) = before else {
                    return Err(AppError::NotFound(format!("image {image_id}")));
                };

                sqlx::query("DELETE FROM product_images WHERE id = ?")
                    .bind(image_id)
                    .execute(&mut *tx)
                    .await?;

                if is_primary != 0 {
                    sqlx::query(
                        "UPDATE product_images SET is_primary = 1
                         WHERE product_id = ? AND id = (
                             SELECT id FROM product_images WHERE product_id = ?
                             ORDER BY sort_order, id LIMIT 1
                         )",
                    )
                    .bind(product_id)
                    .bind(product_id)
                    .execute(&mut *tx)
                    .await?;
                }

                // The files are removed AFTER the transaction commits below so a
                // rolled-back delete never loses a referenced image.
                if let Ok(mut files) = removed_files_arc.lock() {
                    files.push(rel.clone());
                    files.push(thumb_rel.clone());
                }

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product.image.remove".into(),
                            entity_type: Some("product".into()),
                            entity_id: Some(product_id.to_string()),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await?;

    if let Ok(removed_files) = std::sync::Arc::try_unwrap(removed_files) {
        if let Ok(files) = removed_files.into_inner() {
            let refs: Vec<&str> = files.iter().map(String::as_str).collect();
            if !refs.is_empty() {
                let _ = infrastructure::discard_generated_images(&state.paths.images_dir, &refs);
            }
        }
    }

    get_product_inner(state, principal, product_id).await
}

pub async fn set_primary_product_image(
    state: &AppState,
    principal: &Principal,
    product_id: i64,
    image_id: i64,
    correlation_id: &str,
) -> Result<ProductDetailDto, AppError> {
    principal.require("product.create")?;
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let belongs: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM product_images WHERE id = ? AND product_id = ?",
                )
                .bind(image_id)
                .bind(product_id)
                .fetch_optional(&mut *tx)
                .await?;
                if belongs.is_none() {
                    return Err(AppError::NotFound(format!("image {image_id}")));
                }

                sqlx::query("UPDATE product_images SET is_primary = 0 WHERE product_id = ?")
                    .bind(product_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "UPDATE product_images SET is_primary = 1, sort_order = 0 WHERE id = ?",
                )
                .bind(image_id)
                .execute(&mut *tx)
                .await?;

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product.image.primary".into(),
                            entity_type: Some("product".into()),
                            entity_id: Some(product_id.to_string()),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await?;

    get_product_inner(state, principal, product_id).await
}

pub async fn reorder_product_images(
    state: &AppState,
    principal: &Principal,
    product_id: i64,
    ordered_ids: Vec<i64>,
    correlation_id: &str,
) -> Result<ProductDetailDto, AppError> {
    principal.require("product.create")?;
    let actor_id = principal.user_id;
    let actor_session = principal.session_id.clone();
    let correlation = correlation_id.to_string();
    let audits = state.audits.clone();

    state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                let count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM product_images WHERE product_id = ?",
                )
                .bind(product_id)
                .fetch_one(&mut *tx)
                .await?;
                if ordered_ids.len() as i64 != count {
                    return Err(AppError::Validation(
                        "image order list does not match the product images".into(),
                    ));
                }
                for id in &ordered_ids {
                    let ok: Option<i64> = sqlx::query_scalar(
                        "SELECT 1 FROM product_images WHERE id = ? AND product_id = ?",
                    )
                    .bind(id)
                    .bind(product_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if ok.is_none() {
                        return Err(AppError::Validation(
                            "image order list contains an image that does not belong to this product".into(),
                        ));
                    }
                }
                for (idx, id) in ordered_ids.iter().enumerate() {
                    let primary = if idx == 0 { 1 } else { 0 };
                    sqlx::query(
                        "UPDATE product_images SET sort_order = ?, is_primary = ? WHERE id = ?",
                    )
                    .bind(idx as i64)
                    .bind(primary)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                }

                audits
                    .record(
                        &mut *tx,
                        AuditInput {
                            user_id: Some(actor_id),
                            session_id: Some(actor_session.clone()),
                            action: "product.image.reorder".into(),
                            entity_type: Some("product".into()),
                            entity_id: Some(product_id.to_string()),
                            correlation_id: Some(correlation),
                            ..Default::default()
                        },
                    )
                    .await?;
                Ok(())
            })
        })
        .await?;

    get_product_inner(state, principal, product_id).await
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::auth::{resolve_session, Principal};
    use crate::application::catalogue::{
        archive_category, create_category, create_product_type, list_categories,
        list_product_types, list_units,
    };
    use crate::dto::catalogue::{AttributeInputDto, CreateProductInput, UpdateProductInput};

    struct Harness {
        dir: std::path::PathBuf,
        state: AppState,
        owner: Principal,
        clerk: Principal,
    }

    async fn seed_user(state: &AppState, username: &str, full_name: &str, role: &str) -> i64 {
        let hash = crate::infrastructure::password::hash_password("Passw0rd! 1").unwrap();
        let id: i64 =
            sqlx::query("INSERT INTO users (username, password_hash, full_name) VALUES (?, ?, ?)")
                .bind(username)
                .bind(&hash)
                .bind(full_name)
                .execute(&state.pool)
                .await
                .unwrap()
                .last_insert_rowid();
        let role_id: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE code = ?")
            .bind(role)
            .fetch_one(&state.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
            .bind(id)
            .bind(role_id)
            .execute(&state.pool)
            .await
            .unwrap();
        id
    }

    async fn principal_for(state: &AppState, user_id: i64) -> Principal {
        let session = state.sessions.create(user_id).await.unwrap();
        resolve_session(state, &session.id).await.unwrap()
    }

    async fn setup() -> Harness {
        let dir =
            std::env::temp_dir().join(format!("furniture-shop-catalogue-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).unwrap();
        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let state = AppState::new(pool, paths);
        let owner_id = seed_user(&state, "owner", "Owner", "owner").await;
        let clerk_id = seed_user(&state, "clerk", "Clerk", "salesperson").await;
        let owner = principal_for(&state, owner_id).await;
        let clerk = principal_for(&state, clerk_id).await;
        Harness {
            dir,
            state,
            owner,
            clerk,
        }
    }

    fn make_png(dir: &std::path::Path, name: &str) -> String {
        let path = dir.join(name);
        let img = image::DynamicImage::new_rgba8(64, 48);
        img.save(&path).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn make_jpeg(dir: &std::path::Path, name: &str) -> String {
        let path = dir.join(name);
        let img = image::RgbImage::from_pixel(16, 16, image::Rgb([200, 80, 20]));
        img.save_with_format(&path, image::ImageFormat::Jpeg)
            .unwrap();
        path.to_string_lossy().into_owned()
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for (i, slot) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB88320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *slot = c;
        }
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc ^ 0xFFFF_FFFF
    }

    /// Head-only PNG: signature plus a valid IHDR chunk. `into_dimensions`
    /// reads dimensions from the header without touching pixel data, so this
    /// lets the pipeline's declared-size budget be exercised without decoding
    /// (and without allocating) a huge image.
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        let mut data = Vec::new();
        data.extend_from_slice(&width.to_be_bytes());
        data.extend_from_slice(&height.to_be_bytes());
        data.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit truecolor, no interlace
        let mut crc_input = Vec::new();
        crc_input.extend_from_slice(b"IHDR");
        crc_input.extend_from_slice(&data);
        out.extend_from_slice(&13u32.to_be_bytes());
        out.extend_from_slice(b"IHDR");
        out.extend_from_slice(&data);
        out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
        out
    }

    fn base_input(category_id: i64) -> CreateProductInput {
        CreateProductInput {
            article_number: "A-100".into(),
            name: "Test Sofa".into(),
            category_id,
            product_type_id: None,
            unit_id: None,
            description: Some("A sofa".into()),
            material: Some("Velvet".into()),
            color: Some("Green".into()),
            dimensions_text: Some("200x90x80".into()),
            brand: Some("Nordfurn".into()),
            barcode: Some("890123".into()),
            warranty_months: Some(24),
            notes: Some("floor model".into()),
            cost_minor: 25_000,
            sale_price_minor: 45_000,
            minimum_stock: 2,
            track_stock: true,
            attributes: vec![AttributeInputDto {
                name: "Seats".into(),
                value: "3".into(),
            }],
            image_paths: vec![],
        }
    }

    #[tokio::test]
    async fn categories_are_sibling_unique_and_archive_safe() {
        let h = setup().await;
        let cat = create_category(&h.state, &h.owner, "Chairs", None, 1, "c1")
            .await
            .unwrap();
        assert!(cat.is_active);

        let dup = create_category(&h.state, &h.owner, "chairs", None, 2, "c2")
            .await
            .unwrap_err();
        assert!(matches!(dup, AppError::Conflict(_)));

        let nested = create_category(&h.state, &h.owner, "Office", Some(cat.id), 1, "c3")
            .await
            .unwrap();
        let other_root = create_category(&h.state, &h.owner, "Tables", None, 1, "c4")
            .await
            .unwrap();
        assert!(
            create_category(&h.state, &h.owner, "office", Some(other_root.id), 2, "c5")
                .await
                .is_ok(),
            "same display name under a different parent is allowed"
        );

        let units = list_units(&h.state, &h.owner).await.unwrap();
        let names: Vec<&str> = units.iter().map(|u| u.name.as_str()).collect();
        assert!(names.contains(&"Pcs"));
        assert!(names.contains(&"Meter"));

        archive_category(&h.state, &h.owner, nested.id, "obsolete", "c6")
            .await
            .unwrap();
        let err = create_product_type(&h.state, &h.owner, nested.id, "Ergonomic", "c7")
            .await
            .unwrap_err();
        assert!(
            matches!(err, AppError::Validation(_)),
            "archived category must reject new product types"
        );
        let cats = list_categories(&h.state, &h.owner).await.unwrap();
        assert!(!cats.iter().find(|c| c.id == nested.id).unwrap().is_active);

        assert!(h.state.audits.verify_chain().await.unwrap().is_none());
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test]
    async fn product_update_duplicate_and_cost_gating() {
        let h = setup().await;
        let cat = create_category(&h.state, &h.owner, "Sofas", None, 1, "c1")
            .await
            .unwrap();

        let created = create_product(&h.state, &h.owner, base_input(cat.id), "c-create")
            .await
            .unwrap();
        assert_eq!(created.article_number, "A-100");
        assert_eq!(created.cost_minor, Some(25_000));
        assert_eq!(created.attributes.len(), 1);

        let dup = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: " a-100 ".into(),
                ..base_input(cat.id)
            },
            "c-dup",
        )
        .await
        .unwrap_err();
        assert!(
            matches!(dup, AppError::Conflict(_)),
            "live duplicate article (case-insensitive, trimmed) must be rejected"
        );

        let updated = update_product(
            &h.state,
            &h.owner,
            created.id,
            UpdateProductInput {
                name: Some("Velvet Loveseat".into()),
                article_number: Some("a-200".into()),
                attributes: Some(vec![
                    AttributeInputDto {
                        name: "Seats".into(),
                        value: "2".into(),
                    },
                    AttributeInputDto {
                        name: "Armrests".into(),
                        value: "Wood".into(),
                    },
                ]),
                ..Default::default()
            },
            "c-update",
        )
        .await
        .unwrap();
        assert_eq!(updated.name, "Velvet Loveseat");
        assert_eq!(
            updated.article_number, "a-200",
            "display article keeps the shape the editor typed; only the unique index is normalized"
        );
        assert_eq!(updated.attributes.len(), 2);
        assert_eq!(updated.attributes[0].name, "Seats");

        let conflict = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "A-200".into(),
                ..base_input(cat.id)
            },
            "c-conflict",
        )
        .await
        .unwrap_err();
        assert!(matches!(conflict, AppError::Conflict(_)));

        let duplicated = duplicate_product(&h.state, &h.owner, created.id, "A-300", "c-dup2")
            .await
            .unwrap();
        assert_eq!(duplicated.article_number, "A-300");
        assert_eq!(duplicated.attributes.len(), 2);
        assert_eq!(duplicated.name, "Velvet Loveseat");

        let clerk_detail = get_product(&h.state, &h.clerk, created.id).await.unwrap();
        assert_eq!(
            clerk_detail.cost_minor, None,
            "salesperson has product.create but no product.cost.view"
        );
        assert_eq!(clerk_detail.sale_price_minor, 45_000);
        let clerk_list = list_products(
            &h.state,
            &h.clerk,
            None,
            Some("A-200".to_string()),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(clerk_list[0].cost_minor, None);
        let owner_detail = get_product(&h.state, &h.owner, created.id).await.unwrap();
        assert_eq!(owner_detail.cost_minor, Some(25_000));

        let forbidden = {
            let acct_id = seed_user(&h.state, "acct", "Accountant", "accountant").await;
            let acct = principal_for(&h.state, acct_id).await;
            create_product(&h.state, &acct, base_input(cat.id), "c-forbid")
                .await
                .unwrap_err()
        };
        assert!(
            matches!(forbidden, AppError::Unauthorized(_)),
            "accountant has no product.create"
        );
        assert!(h.state.audits.verify_chain().await.unwrap().is_none());
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test]
    async fn product_archive_unarchive_and_article_reuse() {
        let h = setup().await;
        let cat = create_category(&h.state, &h.owner, "Sofas", None, 1, "c1")
            .await
            .unwrap();
        let created = create_product(&h.state, &h.owner, base_input(cat.id), "c-create")
            .await
            .unwrap();

        archive_product(&h.state, &h.owner, created.id, "sold out", "c-arch")
            .await
            .unwrap();
        let archived = list_products(
            &h.state,
            &h.owner,
            Some("archived".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert!(archived.iter().any(|p| p.id == created.id));
        let active = list_products(
            &h.state,
            &h.owner,
            Some("active".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert!(!active.iter().any(|p| p.id == created.id));

        let detail = get_product(&h.state, &h.owner, created.id).await.unwrap();
        assert_eq!(detail.article_number, "A-100");
        assert_eq!(detail.name, "Test Sofa");
        assert_eq!(detail.attributes.len(), 1);
        assert!(
            detail.archived_at.is_some(),
            "archiving must hide the product from new sales but keep it readable for history"
        );

        let again = archive_product(&h.state, &h.owner, created.id, "why", "c-arch2")
            .await
            .unwrap_err();
        assert!(matches!(again, AppError::Conflict(_)));

        unarchive_product(&h.state, &h.owner, created.id, "c-unarch")
            .await
            .unwrap();
        let active = list_products(
            &h.state,
            &h.owner,
            Some("active".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert!(active.iter().any(|p| p.id == created.id));

        let dup = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "A-100".into(),
                ..base_input(cat.id)
            },
            "c-dup",
        )
        .await
        .unwrap_err();
        assert!(matches!(dup, AppError::Conflict(_)));

        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test]
    async fn list_filters_include_category_descendants() {
        let h = setup().await;
        let root = create_category(&h.state, &h.owner, "Living", None, 1, "c1")
            .await
            .unwrap();
        let child = create_category(&h.state, &h.owner, "Sofas", Some(root.id), 1, "c2")
            .await
            .unwrap();
        let grand = create_category(&h.state, &h.owner, "Corner", Some(child.id), 1, "c3")
            .await
            .unwrap();
        let other = create_category(&h.state, &h.owner, "Tables", None, 1, "c4")
            .await
            .unwrap();

        let p_root = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "LIV-1".into(),
                ..base_input(root.id)
            },
            "c1",
        )
        .await
        .unwrap();
        let p_child = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "SOF-1".into(),
                ..base_input(child.id)
            },
            "c2",
        )
        .await
        .unwrap();
        let p_grand = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "COR-1".into(),
                ..base_input(grand.id)
            },
            "c3",
        )
        .await
        .unwrap();
        let p_other = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "TBL-1".into(),
                ..base_input(other.id)
            },
            "c4",
        )
        .await
        .unwrap();

        let in_tree = list_products(
            &h.state,
            &h.owner,
            None,
            None,
            Some(root.id),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        let ids: Vec<i64> = in_tree.iter().map(|p| p.id).collect();
        assert!(ids.contains(&p_root.id));
        assert!(ids.contains(&p_child.id));
        assert!(ids.contains(&p_grand.id));
        assert!(!ids.contains(&p_other.id));

        let only_child = list_products(
            &h.state,
            &h.owner,
            None,
            None,
            Some(child.id),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            only_child.len(),
            2,
            "child bucket covers its own descendants"
        );

        let matched = list_products(
            &h.state,
            &h.owner,
            None,
            Some("corner".to_string()),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(matched[0].id, p_grand.id);
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test]
    async fn list_filters_compose_price_and_attributes_with_search() {
        let h = setup().await;
        let cat = create_category(&h.state, &h.owner, "Sofas", None, 1, "c1")
            .await
            .unwrap();
        let other_cat = create_category(&h.state, &h.owner, "Tables", None, 2, "c2")
            .await
            .unwrap();

        let cheap = create_product(&h.state, &h.owner, base_input(cat.id), "c1")
            .await
            .unwrap();
        let oak_sofa = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "A-200".into(),
                name: "Oak Sofa".into(),
                sale_price_minor: 120_000,
                attributes: vec![AttributeInputDto {
                    name: "Material".into(),
                    value: "Oak".into(),
                }],
                ..base_input(cat.id)
            },
            "c2",
        )
        .await
        .unwrap();
        let oak_table = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "T-100".into(),
                name: "Oak Table".into(),
                sale_price_minor: 60_000,
                ..base_input(other_cat.id)
            },
            "c3",
        )
        .await
        .unwrap();

        let price_only = list_products(
            &h.state,
            &h.owner,
            None,
            None,
            None,
            None,
            Some(50_000),
            Some(130_000),
            None,
        )
        .await
        .unwrap();
        let ids: Vec<i64> = price_only.iter().map(|p| p.id).collect();
        assert!(ids.contains(&oak_sofa.id));
        assert!(ids.contains(&oak_table.id));
        assert!(
            !ids.contains(&cheap.id),
            "sale price 45k is below the 50k floor"
        );

        let by_attr = list_products(
            &h.state,
            &h.owner,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("oak".into()),
        )
        .await
        .unwrap();
        let ids: Vec<i64> = by_attr.iter().map(|p| p.id).collect();
        assert!(
            !ids.contains(&cheap.id),
            "Seats=3 does not match attribute search 'oak'"
        );
        assert!(ids.contains(&oak_sofa.id));
        assert!(ids.contains(&oak_table.id));

        let combo = list_products(
            &h.state,
            &h.owner,
            None,
            Some("oak".into()),
            Some(cat.id),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        let ids: Vec<i64> = combo.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            vec![oak_sofa.id],
            "search composed with the category tree excludes the table"
        );
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test]
    async fn product_images_lifecycle_and_limits() {
        let h = setup().await;
        let cat = create_category(&h.state, &h.owner, "Sofas", None, 1, "c1")
            .await
            .unwrap();

        let img_root = h.dir.join("sources");
        std::fs::create_dir_all(&img_root).unwrap();
        let p1 = make_png(&img_root, "1.png");
        let p2 = make_png(&img_root, "2.png");
        let feedback = make_png(&img_root, "3.png");

        let created = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "IMG-1".into(),
                image_paths: vec![p1.clone(), p2.clone()],
                ..base_input(cat.id)
            },
            "c-create",
        )
        .await
        .unwrap();
        assert_eq!(created.images.len(), 2);
        assert_eq!(created.images.iter().filter(|i| i.is_primary).count(), 1);

        // Stores the generated + thumb files for both images.
        let images_dir = &h.state.paths.images_dir;
        let stored: Vec<String> = created
            .images
            .iter()
            .map(|i| i.relative_path.clone())
            .collect();
        for name in &stored {
            assert!(images_dir.join(name).exists());
        }

        let added = add_product_image(&h.state, &h.owner, created.id, &feedback, "c-add")
            .await
            .unwrap();
        assert_eq!(added.images.len(), 3);

        let ids: Vec<i64> = added.images.iter().map(|i| i.id).collect();
        let second = ids[1];
        let detail = set_primary_product_image(&h.state, &h.owner, created.id, second, "c-primary")
            .await
            .unwrap();
        let primary = detail.images.iter().find(|i| i.is_primary).unwrap();
        assert_eq!(primary.id, second);

        let reordered = [ids[2], ids[0], ids[1]];
        let detail = reorder_product_images(
            &h.state,
            &h.owner,
            created.id,
            reordered.to_vec(),
            "c-reorder",
        )
        .await
        .unwrap();
        let order: Vec<i64> = detail.images.iter().map(|i| i.id).collect();
        assert_eq!(order, reordered);
        assert!(detail.images[0].is_primary);

        // Removing an image deletes its files only after commit.
        let target = detail.images[1].clone();
        let removed = remove_product_image(&h.state, &h.owner, created.id, target.id, "c-remove")
            .await
            .unwrap();
        assert_eq!(removed.images.len(), 2);
        assert!(!images_dir.join(&target.relative_path).exists());
        assert!(!images_dir.join(&target.thumbnail_relative_path).exists());

        // Eight images is the cap; a ninth is rejected and its staged files discarded.
        let mut many = vec![p1.clone(), p2.clone(), feedback.clone()];
        for i in 0..8 - many.len() {
            many.push(make_png(&img_root, &format!("x{i}.png")));
        }
        let full = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "IMG-8".into(),
                image_paths: many.clone(),
                ..base_input(cat.id)
            },
            "c-full",
        )
        .await
        .unwrap();
        assert_eq!(full.images.len(), 8);
        let before = std::fs::read_dir(images_dir).unwrap().count();
        let ninth = make_png(&img_root, "9.png");
        let err = add_product_image(&h.state, &h.owner, full.id, &ninth, "c-over")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        wait_for_image_count(&h, before);

        assert!(h.state.audits.verify_chain().await.unwrap().is_none());
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test]
    async fn read_product_image_serves_data_url() {
        let h = setup().await;
        let cat = create_category(&h.state, &h.owner, "Sofas", None, 1, "c1")
            .await
            .unwrap();

        let img_root = h.dir.join("sources");
        std::fs::create_dir_all(&img_root).unwrap();
        let p1 = make_png(&img_root, "1.png");
        let created = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "IMG-DATA".into(),
                image_paths: vec![p1],
                ..base_input(cat.id)
            },
            "c-create",
        )
        .await
        .unwrap();
        let image = &created.images[0];

        let url = read_product_image(&h.state, &h.clerk, &image.image_path)
            .await
            .unwrap();
        assert!(
            url.starts_with("data:image/webp;base64,"),
            "unexpected url: {url}"
        );
        assert!(url.len() > "data:image/webp;base64,".len());

        let bad = read_product_image(&h.state, &h.clerk, h.dir.to_string_lossy().as_ref())
            .await
            .unwrap_err();
        assert!(matches!(bad, AppError::Validation(_)));

        let missing = h.state.paths.images_dir.join("does-not-exist.webp");
        let err = read_product_image(&h.state, &h.clerk, missing.to_string_lossy().as_ref())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));

        assert!(h.state.audits.verify_chain().await.unwrap().is_none());
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    fn wait_for_image_count(h: &Harness, expected: usize) {
        for _ in 0..100 {
            let entries = std::fs::read_dir(&h.state.paths.images_dir)
                .unwrap()
                .count();
            if entries == expected {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("staged image file count never returned to {expected}");
    }

    #[tokio::test]
    async fn staged_files_are_discarded_on_failed_create() {
        let h = setup().await;
        let img_root = h.dir.join("sources");
        std::fs::create_dir_all(&img_root).unwrap();
        let p1 = make_png(&img_root, "a.png");

        let err = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "FAIL-1".into(),
                category_id: 999_999,
                image_paths: vec![p1],
                ..base_input(999_999)
            },
            "c-fail",
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));

        let remaining = std::fs::read_dir(&h.state.paths.images_dir)
            .unwrap()
            .count();
        assert_eq!(
            remaining, 0,
            "staged files must not remain after a failed create"
        );
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test]
    async fn product_types_follow_category_rules() {
        let h = setup().await;
        let cat = create_category(&h.state, &h.owner, "Beds", None, 1, "c1")
            .await
            .unwrap();
        let other = create_category(&h.state, &h.owner, "Cabinets", None, 1, "c2")
            .await
            .unwrap();
        create_product_type(&h.state, &h.owner, cat.id, "King", "c3")
            .await
            .unwrap();

        let dup = create_product_type(&h.state, &h.owner, cat.id, "king", "c4")
            .await
            .unwrap_err();
        assert!(matches!(dup, AppError::Conflict(_)));

        create_product_type(&h.state, &h.owner, other.id, "King", "c5")
            .await
            .unwrap();
        let types = list_product_types(&h.state, &h.owner, Some(cat.id))
            .await
            .unwrap();
        assert_eq!(types.len(), 1);
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test]
    async fn duplicate_articles_are_case_and_space_normalized() {
        let h = setup().await;
        let cat = create_category(&h.state, &h.owner, "Sofas", None, 1, "c1")
            .await
            .unwrap();
        create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "C-100 X".into(),
                ..base_input(cat.id)
            },
            "c1",
        )
        .await
        .unwrap();

        for variant in ["c-100   x", "  C-100 X  ", "C-100  x"] {
            let err = create_product(
                &h.state,
                &h.owner,
                CreateProductInput {
                    article_number: variant.into(),
                    ..base_input(cat.id)
                },
                "c-dup",
            )
            .await
            .unwrap_err();
            assert!(
                matches!(err, AppError::Conflict(_)),
                "variant {variant:?} must be a normalized duplicate of C-100 X: {err:?}"
            );
        }

        let distinct = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "C-100Y".into(),
                ..base_input(cat.id)
            },
            "c-ok",
        )
        .await
        .unwrap();
        assert_eq!(distinct.article_number, "C-100Y");
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test]
    async fn search_ranks_exact_article_first() {
        let h = setup().await;
        let cat = create_category(&h.state, &h.owner, "Sofas", None, 1, "c1")
            .await
            .unwrap();
        let exact = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "A-100".into(),
                ..base_input(cat.id)
            },
            "c1",
        )
        .await
        .unwrap();
        create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "A-1000".into(),
                name: "Extended Ottoman".into(),
                ..base_input(cat.id)
            },
            "c2",
        )
        .await
        .unwrap();
        create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "A-100-X".into(),
                name: "Corner Unit".into(),
                ..base_input(cat.id)
            },
            "c3",
        )
        .await
        .unwrap();
        let name_match = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "Z-001".into(),
                name: "Deluxe A-100 Frame".into(),
                ..base_input(cat.id)
            },
            "c4",
        )
        .await
        .unwrap();

        let results = list_products(
            &h.state,
            &h.owner,
            None,
            Some("a-100".to_string()),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            results[0].id,
            exact.id,
            "exact article match must rank first: {}",
            results
                .iter()
                .take(4)
                .map(|p| p.article_number.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let ids: Vec<i64> = results.iter().map(|p| p.id).collect();
        assert!(
            ids.contains(&name_match.id),
            "name-contains match is included"
        );
        assert!(
            results.iter().any(|p| p.article_number == "A-1000")
                && results.iter().any(|p| p.article_number == "A-100-X"),
            "prefix matches are included"
        );
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test]
    async fn invalid_corrupt_renamed_and_oversized_images_are_rejected_or_sniffed() {
        let h = setup().await;
        let cat = create_category(&h.state, &h.owner, "Sofas", None, 1, "c1")
            .await
            .unwrap();
        let img_root = h.dir.join("sources");
        std::fs::create_dir_all(&img_root).unwrap();

        // Garbage wearing a .png name: content must be sniffed, not trusted.
        let corrupt = img_root.join("corrupt.png");
        std::fs::write(&corrupt, b"this is not an image").unwrap();
        let err = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "BAD-1".into(),
                image_paths: vec![corrupt.to_string_lossy().into_owned()],
                ..base_input(cat.id)
            },
            "c-bad1",
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err, AppError::Image(_)),
            "garbage bytes must be rejected: {err:?}"
        );
        assert_eq!(
            std::fs::read_dir(&h.state.paths.images_dir)
                .unwrap()
                .count(),
            0,
            "failed image import leaves no orphan files"
        );

        // A valid JPEG renamed to .png is decoded from its bytes, not its name.
        let jpeg = make_jpeg(&img_root, "photo.png");
        let ok = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "OK-1".into(),
                image_paths: vec![jpeg],
                ..base_input(cat.id)
            },
            "c-ok",
        )
        .await
        .unwrap();
        assert_eq!(ok.images.len(), 1);
        assert!(ok.images[0].mime_type.starts_with("image/webp"));
        assert!(
            h.state
                .paths
                .images_dir
                .join(&ok.images[0].relative_path)
                .is_file(),
            "stored original must exist under its generated name"
        );

        // Oversized: PNG header declares far more pixels than the budget.
        let oversized = img_root.join("huge.png");
        std::fs::write(&oversized, png_header(20_002, 2_000)).unwrap();
        let err = create_product(
            &h.state,
            &h.owner,
            CreateProductInput {
                article_number: "BIG-1".into(),
                image_paths: vec![oversized.to_string_lossy().into_owned()],
                ..base_input(cat.id)
            },
            "c-big",
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err, AppError::Image(_)),
            "declared pixel budget overflow must be rejected: {err:?}"
        );

        assert!(h.state.audits.verify_chain().await.unwrap().is_none());
        let _ = std::fs::remove_dir_all(&h.dir);
    }
}
