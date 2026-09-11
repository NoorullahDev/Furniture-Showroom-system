use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardDeliveryDto {
    pub id: i64,
    pub delivery_number: Option<String>,
    pub sale_id: i64,
    pub customer_id: Option<i64>,
    pub customer_name: String,
    pub items: String,
    pub scheduled_at: Option<String>,
    pub status: String,
    pub is_overdue: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardTransactionDto {
    pub id: i64,
    pub customer_id: Option<i64>,
    pub transaction_date: String,
    pub customer_name: String,
    pub reference: String,
    pub transaction_type: String,
    pub amount_minor: i64,
}

/// Permission-filtered landing summary. A missing value means the signed-in
/// account is not allowed to view that metric; a real zero is always returned
/// as `Some(0)` so permission restrictions and failed requests never masquerade
/// as genuine zero balances.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSummaryDto {
    pub as_of: String,
    pub shop_date: String,
    pub today_sales_count: Option<i64>,
    pub today_sales_minor: Option<i64>,
    pub today_received_count: Option<i64>,
    pub today_received_minor: Option<i64>,
    pub month_sales_minor: Option<i64>,
    pub customer_dues_minor: Option<i64>,
    pub supplier_payables_minor: Option<i64>,
    pub pending_deliveries: Option<i64>,
    pub overdue_customer_count: Option<i64>,
    pub overdue_customer_minor: Option<i64>,
    pub overdue_supplier_count: Option<i64>,
    pub overdue_supplier_minor: Option<i64>,
    pub low_stock_count: i64,
    pub open_damage_count: Option<i64>,
    pub upcoming_deliveries: Vec<DashboardDeliveryDto>,
    pub recent_transactions: Vec<DashboardTransactionDto>,
}
