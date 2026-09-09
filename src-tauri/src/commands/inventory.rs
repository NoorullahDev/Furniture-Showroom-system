use tauri::State;

use crate::application;
use crate::commands::authenticated;
use crate::commands::wrapper::{run_command, run_command_with_correlation};
use crate::dto::inventory::{
    AdjustStockInput, DamageStockInput, LocationDto, PostStockInput, ReleaseStockInput,
    StockBalanceDto, StockMovementDto, TransferStockInput, ValuationLineDto,
};
use crate::dto::AppErrorDto;
use crate::error::new_correlation_id;
use crate::state::AppState;

#[tauri::command]
pub async fn location_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<LocationDto>, AppErrorDto> {
    run_command("location_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::inventory::list_locations(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn stock_balance_list(
    state: State<'_, AppState>,
    session: String,
    location_id: Option<i64>,
) -> Result<Vec<StockBalanceDto>, AppErrorDto> {
    run_command("stock_balance_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::inventory::list_balances(&state, &principal, location_id).await
    })
    .await
}

#[tauri::command]
pub async fn stock_movement_list(
    state: State<'_, AppState>,
    session: String,
    product_id: Option<i64>,
    location_id: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<StockMovementDto>, AppErrorDto> {
    run_command("stock_movement_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::inventory::list_movements(&state, &principal, product_id, location_id, limit)
            .await
    })
    .await
}

#[tauri::command]
pub async fn stock_valuation(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<ValuationLineDto>, AppErrorDto> {
    run_command("stock_valuation", async move {
        let principal = authenticated(&state, &session).await?;
        application::inventory::valuation(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn stock_opening(
    state: State<'_, AppState>,
    session: String,
    input: PostStockInput,
) -> Result<StockMovementDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("stock_opening", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::inventory::post_opening(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn stock_transfer(
    state: State<'_, AppState>,
    session: String,
    input: TransferStockInput,
) -> Result<Vec<StockMovementDto>, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("stock_transfer", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::inventory::post_transfer(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn stock_adjust(
    state: State<'_, AppState>,
    session: String,
    input: AdjustStockInput,
) -> Result<StockMovementDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("stock_adjust", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::inventory::post_adjust(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn stock_damage(
    state: State<'_, AppState>,
    session: String,
    input: DamageStockInput,
) -> Result<StockMovementDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("stock_damage", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::inventory::post_damage(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn stock_repair(
    state: State<'_, AppState>,
    session: String,
    input: DamageStockInput,
) -> Result<StockMovementDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("stock_repair", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::inventory::post_repair(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn stock_reserve(
    state: State<'_, AppState>,
    session: String,
    input: ReleaseStockInput,
) -> Result<StockMovementDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("stock_reserve", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::inventory::post_reserve(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn stock_release(
    state: State<'_, AppState>,
    session: String,
    input: ReleaseStockInput,
) -> Result<StockMovementDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("stock_release", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::inventory::post_release(&state, &principal, input, &correlation_id).await
    })
    .await
}
