use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportFilterInput {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub cash_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Pdf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportExportResult {
    pub report_path: String,
    pub format: String,
    pub row_count: i64,
    pub generated_at: String,
}
