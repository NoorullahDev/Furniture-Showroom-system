use tauri::State;

use crate::application;
use crate::commands::authenticated;
use crate::commands::wrapper::{run_command, run_command_with_correlation};
use crate::dto::catalogue::{CategoryDto, ProductTypeDto, UnitDto};
use crate::dto::AppErrorDto;
use crate::error::new_correlation_id;
use crate::state::AppState;

#[tauri::command]
pub async fn category_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<CategoryDto>, AppErrorDto> {
    run_command("category_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::catalogue::list_categories(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn category_create(
    state: State<'_, AppState>,
    session: String,
    name: String,
    parent_id: Option<i64>,
    sort_order: Option<i64>,
) -> Result<CategoryDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("category_create", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::catalogue::create_category(
            &state,
            &principal,
            &name,
            parent_id,
            sort_order.unwrap_or(0),
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn category_update(
    state: State<'_, AppState>,
    session: String,
    category_id: i64,
    name: Option<String>,
    parent_id: Option<Option<i64>>,
    sort_order: Option<i64>,
    is_active: Option<bool>,
) -> Result<CategoryDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("category_update", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::catalogue::update_category(
            &state,
            &principal,
            category_id,
            name,
            parent_id,
            sort_order,
            is_active,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn category_archive(
    state: State<'_, AppState>,
    session: String,
    category_id: i64,
    reason: Option<String>,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("category_archive", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::catalogue::archive_category(
            &state,
            &principal,
            category_id,
            &reason.unwrap_or_default(),
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn product_type_list(
    state: State<'_, AppState>,
    session: String,
    category_id: Option<i64>,
) -> Result<Vec<ProductTypeDto>, AppErrorDto> {
    run_command("product_type_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::catalogue::list_product_types(&state, &principal, category_id).await
    })
    .await
}

#[tauri::command]
pub async fn product_type_create(
    state: State<'_, AppState>,
    session: String,
    category_id: i64,
    name: String,
) -> Result<ProductTypeDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("product_type_create", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::catalogue::create_product_type(
            &state,
            &principal,
            category_id,
            &name,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn product_type_update(
    state: State<'_, AppState>,
    session: String,
    product_type_id: i64,
    name: Option<String>,
    is_active: Option<bool>,
) -> Result<ProductTypeDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("product_type_update", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::catalogue::update_product_type(
            &state,
            &principal,
            product_type_id,
            name,
            is_active,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn product_type_archive(
    state: State<'_, AppState>,
    session: String,
    product_type_id: i64,
    reason: Option<String>,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("product_type_archive", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::catalogue::archive_product_type(
            &state,
            &principal,
            product_type_id,
            &reason.unwrap_or_default(),
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn unit_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<UnitDto>, AppErrorDto> {
    run_command("unit_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::catalogue::list_units(&state, &principal).await
    })
    .await
}
