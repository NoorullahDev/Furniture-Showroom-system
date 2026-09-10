use tauri::State;

use crate::commands::wrapper::{run_command, run_command_with_correlation};
use crate::commands::{authed, authenticated};
use crate::dto::expenses::{
    ExpenseCategoryDto, ExpenseCategoryInput, ExpenseCategoryUpdateInput, ExpenseDto, ExpenseInput,
    ExpenseListInput, ExpenseReverseInput, OwnerTransactionDto, OwnerTransactionInput,
    ProfitSummaryDto,
};
use crate::dto::AppErrorDto;
use crate::error::new_correlation_id;
use crate::state::AppState;

#[tauri::command]
pub async fn expense_category_list(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<ExpenseCategoryDto>, AppErrorDto> {
    run_command("expense_category_list", async move {
        let principal = authenticated(&state, &session).await?;
        crate::application::expenses::expense_category_list(&state, &principal).await
    })
    .await
}

#[tauri::command]
pub async fn expense_category_create(
    state: State<'_, AppState>,
    session: String,
    input: ExpenseCategoryInput,
) -> Result<ExpenseCategoryDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation(
        "expense_category_create",
        correlation_id.clone(),
        async move {
            let principal = authenticated(&state, &session).await?;
            crate::application::expenses::expense_category_create(
                &state,
                &principal,
                input,
                &correlation_id,
            )
            .await
        },
    )
    .await
}

#[tauri::command]
pub async fn expense_category_update(
    state: State<'_, AppState>,
    session: String,
    input: ExpenseCategoryUpdateInput,
) -> Result<ExpenseCategoryDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation(
        "expense_category_update",
        correlation_id.clone(),
        async move {
            let principal = authenticated(&state, &session).await?;
            crate::application::expenses::expense_category_update(
                &state,
                &principal,
                input,
                &correlation_id,
            )
            .await
        },
    )
    .await
}

#[tauri::command]
pub async fn expense_list(
    state: State<'_, AppState>,
    session: String,
    input: ExpenseListInput,
) -> Result<Vec<ExpenseDto>, AppErrorDto> {
    run_command("expense_list", async move {
        let principal = authenticated(&state, &session).await?;
        crate::application::expenses::expense_list(&state, &principal, input).await
    })
    .await
}

#[tauri::command]
pub async fn expense_post(
    state: State<'_, AppState>,
    session: String,
    input: ExpenseInput,
) -> Result<ExpenseDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("expense_post", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        crate::application::expenses::expense_post(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn expense_reverse(
    state: State<'_, AppState>,
    session: String,
    input: ExpenseReverseInput,
) -> Result<ExpenseDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("expense_reverse", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        crate::application::expenses::expense_reverse(&state, &principal, input, &correlation_id)
            .await
    })
    .await
}

#[tauri::command]
pub async fn owner_transaction_post(
    state: State<'_, AppState>,
    session: String,
    input: OwnerTransactionInput,
) -> Result<OwnerTransactionDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation(
        "owner_transaction_post",
        correlation_id.clone(),
        async move {
            let principal = authenticated(&state, &session).await?;
            crate::application::expenses::owner_transaction_post(
                &state,
                &principal,
                input,
                &correlation_id,
            )
            .await
        },
    )
    .await
}

#[tauri::command]
pub async fn owner_transaction_list(
    state: State<'_, AppState>,
    session: String,
    limit: Option<i64>,
) -> Result<Vec<OwnerTransactionDto>, AppErrorDto> {
    run_command("owner_transaction_list", async move {
        let principal = authenticated(&state, &session).await?;
        crate::application::expenses::owner_transaction_list(&state, &principal, limit).await
    })
    .await
}

#[tauri::command]
pub async fn profit_summary(
    state: State<'_, AppState>,
    session: String,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<ProfitSummaryDto, AppErrorDto> {
    run_command("profit_summary", async move {
        let principal = authed(&state, &session, "profit.view").await?;
        crate::application::expenses::profit_summary(&state, &principal, from_date, to_date).await
    })
    .await
}
