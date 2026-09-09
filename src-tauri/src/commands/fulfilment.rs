use tauri::State;

use crate::application;
use crate::commands::authenticated;
use crate::commands::wrapper::{run_command, run_command_with_correlation};
use crate::dto::fulfilment::{
    CreditNoteDto, CreditNoteListInput, DamageDecisionInput, DamageListInput, DamageRecordDto,
    DamageRecordInput, DeliveryCreateInput, DeliveryDto, DeliveryListInput,
    DeliveryRescheduleInput, DeliveryTransitionInput, ReturnListInput, ReturnVoidInput,
    SaleReturnDto, SaleReturnInput,
};
use crate::dto::{AppErrorDto, PdfResultDto};
use crate::error::new_correlation_id;
use crate::state::AppState;

#[tauri::command]
pub async fn delivery_create(
    state: State<'_, AppState>,
    session: String,
    input: DeliveryCreateInput,
) -> Result<DeliveryDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("delivery_create", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::create_delivery(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn delivery_transition(
    state: State<'_, AppState>,
    session: String,
    input: DeliveryTransitionInput,
) -> Result<DeliveryDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("delivery_transition", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::transition_delivery(&state, &principal, input, &correlation_id)
            .await
    })
    .await
}

#[tauri::command]
pub async fn delivery_reschedule(
    state: State<'_, AppState>,
    session: String,
    input: DeliveryRescheduleInput,
) -> Result<DeliveryDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("delivery_reschedule", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::reschedule_delivery(&state, &principal, input, &correlation_id)
            .await
    })
    .await
}

#[tauri::command]
pub async fn delivery_list(
    state: State<'_, AppState>,
    session: String,
    input: DeliveryListInput,
) -> Result<Vec<DeliveryDto>, AppErrorDto> {
    run_command("delivery_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::list_deliveries(&state, &principal, input).await
    })
    .await
}

#[tauri::command]
pub async fn delivery_get(
    state: State<'_, AppState>,
    session: String,
    delivery_id: i64,
) -> Result<DeliveryDto, AppErrorDto> {
    run_command("delivery_get", async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::get_delivery(&state, &principal, delivery_id).await
    })
    .await
}

#[tauri::command]
pub async fn delivery_note_pdf(
    state: State<'_, AppState>,
    session: String,
    delivery_id: i64,
) -> Result<PdfResultDto, AppErrorDto> {
    run_command("delivery_note_pdf", async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::delivery_note_pdf(&state, &principal, delivery_id).await
    })
    .await
}

#[tauri::command]
pub async fn sale_return_post(
    state: State<'_, AppState>,
    session: String,
    input: SaleReturnInput,
) -> Result<SaleReturnDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("sale_return_post", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::post_return(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn sale_return_void(
    state: State<'_, AppState>,
    session: String,
    input: ReturnVoidInput,
) -> Result<SaleReturnDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("sale_return_void", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::void_return(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn sale_return_list(
    state: State<'_, AppState>,
    session: String,
    input: ReturnListInput,
) -> Result<Vec<SaleReturnDto>, AppErrorDto> {
    run_command("sale_return_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::list_returns(&state, &principal, input).await
    })
    .await
}

#[tauri::command]
pub async fn sale_return_get(
    state: State<'_, AppState>,
    session: String,
    return_id: i64,
) -> Result<SaleReturnDto, AppErrorDto> {
    run_command("sale_return_get", async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::get_return(&state, &principal, return_id).await
    })
    .await
}

#[tauri::command]
pub async fn credit_note_list(
    state: State<'_, AppState>,
    session: String,
    input: CreditNoteListInput,
) -> Result<Vec<CreditNoteDto>, AppErrorDto> {
    run_command("credit_note_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::list_credit_notes(&state, &principal, input).await
    })
    .await
}

#[tauri::command]
pub async fn credit_note_pdf(
    state: State<'_, AppState>,
    session: String,
    credit_id: i64,
) -> Result<PdfResultDto, AppErrorDto> {
    run_command("credit_note_pdf", async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::credit_note_pdf(&state, &principal, credit_id).await
    })
    .await
}

#[tauri::command]
pub async fn damage_record(
    state: State<'_, AppState>,
    session: String,
    input: DamageRecordInput,
) -> Result<DamageRecordDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("damage_record", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::record_damage(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn damage_decide(
    state: State<'_, AppState>,
    session: String,
    input: DamageDecisionInput,
) -> Result<DamageRecordDto, AppErrorDto> {
    let correlation_id = new_correlation_id();
    run_command_with_correlation("damage_decide", correlation_id.clone(), async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::decide_damage(&state, &principal, input, &correlation_id).await
    })
    .await
}

#[tauri::command]
pub async fn damage_list(
    state: State<'_, AppState>,
    session: String,
    input: DamageListInput,
) -> Result<Vec<DamageRecordDto>, AppErrorDto> {
    run_command("damage_list", async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::list_damage(&state, &principal, input).await
    })
    .await
}

#[tauri::command]
pub async fn damage_get(
    state: State<'_, AppState>,
    session: String,
    damage_id: i64,
) -> Result<DamageRecordDto, AppErrorDto> {
    run_command("damage_get", async move {
        let principal = authenticated(&state, &session).await?;
        application::fulfilment::get_damage(&state, &principal, damage_id).await
    })
    .await
}
