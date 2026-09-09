use serde::{Deserialize, Serialize};

use crate::dto::sales::CustomerLedgerEntryDto;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerStatementInput {
    pub customer_id: i64,
    pub from_date: String,
    pub to_date: String,
}

/// Statement for a date range: opening balance, the entries within the range
/// (with running balances), and the closing balance implied by the ledger.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerStatementDto {
    pub customer_id: i64,
    pub customer_name: String,
    pub from_date: String,
    pub to_date: String,
    pub opening_balance_minor: i64,
    pub closing_balance_minor: i64,
    pub entries: Vec<CustomerLedgerEntryDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerReceiptPreviewInput {
    pub customer_id: i64,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptAllocationPreviewDto {
    pub sale_id: i64,
    pub sale_number: Option<String>,
    pub sale_date: String,
    pub due_date: Option<String>,
    pub total_minor: i64,
    pub due_minor: i64,
    pub allocated_minor: i64,
}

/// How a receipt amount would be split if posted now: oldest-first over open
/// confirmed invoices, with the remainder recorded as a customer advance.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptPreviewDto {
    pub customer_id: i64,
    pub customer_name: String,
    pub amount_minor: i64,
    pub allocations: Vec<ReceiptAllocationPreviewDto>,
    pub advance_minor: i64,
}

/// An open confirmed invoice that is overdue or due soon.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivableSaleDto {
    pub sale_id: i64,
    pub sale_number: Option<String>,
    pub customer_id: i64,
    pub customer_name: String,
    pub sale_date: String,
    pub due_date: Option<String>,
    pub total_minor: i64,
    pub paid_minor: i64,
    pub advance_used_minor: i64,
    pub due_minor: i64,
    /// Signed day offset: for the overdue list this is days overdue (>= 1);
    /// for the due-soon list this is days until the due date (>= 0).
    pub days: i64,
}

/// A customer with an outstanding balance and overdue exposure.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivableCustomerDto {
    pub customer_id: i64,
    pub customer_name: String,
    pub phone: Option<String>,
    pub balance_minor: i64,
    pub credit_limit_minor: i64,
    pub due_minor_total: i64,
    pub overdue_minor_total: i64,
}

/// Phase 7 due-control snapshot used for exception screens and reminders.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivablesDto {
    pub overdue: Vec<ReceivableSaleDto>,
    pub due_soon: Vec<ReceivableSaleDto>,
    pub high_balance: Vec<ReceivableCustomerDto>,
    pub credit_limit_exceptions: Vec<ReceivableCustomerDto>,
}
