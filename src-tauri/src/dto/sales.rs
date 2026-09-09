use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerDto {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub credit_limit_minor: i64,
    pub opening_balance_minor: i64,
    pub balance_minor: i64,
    pub advance_minor: i64,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerInput {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub credit_limit_minor: Option<i64>,
    #[serde(default)]
    pub opening_balance_minor: Option<i64>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerLedgerEntryDto {
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
pub struct BundleItemInput {
    pub product_id: i64,
    pub quantity: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleInput {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub cover_image_path: Option<String>,
    #[serde(default)]
    pub default_price_minor: Option<i64>,
    #[serde(default)]
    pub is_active: Option<bool>,
    pub items: Vec<BundleItemInput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleItemDto {
    pub product_id: i64,
    pub article_number: String,
    pub product_name: String,
    pub quantity: i64,
    pub sort_order: i64,
    pub unit_cost_estimate_minor: i64,
    pub line_cost_estimate_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleDto {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub cover_image_path: Option<String>,
    pub default_price_minor: i64,
    pub is_active: bool,
    pub items: Vec<BundleItemDto>,
    pub cost_estimate_minor: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleAvailabilityDto {
    pub bundle_id: i64,
    pub bundle_name: String,
    pub location_id: i64,
    pub available_count: i64,
    pub limiting_product_id: Option<i64>,
    pub limiting_product_name: Option<String>,
    pub limiting_available: Option<i64>,
    pub limiting_needed_per_set: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleItemInput {
    #[serde(default)]
    pub product_id: Option<i64>,
    #[serde(default)]
    pub bundle_id: Option<i64>,
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleItemDto {
    pub id: i64,
    pub product_id: Option<i64>,
    pub bundle_id: Option<i64>,
    pub article_number: String,
    pub product_name: String,
    pub quantity: i64,
    pub unit_price_minor: i64,
    pub line_total_minor: i64,
    pub unit_cost_minor: i64,
    pub line_cost_minor: i64,
    pub components: Vec<SaleComponentDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleComponentDto {
    pub product_id: i64,
    pub article_number: String,
    pub product_name: String,
    pub quantity: i64,
    pub unit_cost_minor: i64,
    pub line_cost_minor: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleCreateInput {
    pub location_id: i64,
    #[serde(default)]
    pub customer_id: Option<i64>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub sale_date: Option<String>,
    #[serde(default)]
    pub discount_minor: Option<i64>,
    #[serde(default)]
    pub delivery_charge_minor: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
    pub items: Vec<SaleItemInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleConfirmInput {
    pub sale_id: i64,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub paid_minor: Option<i64>,
    #[serde(default)]
    pub cash_account_id: Option<i64>,
    #[serde(default)]
    pub payment_method_id: Option<i64>,
    #[serde(default)]
    pub advance_used_minor: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleDto {
    pub id: i64,
    pub sale_number: Option<String>,
    pub kind: String,
    pub customer_id: Option<i64>,
    pub customer_name: Option<String>,
    pub location_id: i64,
    pub sale_date: String,
    pub status: String,
    pub subtotal_minor: i64,
    pub discount_minor: i64,
    pub delivery_charge_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
    pub paid_minor: i64,
    pub advance_used_minor: i64,
    pub due_minor: i64,
    pub cost_minor: i64,
    pub notes: Option<String>,
    pub items: Vec<SaleItemDto>,
    pub created_at: String,
    pub confirmed_at: Option<String>,
    pub confirmed_by: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleCancelInput {
    pub sale_id: i64,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerReceiptInput {
    pub customer_id: i64,
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
pub struct SalePaymentAllocationDto {
    pub sale_id: i64,
    pub sale_number: Option<String>,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerPaymentDto {
    pub id: i64,
    pub receipt_number: Option<String>,
    pub customer_id: i64,
    pub customer_name: String,
    pub sale_id: Option<i64>,
    pub payment_method_id: i64,
    pub payment_method_name: String,
    pub cash_account_id: i64,
    pub cash_account_name: String,
    pub payment_date: String,
    pub amount_minor: i64,
    pub advance_alloc_minor: i64,
    pub status: String,
    pub notes: Option<String>,
    pub allocations: Vec<SalePaymentAllocationDto>,
    pub created_at: String,
    pub voided_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerPaymentVoidInput {
    pub payment_id: i64,
    #[serde(default)]
    pub reason: Option<String>,
}
