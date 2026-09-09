use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Deliveries
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryCreateInput {
    pub sale_id: i64,
    #[serde(default)]
    pub scheduled_at: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub contact_name: Option<String>,
    #[serde(default)]
    pub contact_phone: Option<String>,
    #[serde(default)]
    pub driver_note: Option<String>,
    #[serde(default)]
    pub vehicle_note: Option<String>,
    #[serde(default)]
    pub delivery_charge_minor: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
    pub items: Vec<DeliveryItemInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryItemInput {
    pub sale_item_id: i64,
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryItemDto {
    pub id: i64,
    pub delivery_id: i64,
    pub sale_item_id: i64,
    pub product_id: Option<i64>,
    pub article_number: String,
    pub product_name: String,
    pub quantity: i64,
    pub unit_price_minor: i64,
    pub line_total_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryDto {
    pub id: i64,
    pub delivery_number: Option<String>,
    pub sale_id: i64,
    pub sale_number: Option<String>,
    pub customer_id: Option<i64>,
    pub customer_name: Option<String>,
    pub location_id: i64,
    pub status: String,
    pub scheduled_at: Option<String>,
    pub address: Option<String>,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub driver_note: Option<String>,
    pub vehicle_note: Option<String>,
    pub receiver_name: Option<String>,
    pub proof_reference: Option<String>,
    pub delivery_charge_minor: i64,
    pub notes: Option<String>,
    pub reschedule_count: i64,
    pub delivered_at: Option<String>,
    pub delivered_by: Option<i64>,
    pub dispatched_at: Option<String>,
    pub dispatched_by: Option<i64>,
    pub failed_reason: Option<String>,
    pub failed_at: Option<String>,
    pub failed_by: Option<i64>,
    pub cancelled_reason: Option<String>,
    pub cancelled_at: Option<String>,
    pub cancelled_by: Option<i64>,
    pub items: Vec<DeliveryItemDto>,
    pub created_by: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryTransitionInput {
    pub delivery_id: i64,
    /// 'ready', 'dispatched', 'delivered', 'failed', 'cancelled', 'reschedule'.
    pub action: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub scheduled_at: Option<String>,
    #[serde(default)]
    pub receiver_name: Option<String>,
    #[serde(default)]
    pub proof_reference: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryListInput {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub sale_id: Option<i64>,
    #[serde(default)]
    pub customer_id: Option<i64>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryRescheduleInput {
    pub delivery_id: i64,
    pub scheduled_at: String,
    #[serde(default)]
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Sales Returns
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnItemInput {
    pub sale_item_id: i64,
    pub quantity: i64,
    /// 'sellable', 'damaged', 'repair', 'disposed'.
    pub classification: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleReturnInput {
    pub sale_id: i64,
    /// 'credit', 'cash', or 'exchange'.
    pub refund_type: String,
    pub return_date: String,
    pub items: Vec<ReturnItemInput>,
    #[serde(default)]
    pub cash_account_id: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleReturnItemDto {
    pub id: i64,
    pub return_id: i64,
    pub sale_item_id: i64,
    pub product_id: Option<i64>,
    pub bundle_id: Option<i64>,
    pub article_number: String,
    pub product_name: String,
    pub quantity: i64,
    pub unit_price_minor: i64,
    pub unit_refund_minor: i64,
    pub line_refund_minor: i64,
    pub classification: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleReturnDto {
    pub id: i64,
    pub return_number: Option<String>,
    pub sale_id: i64,
    pub sale_number: Option<String>,
    pub customer_id: Option<i64>,
    pub customer_name: Option<String>,
    pub location_id: i64,
    pub return_date: String,
    pub status: String,
    pub refund_type: String,
    pub total_minor: i64,
    pub total_refund_minor: i64,
    pub cash_refund_minor: i64,
    pub credit_note_minor: i64,
    pub notes: Option<String>,
    pub posted_by: Option<i64>,
    pub posted_at: Option<String>,
    pub voided_by: Option<i64>,
    pub voided_at: Option<String>,
    pub items: Vec<SaleReturnItemDto>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnVoidInput {
    pub return_id: i64,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnListInput {
    #[serde(default)]
    pub sale_id: Option<i64>,
    #[serde(default)]
    pub customer_id: Option<i64>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

// ---------------------------------------------------------------------------
// Credit Notes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditNoteDto {
    pub id: i64,
    pub credit_number: Option<String>,
    pub customer_id: i64,
    pub return_id: Option<i64>,
    pub sale_id: Option<i64>,
    pub amount_minor: i64,
    pub status: String,
    pub notes: Option<String>,
    pub applied_at: Option<String>,
    pub created_by: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditNoteListInput {
    #[serde(default)]
    pub customer_id: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

// ---------------------------------------------------------------------------
// Damage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageRecordInput {
    pub product_id: i64,
    pub location_id: i64,
    pub quantity: i64,
    pub damage_date: String,
    /// 'in_hand', 'customer_return', 'count', 'other'.
    pub source: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub estimated_loss_minor: Option<i64>,
    #[serde(default)]
    pub photo_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageDecisionInput {
    pub damage_id: i64,
    /// 'repair', 'supplier_return', 'damaged_sale', 'write_off'.
    pub decision: String,
    #[serde(default)]
    pub decision_note: Option<String>,
    #[serde(default)]
    pub linked_sale_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageRecordDto {
    pub id: i64,
    pub damage_number: Option<String>,
    pub product_id: i64,
    pub article_number: String,
    pub product_name: String,
    pub location_id: i64,
    pub location_name: String,
    pub quantity: i64,
    pub damage_date: String,
    pub source: String,
    pub reason: Option<String>,
    pub estimated_loss_minor: i64,
    pub photo_path: Option<String>,
    pub status: String,
    pub decision: Option<String>,
    pub decision_note: Option<String>,
    pub linked_sale_id: Option<i64>,
    pub resolved_by: Option<i64>,
    pub resolved_at: Option<String>,
    pub created_by: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageListInput {
    #[serde(default)]
    pub product_id: Option<i64>,
    #[serde(default)]
    pub location_id: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}
