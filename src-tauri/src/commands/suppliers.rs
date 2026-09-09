use tauri::State;

use crate::application;
use crate::commands::authenticated;
use crate::commands::wrapper::{run_command, run_command_with_correlation};
use crate::dto::purchases::{
    CashAccountDto, CashAccountInput, CashEntryDto, SupplierDto, SupplierInput,
    SupplierLedgerEntryDto,
};
use crate::dto::AppErrorDto;
use crate::error::new_correlation_id;
use crate::state::AppState;

#[tauri::command]
pub async fn supplier_create(
    state: State<'_, AppState>,
    session: String,
    input: SupplierInput,
) -> Result<SupplierDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("supplier_create", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::suppliers::create(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn supplier_update(
    state: State<'_, AppState>,
    session: String,
    supplier_id: i64,
    input: SupplierInput,
) -> Result<SupplierDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("supplier_update", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::suppliers::update(&state, &principal, supplier_id, input, &correlation_id)
            .await
    })
    .await
}

#[tauri::command]
pub async fn supplier_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<SupplierDto>, AppErrorDto> {
    run_command("supplier_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::suppliers::list(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn supplier_get(
    state: State<'_, AppState>,
    session: String,
    supplier_id: i64,
) -> Result<SupplierDto, AppErrorDto> {
    run_command("supplier_get", async move {
        let principal = authenticated(&state, &session).await?;
        application::suppliers::get(&state, &principal, supplier_id).await
    })
    .await
}

#[tauri::command]
pub async fn supplier_ledger(
    state: State<'_, AppState>,
    session: String,
    supplier_id: i64,
) -> Result<Vec<SupplierLedgerEntryDto>, AppErrorDto> {
    run_command("supplier_ledger", async move {
        let principal = authenticated(&state, &session).await?;
        application::suppliers::ledger(&state, &principal, supplier_id).await
    })
    .await
}

#[tauri::command]
pub async fn payment_method_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<application::cash::PaymentMethodDto>, AppErrorDto> {
    run_command("payment_method_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::cash::list_payment_methods(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn cash_account_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<CashAccountDto>, AppErrorDto> {
    run_command("cash_account_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::cash::list_cash_accounts(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn cash_account_create(
    state: State<'_, AppState>,
    session: String,
    input: CashAccountInput,
) -> Result<CashAccountDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("cash_account_create", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::cash::create_cash_account(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn cash_entry_list(
    state: State<'_, AppState>,
    session: String,
    account_id: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<CashEntryDto>, AppErrorDto> {
    run_command("cash_entry_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::cash::list_cash_entries(&state, &principal, account_id, limit).await
    })
    .await
}
