use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseCategoryDto {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub is_active: bool,
    pub created_by: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseCategoryInput {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseCategoryUpdateInput {
    pub id: i64,
    pub name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseDto {
    pub id: i64,
    pub expense_number: Option<String>,
    pub category_id: i64,
    pub category_code: String,
    pub category_name: String,
    pub amount_minor: i64,
    pub expense_date: String,
    pub cash_account_id: i64,
    pub cash_account_name: String,
    pub description: String,
    pub payee: Option<String>,
    pub reference: Option<String>,
    pub attachment_path: Option<String>,
    pub status: String,
    pub idempotency_key: Option<String>,
    pub created_by: i64,
    pub created_at: String,
    pub posted_by: Option<i64>,
    pub posted_at: Option<String>,
    pub reversed_by: Option<i64>,
    pub reversed_at: Option<String>,
    pub reversal_reason: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseInput {
    pub category_id: i64,
    pub amount_minor: i64,
    pub expense_date: String,
    pub cash_account_id: i64,
    pub description: String,
    #[serde(default)]
    pub payee: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub attachment_path: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseReverseInput {
    pub expense_id: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseListInput {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub category_id: Option<i64>,
    #[serde(default)]
    pub cash_account_id: Option<i64>,
    #[serde(default)]
    pub from_date: Option<String>,
    #[serde(default)]
    pub to_date: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerTransactionDto {
    pub id: i64,
    pub transaction_number: String,
    pub kind: String,
    pub amount_minor: i64,
    pub transaction_date: String,
    pub cash_account_id: i64,
    pub cash_account_name: String,
    pub notes: Option<String>,
    pub idempotency_key: Option<String>,
    pub created_by: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerTransactionInput {
    /// 'capital_in' or 'withdrawal'.
    pub kind: String,
    pub amount_minor: i64,
    pub transaction_date: String,
    pub cash_account_id: i64,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfitSummaryDto {
    pub from_date: String,
    pub to_date: String,
    /// Net revenue from confirmed sales (that day basis), net of posted returns.
    pub revenue_minor: i64,
    /// Cost of goods sold at confirm-time cost snapshots, net of sellable returns.
    pub cogs_minor: i64,
    /// Delivery/service income booked as part of confirmed sales.
    pub delivery_income_minor: i64,
    pub gross_profit_minor: i64,
    /// Posted operating expenses on the document date.
    pub expenses_minor: i64,
    /// Write-off losses resolved in the range.
    pub damage_loss_minor: i64,
    pub operational_profit_minor: i64,
    /// Owner capital/withdrawal amounts (financing flows, excluded from profit).
    pub owner_capital_in_minor: i64,
    pub owner_withdrawals_minor: i64,
    /// Cash book movement on entry (payment) dates within the range.
    pub cash_inflow_minor: i64,
    pub cash_outflow_minor: i64,
    pub net_cash_flow_minor: i64,
}
