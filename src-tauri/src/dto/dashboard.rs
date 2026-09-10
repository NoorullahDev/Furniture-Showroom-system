use serde::Serialize;

/// One recent audit event shown in the dashboard activity stream. The audit
/// record already redacts secrets; only the safe projection is surfaced here.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardActivityDto {
    pub id: i64,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub username: Option<String>,
    pub created_at: String,
}

/// One day in the trailing seven-day mini trend. Money is in minor units
/// (paisa); all figures are document-date based to match module reports.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardTrendDayDto {
    pub day: String,
    pub sales_count: i64,
    pub sales_minor: i64,
    pub receipts_minor: i64,
    pub expenses_minor: i64,
}

/// Permission-filtered landing summary. Fields the caller is not allowed to
/// see are returned as 0 (invisible metrics) while cost-based figures such as
/// stock value and gross profit are `None` and omitted from the payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSummaryDto {
    pub as_of: String,
    pub today_sales_count: i64,
    pub today_sales_minor: i64,
    pub today_receipts_minor: i64,
    pub today_expenses_minor: i64,
    pub net_cash_minor: i64,
    pub dues_minor: i64,
    pub overdue_dues_minor: i64,
    pub payables_minor: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock_value_minor: Option<i64>,
    pub low_stock_count: i64,
    pub pending_deliveries: i64,
    pub open_damage_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month_gross_profit_minor: Option<i64>,
    pub month_revenue_minor: i64,
    pub month_expenses_minor: i64,
    pub trend: Vec<DashboardTrendDayDto>,
    pub recent_activity: Vec<DashboardActivityDto>,
}
