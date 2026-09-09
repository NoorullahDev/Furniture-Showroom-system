use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationDto {
    pub id: i64,
    pub name: String,
    pub location_type: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockBalanceDto {
    pub product_id: i64,
    pub article_number: String,
    pub product_name: String,
    pub thumbnail_path: Option<String>,
    pub location_id: i64,
    pub location_name: String,
    pub on_hand: i64,
    pub reserved: i64,
    pub damaged: i64,
    pub minimum_stock: i64,
    pub available: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockMovementDto {
    pub id: i64,
    pub product_id: i64,
    pub article_number: Option<String>,
    pub product_name: Option<String>,
    pub location_id: i64,
    pub location_name: Option<String>,
    pub movement_type: String,
    pub quantity_delta: i64,
    pub move_number: Option<String>,
    pub unit_cost_minor: Option<i64>,
    pub reference_type: Option<String>,
    pub reference_id: Option<i64>,
    pub reason: Option<String>,
    pub reversal_of_id: Option<i64>,
    pub created_by: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValuationLineDto {
    pub product_id: i64,
    pub article_number: String,
    pub product_name: String,
    pub unit_cost_minor: i64,
    pub sellable_qty: i64,
    pub value_minor: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostStockInput {
    pub product_id: i64,
    pub location_id: i64,
    pub quantity: i64,
    #[serde(default)]
    pub unit_cost_minor: Option<i64>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferStockInput {
    pub product_id: i64,
    pub from_location_id: i64,
    pub to_location_id: i64,
    pub quantity: i64,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdjustStockInput {
    pub product_id: i64,
    pub location_id: i64,
    pub adjustment_qty: i64,
    #[serde(default)]
    pub unit_cost_minor: Option<i64>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageStockInput {
    pub product_id: i64,
    pub location_id: i64,
    pub quantity: i64,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseStockInput {
    pub product_id: i64,
    pub location_id: i64,
    pub quantity: i64,
    #[serde(default)]
    pub reason: Option<String>,
}
