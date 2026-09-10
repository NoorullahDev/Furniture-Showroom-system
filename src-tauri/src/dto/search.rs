use serde::Serialize;

/// A single global-search hit. Kind names match the frontend view keys so a
/// result can deep-link straight into the filtered source screen.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultDto {
    /// One of: product, customer, supplier, sale, purchase,
    /// supplier_payment, delivery, receipt, expense.
    pub kind: String,
    pub id: i64,
    pub title: String,
    /// Secondary line, e.g. phone or counter-party name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// Human reference number, e.g. an invoice or PO number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_number: Option<String>,
    /// 0 = exact match, 1 = starts-with, 2 = substring.
    pub rank: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultsDto {
    pub query: String,
    pub results: Vec<SearchResultDto>,
}
