use tauri::State;

use crate::application;
use crate::commands::authenticated;
use crate::commands::wrapper::{run_command, run_command_with_correlation};
use crate::dto::sales::{
    BundleAvailabilityDto, BundleDto, BundleInput, CustomerDto, CustomerInput,
    CustomerLedgerEntryDto, CustomerPaymentDto, CustomerPaymentVoidInput, CustomerReceiptInput,
    SaleCancelInput, SaleConfirmInput, SaleCreateInput, SaleDto,
};
use crate::dto::{AppErrorDto, PdfResultDto};
use crate::error::new_correlation_id;
use crate::state::AppState;

#[tauri::command]
pub async fn customer_create(
    state: State<'_, AppState>,
    session: String,
    input: CustomerInput,
) -> Result<CustomerDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("customer_create", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::customers::create(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn customer_update(
    state: State<'_, AppState>,
    session: String,
    customer_id: i64,
    input: CustomerInput,
) -> Result<CustomerDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("customer_update", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::customers::update(&state, &principal, customer_id, input, &correlation_id)
            .await
    })
    .await
}

#[tauri::command]
pub async fn customer_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<CustomerDto>, AppErrorDto> {
    run_command("customer_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::customers::list(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn customer_get(
    state: State<'_, AppState>,
    session: String,
    customer_id: i64,
) -> Result<CustomerDto, AppErrorDto> {
    run_command("customer_get", async move {
        let principal = authenticated(&state, &session).await?;
        application::customers::get(&state, &principal, customer_id).await
    })
    .await
}

#[tauri::command]
pub async fn customer_ledger(
    state: State<'_, AppState>,
    session: String,
    customer_id: i64,
) -> Result<Vec<CustomerLedgerEntryDto>, AppErrorDto> {
    run_command("customer_ledger", async move {
        let principal = authenticated(&state, &session).await?;
        application::customers::ledger(&state, &principal, customer_id).await
    })
    .await
}

#[tauri::command]
pub async fn customer_receipt_create(
    state: State<'_, AppState>,
    session: String,
    input: CustomerReceiptInput,
) -> Result<CustomerPaymentDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation(
        "customer_receipt_create",
        correlation_id.clone(),
        async move {
            let principal = authenticated(&state, &session).await?;
            application::customers::create_receipt(&state, &principal, input, &correlation_id).await
        },
    )
    .await
}

#[tauri::command]
pub async fn customer_receipt_void(
    state: State<'_, AppState>,
    session: String,
    input: CustomerPaymentVoidInput,
) -> Result<CustomerPaymentDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation(
        "customer_receipt_void",
        correlation_id.clone(),
        async move {
            let principal = authenticated(&state, &session).await?;
            application::customers::void_receipt(&state, &principal, input, &correlation_id).await
        },
    )
    .await
}

#[tauri::command]
pub async fn customer_receipt_list(
    state: State<'_, AppState>,
    session: String,
    customer_id: i64,
) -> Result<Vec<CustomerPaymentDto>, AppErrorDto> {
    run_command("customer_receipt_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::customers::list_receipts(&state, &principal, customer_id).await
    })
    .await
}

#[tauri::command]
pub async fn bundle_create(
    state: State<'_, AppState>,
    session: String,
    input: BundleInput,
) -> Result<BundleDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("bundle_create", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::sets::create(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn bundle_update(
    state: State<'_, AppState>,
    session: String,
    bundle_id: i64,
    input: BundleInput,
) -> Result<BundleDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("bundle_update", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::sets::update(&state, &principal, bundle_id, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn bundle_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<BundleDto>, AppErrorDto> {
    run_command("bundle_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::sets::list(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn bundle_get(
    state: State<'_, AppState>,
    session: String,
    bundle_id: i64,
) -> Result<BundleDto, AppErrorDto> {
    run_command("bundle_get", async move {
        let principal = authenticated(&state, &session).await?;
        application::sets::get(&state, &principal, bundle_id).await
    })
    .await
}

#[tauri::command]
pub async fn bundle_availability(
    state: State<'_, AppState>,
    session: String,
    bundle_id: i64,
    location_id: i64,
) -> Result<BundleAvailabilityDto, AppErrorDto> {
    run_command("bundle_availability", async move {
        let principal = authenticated(&state, &session).await?;
        application::sets::availability(&state, &principal, bundle_id, location_id).await
    })
    .await
}

#[tauri::command]
pub async fn sale_create(
    state: State<'_, AppState>,
    session: String,
    input: SaleCreateInput,
) -> Result<SaleDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("sale_create", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::sales::create_sale(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn sale_confirm(
    state: State<'_, AppState>,
    session: String,
    input: SaleConfirmInput,
) -> Result<SaleDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("sale_confirm", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::sales::confirm_sale(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn sale_cancel(
    state: State<'_, AppState>,
    session: String,
    input: SaleCancelInput,
) -> Result<SaleDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("sale_cancel", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::sales::cancel_sale(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn sale_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<SaleDto>, AppErrorDto> {
    run_command("sale_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::sales::list_sales(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn sale_get(
    state: State<'_, AppState>,
    session: String,
    sale_id: i64,
) -> Result<SaleDto, AppErrorDto> {
    run_command("sale_get", async move {
        let principal = authenticated(&state, &session).await?;
        application::sales::get_sale(&state, &principal, sale_id).await
    })
    .await
}

#[tauri::command]
pub async fn sale_invoice_pdf(
    state: State<'_, AppState>,
    session: String,
    sale_id: i64,
) -> Result<PdfResultDto, AppErrorDto> {
    run_command("sale_invoice_pdf", async move {
        let principal = authenticated(&state, &session).await?;
        application::sales::invoice_pdf(&state, &principal, sale_id).await
    })
    .await
}
