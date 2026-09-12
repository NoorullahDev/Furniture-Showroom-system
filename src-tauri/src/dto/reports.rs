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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportViewColumnInput {
    pub header: String,
    pub align_right: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportViewSummaryInput {
    pub label: String,
    pub value: String,
}

/// The already-filtered table shown on the Reports page. Supplying this with
/// an export keeps CSV, PDF, preview, and print output on the exact same data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportViewInput {
    pub title: String,
    pub columns: Vec<ReportViewColumnInput>,
    pub rows: Vec<Vec<String>>,
    pub summary: Vec<ReportViewSummaryInput>,
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
