use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierDto {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub opening_balance_minor: i64,
    pub balance_minor: i64,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierInput {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub opening_balance_minor: Option<i64>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierLedgerEntryDto {
    pub id: i64,
    pub entry_type: String,
    pub document_type: Option<String>,
    pub document_id: Option<i64>,
    pub amount_minor: i64,
    pub balance_after_minor: i64,
    pub notes: Option<String>,
    pub created_by: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseItemInput {
    pub product_id: i64,
    pub quantity: i64,
    pub unit_cost_minor: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseCreateInput {
    pub supplier_id: i64,
    pub location_id: i64,
    pub invoice_number: String,
    pub invoice_date: String,
    #[serde(default)]
    pub purchase_date: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub items: Vec<PurchaseItemInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchasePostInput {
    pub purchase_id: i64,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub paid_minor: Option<i64>,
    #[serde(default)]
    pub cash_account_id: Option<i64>,
    #[serde(default)]
    pub payment_method_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseItemDto {
    pub product_id: i64,
    pub article_number: String,
    pub product_name: String,
    pub quantity: i64,
    pub unit_cost_minor: i64,
    pub line_total_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseDto {
    pub id: i64,
    pub purchase_number: Option<String>,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub location_id: i64,
    pub invoice_number: String,
    pub invoice_date: String,
    pub purchase_date: String,
    pub status: String,
    pub total_minor: i64,
    pub paid_minor: i64,
    pub due_minor: i64,
    pub notes: Option<String>,
    pub items: Vec<PurchaseItemDto>,
    pub created_at: String,
    pub posted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayableAgingRowDto {
    pub purchase_id: i64,
    pub purchase_number: Option<String>,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub invoice_date: String,
    pub due_minor: i64,
    pub age_days: i64,
    pub bucket: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierPaymentInput {
    pub supplier_id: i64,
    pub payment_method_id: i64,
    pub cash_account_id: i64,
    pub payment_date: String,
    pub amount_minor: i64,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentAllocationDto {
    pub purchase_id: i64,
    pub purchase_number: Option<String>,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierPaymentDto {
    pub id: i64,
    pub payment_number: Option<String>,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub payment_method_id: i64,
    pub payment_method_name: String,
    pub cash_account_id: i64,
    pub cash_account_name: String,
    pub payment_date: String,
    pub amount_minor: i64,
    pub status: String,
    pub notes: Option<String>,
    pub allocations: Vec<PaymentAllocationDto>,
    pub created_at: String,
    pub voided_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierPaymentVoidInput {
    pub payment_id: i64,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierReturnItemInput {
    pub product_id: i64,
    pub quantity: i64,
    pub unit_cost_minor: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierReturnCreateInput {
    pub supplier_id: i64,
    #[serde(default)]
    pub purchase_id: Option<i64>,
    pub location_id: i64,
    pub return_date: String,
    #[serde(default)]
    pub refund_minor: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
    pub items: Vec<SupplierReturnItemInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierReturnPostInput {
    pub return_id: i64,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierReturnItemDto {
    pub product_id: i64,
    pub article_number: String,
    pub product_name: String,
    pub quantity: i64,
    pub unit_cost_minor: i64,
    pub line_total_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierReturnDto {
    pub id: i64,
    pub return_number: Option<String>,
    pub supplier_id: i64,
    pub supplier_name: String,
    pub purchase_id: Option<i64>,
    pub location_id: i64,
    pub return_date: String,
    pub status: String,
    pub total_minor: i64,
    pub refund_minor: i64,
    pub due_reduction_minor: i64,
    pub notes: Option<String>,
    pub items: Vec<SupplierReturnItemDto>,
    pub created_at: String,
    pub posted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashAccountDto {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub opening_balance_minor: i64,
    pub balance_minor: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashAccountInput {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub opening_balance_minor: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashEntryDto {
    pub id: i64,
    pub cash_account_id: i64,
    pub entry_type: String,
    pub amount_minor: i64,
    pub reference_type: Option<String>,
    pub reference_id: Option<i64>,
    pub reason: Option<String>,
    pub created_by: i64,
    pub created_at: String,
}
