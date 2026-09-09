use tauri::State;

use crate::application;
use crate::commands::authenticated;
use crate::commands::wrapper::{run_command, run_command_with_correlation};
use crate::dto::purchases::{
    PayableAgingRowDto, PurchaseCreateInput, PurchaseDto, PurchasePostInput, SupplierPaymentDto,
    SupplierPaymentInput, SupplierPaymentVoidInput, SupplierReturnCreateInput, SupplierReturnDto,
    SupplierReturnPostInput,
};
use crate::dto::AppErrorDto;
use crate::error::new_correlation_id;
use crate::state::AppState;

#[tauri::command]
pub async fn purchase_create(
    state: State<'_, AppState>,
    session: String,
    input: PurchaseCreateInput,
) -> Result<PurchaseDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("purchase_create", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::purchases::create_purchase(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn purchase_post(
    state: State<'_, AppState>,
    session: String,
    input: PurchasePostInput,
) -> Result<PurchaseDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("purchase_post", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::purchases::post_purchase(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn purchase_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<PurchaseDto>, AppErrorDto> {
    run_command("purchase_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::purchases::list_purchases(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn purchase_get(
    state: State<'_, AppState>,
    session: String,
    purchase_id: i64,
) -> Result<PurchaseDto, AppErrorDto> {
    run_command("purchase_get", async move {
        let principal = authenticated(&state, &session).await?;
        application::purchases::get_purchase(&state, &principal, purchase_id).await
    })
    .await
}

#[tauri::command]
pub async fn payable_aging(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<PayableAgingRowDto>, AppErrorDto> {
    run_command("payable_aging", async move {
        let principal = authenticated(&state, &session).await?;
        application::purchases::payable_aging(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn supplier_payment_create(
    state: State<'_, AppState>,
    session: String,
    input: SupplierPaymentInput,
) -> Result<SupplierPaymentDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation(
        "supplier_payment_create",
        correlation_id.clone(),
        async move {
            let principal = authenticated(&state, &session).await?;
            application::purchases::create_payment(&state, &principal, input, &correlation_id).await
        },
    )
    .await
}

#[tauri::command]
pub async fn supplier_payment_void(
    state: State<'_, AppState>,
    session: String,
    input: SupplierPaymentVoidInput,
) -> Result<SupplierPaymentDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation(
        "supplier_payment_void",
        correlation_id.clone(),
        async move {
            let principal = authenticated(&state, &session).await?;
            application::purchases::void_payment(&state, &principal, input, &correlation_id).await
        },
    )
    .await
}

#[tauri::command]
pub async fn supplier_payment_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<SupplierPaymentDto>, AppErrorDto> {
    run_command("supplier_payment_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::purchases::list_payments(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn supplier_return_create(
    state: State<'_, AppState>,
    session: String,
    input: SupplierReturnCreateInput,
) -> Result<SupplierReturnDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation(
        "supplier_return_create",
        correlation_id.clone(),
        async move {
            let principal = authenticated(&state, &session).await?;
            application::purchases::create_return(&state, &principal, input, &correlation_id).await
        },
    )
    .await
}

#[tauri::command]
pub async fn supplier_return_post(
    state: State<'_, AppState>,
    session: String,
    input: SupplierReturnPostInput,
) -> Result<SupplierReturnDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("supplier_return_post", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::purchases::post_return(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn supplier_return_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<SupplierReturnDto>, AppErrorDto> {
    run_command("supplier_return_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::purchases::list_returns(&state, &principal).await
    })
    .await
}
