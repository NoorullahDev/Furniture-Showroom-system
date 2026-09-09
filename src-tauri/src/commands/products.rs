use tauri::State;

use crate::application;
use crate::commands::authenticated;
use crate::commands::wrapper::{run_command, run_command_with_correlation};
use crate::dto::catalogue::{
    CreateProductInput, ProductDetailDto, ProductListItemDto, UpdateProductInput,
};
use crate::dto::AppErrorDto;
use crate::error::new_correlation_id;
use crate::state::AppState;

#[tauri::command]
pub async fn product_create(
    state: State<'_, AppState>,
    session: String,
    input: CreateProductInput,
) -> Result<ProductDetailDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("product_create", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::products::create_product(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn product_update(
    state: State<'_, AppState>,
    session: String,
    product_id: i64,
    input: UpdateProductInput,
) -> Result<ProductDetailDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("product_update", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::products::update_product(
            &state,
            &principal,
            product_id,
            input,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn product_archive(
    state: State<'_, AppState>,
    session: String,
    product_id: i64,
    reason: Option<String>,
) -> Result<(), AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("product_archive", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::products::archive_product(
            &state,
            &principal,
            product_id,
            &reason.unwrap_or_default(),
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn product_unarchive(
    state: State<'_, AppState>,
    session: String,
    product_id: i64,
) -> Result<ProductDetailDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("product_unarchive", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::products::unarchive_product(&state, &principal, product_id, &correlation_id)
            .await
    })
    .await
}

#[tauri::command]
pub async fn product_duplicate(
    state: State<'_, AppState>,
    session: String,
    product_id: i64,
    new_article: String,
) -> Result<ProductDetailDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("product_duplicate", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::products::duplicate_product(
            &state,
            &principal,
            product_id,
            &new_article,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn product_get(
    state: State<'_, AppState>,
    session: String,
    product_id: i64,
) -> Result<ProductDetailDto, AppErrorDto> {
    run_command("product_get", async move {
        let principal = authenticated(&state, &session).await?;
        application::products::get_product(&state, &principal, product_id).await
    })
    .await
}

#[tauri::command]
pub async fn product_image_data(
    state: State<'_, AppState>,
    session: String,
    path: String,
) -> Result<String, AppErrorDto> {
    run_command("product_image_data", async move {
        let principal = authenticated(&state, &session).await?;
        application::products::read_product_image(&state, &principal, &path).await
    })
    .await
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn product_list(
    state: State<'_, AppState>,
    session: String,
    scope: Option<String>,
    q: Option<String>,
    category_id: Option<i64>,
    product_type_id: Option<i64>,
    price_min: Option<i64>,
    price_max: Option<i64>,
    attribute_q: Option<String>,
) -> Result<Vec<ProductListItemDto>, AppErrorDto> {
    run_command("product_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::products::list_products(
            &state,
            &principal,
            scope,
            q,
            category_id,
            product_type_id,
            price_min,
            price_max,
            attribute_q,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn product_image_add(
    state: State<'_, AppState>,
    session: String,
    product_id: i64,
    path: String,
) -> Result<ProductDetailDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("product_image_add", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::products::add_product_image(
            &state,
            &principal,
            product_id,
            &path,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn product_image_remove(
    state: State<'_, AppState>,
    session: String,
    product_id: i64,
    image_id: i64,
) -> Result<ProductDetailDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("product_image_remove", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::products::remove_product_image(
            &state,
            &principal,
            product_id,
            image_id,
            &correlation_id,
        )
        .await
    })
    .await
}

#[tauri::command]
pub async fn product_image_set_primary(
    state: State<'_, AppState>,
    session: String,
    product_id: i64,
    image_id: i64,
) -> Result<ProductDetailDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation(
        "product_image_set_primary",
        correlation_id.clone(),
        async move {
            let principal = authenticated(&state, &session).await?;
            application::products::set_primary_product_image(
                &state,
                &principal,
                product_id,
                image_id,
                &correlation_id,
            )
            .await
        },
    )
    .await
}

#[tauri::command]
pub async fn product_image_reorder(
    state: State<'_, AppState>,
    session: String,
    product_id: i64,
    ordered_ids: Vec<i64>,
) -> Result<ProductDetailDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation(
        "product_image_reorder",
        correlation_id.clone(),
        async move {
            let principal = authenticated(&state, &session).await?;
            application::products::reorder_product_images(
                &state,
                &principal,
                product_id,
                ordered_ids,
                &correlation_id,
            )
            .await
        },
    )
    .await
}
