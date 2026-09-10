# Phase 11: Reports Framework + 6 Key Reports (PDF/CSV Export)

## Scope

Core report framework with shared filter DTOs and export infrastructure, then 6 key reports — each with PDF and CSV export. No new data commands; existing commands supply table data. The reports page provides filter UI, data tables, and one-click export.

## Reports in Scope

| # | Report | Data Source (existing) | Key Columns |
|---|--------|----------------------|-------------|
| 1 | Sales Summary | `sale_list` | date, number, customer, items, total, paid, due, payment method |
| 2 | Stock Valuation | `stock_valuation` | article, name, qty, cost_price, selling_price, total_value |
| 3 | Customer Dues | `receivables` | customer, total_billed, total_paid, balance, overdue |
| 4 | Supplier Payables | `payable_aging` | supplier, total_purchases, total_paid, balance, overdue |
| 5 | Profit & Loss | `profit_summary` | revenue, cogs, gross_profit, expenses, net_profit, margin |
| 6 | Expense Report | `expense_list` | date, category, description, amount, payment_method, created_by |

## Architecture

```
Frontend                          Rust Backend
─────────                         ────────────
reports-page.tsx                   commands/reports.rs
  ├─ filter-panel.tsx                └─ report_export(session, type, filter, format)
  ├─ data-table (reuse)                → application/reports.rs
  └─ export buttons                      ├─ query_sales_summary(filter)
       ↓                                 ├─ query_stock_valuation(filter)
    report_export cmd                    ├─ query_customer_dues(filter)
       ↓                                 ├─ query_supplier_payables(filter)
    PdfResultDto /                       ├─ query_profit_loss(filter)
    ExportResultDto                      ├─ query_expense_report(filter)
                                         ├─ generate_report_pdf(title, columns, rows, totals, path)
                                         └─ generate_report_csv(title, columns, rows, totals, path)
```

### No New Data Commands

Each report reuses the existing data command for table display:
- Sales Summary → `sale_list` (filtered by date)
- Stock Valuation → `stock_valuation`
- Customer Dues → `receivables`
- Supplier Payables → `payable_aging`
- Profit & Loss → `profit_summary`
- Expense Report → `expense_list` (filtered by date/category)

The frontend calls the existing API hooks. Only the export commands are new.

---

## Implementation Steps

### Step 1: Add `csv` crate + shared DTOs

**Files:**
- `src-tauri/Cargo.toml` — add `csv = "1"`
- `src-tauri/src/dto/reports.rs` — new file
- `src-tauri/src/dto/mod.rs` — add `pub mod reports;`

**`dto/reports.rs` content:**

```rust
use serde::{Deserialize, Serialize};

/// Date range filter shared across all reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportFilterInput {
    /// ISO date "2026-01-01" or null for all-time.
    pub from_date: Option<String>,
    /// ISO date "2026-09-30" or null for all-time.
    pub to_date: Option<String>,
    /// Optional cash account filter (expenses report).
    pub cash_account_id: Option<i64>,
    /// Optional category filter (expenses report).
    pub category_id: Option<i64>,
    /// Optional status filter.
    pub status: Option<String>,
}

/// Export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Pdf,
}

/// Result of a file export.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportExportResult {
    pub report_path: String,
    pub format: ExportFormat,
    pub row_count: i64,
    pub generated_at: String,
}
```

**Verification:** `cargo check` passes.

---

### Step 2: CSV export infrastructure

**File:** `src-tauri/src/infrastructure/csv_export.rs` — new file

**`csv_export.rs` content:**

Core function that takes column headers + rows (Vec<Vec<String>>) + totals + metadata and writes a CSV file.

```rust
use std::fs;
use std::io::BufWriter;
use std::path::Path;

pub struct CsvTable {
    pub title: String,
    pub generated_at: String,
    pub filter_summary: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub totals: Option<Vec<String>>,
}

/// Neutralize spreadsheet-formula injection by prefixing with a single quote.
fn sanitize_cell(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('=') || trimmed.starts_with('+') || trimmed.starts_with('-') || trimmed.starts_with('@') {
        format!("'{}", trimmed)
    } else {
        trimmed.to_string()
    }
}

pub fn write_csv(table: &CsvTable, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = fs::File::create(path)?;
    let mut wtr = csv::Writer::from_writer(BufWriter::new(file));

    // Metadata rows
    wtr.write_record(["Report", &table.title])?;
    wtr.write_record(["Generated", &table.generated_at])?;
    wtr.write_record(["Filters", &table.filter_summary])?;
    wtr.write_record::<[&str; 0], _>([])?; // blank row

    // Header row
    wtr.write_record(table.columns.iter().map(|c| sanitize_cell(c)))?;

    // Data rows
    for row in &table.rows {
        wtr.write_record(row.iter().map(|c| sanitize_cell(c)))?;
    }

    // Totals row
    if let Some(totals) = &table.totals {
        wtr.write_record::<[&str; 0], _>([])?; // blank row
        wtr.write_record(totals.iter().map(|c| sanitize_cell(c)))?;
    }

    wtr.flush()?;
    Ok(())
}
```

**`infrastructure/mod.rs`** — add `pub mod csv_export;` + re-export `CsvTable`, `write_csv`.

**Verification:** `cargo check` passes.

---

### Step 3: PDF tabular report template

**File:** `src-tauri/src/infrastructure/pdf.rs` — add new function at bottom

**New function: `generate_report_pdf`**

Extends the existing single-page PDF pattern with:
- Multi-page support (new page via `doc.add_page()` when Y < threshold)
- Table rendering with column headers, rows, and totals
- Header on every page (report title, shop info, date range)
- Page numbers on every page
- Repeated column headers on new pages

**Key design:**

```rust
pub struct ReportPdfInput {
    pub title: String,
    pub shop_name: String,
    pub shop_address: String,
    pub filter_summary: String,
    pub columns: Vec<ReportPdfColumn>,
    pub rows: Vec<Vec<String>>,
    pub totals: Option<Vec<String>>,
}

pub struct ReportPdfColumn {
    pub header: String,
    pub width_mm: f64,  // relative width, scaled to fit A4
    pub align_left: bool,
}

pub struct ReportPdf {
    pub path: String,
    pub pages: usize,
    pub bytes: u64,
}
```

The function:
1. Creates the document with A4 portrait (210×297mm)
2. Loads Helvetica + Helvetica Bold
3. Renders header block (shop name, address, title, filter summary, generated-at)
4. Renders column headers with horizontal rule underneath
5. Iterates rows, tracking Y position
6. When Y < 40mm, calls `doc.add_page()` for a new page, re-renders header + column headers
7. After all rows, renders totals row with bold font
8. Adds page numbers to each page (requires two-pass or post-processing — printpdf 0.7.0 may not support this easily; skip page numbers for MVP if needed)
9. Saves to file, returns path/pages/bytes

**Layout constants:**
- Page: A4 = 210mm × 297mm
- Top margin: 15mm (header starts at Y=282mm)
- Bottom margin: 20mm (footer area)
- Left margin: 15mm
- Right margin: 15mm
- Content width: 180mm
- Column widths: distributed proportionally from `ReportPdfColumn.width_mm`
- Row height: 8mm
- Header block height: ~40mm
- Page-break threshold: Y < 40mm

**Horizontal rules:** Use `printpdf::Line::from_iter()` + `layer.add_shape(line)` to draw thin lines under column headers and above totals.

**Verification:** `cargo check` passes.

---

### Step 4: Report export command dispatch

**Files:**
- `src-tauri/src/application/reports.rs` — new file
- `src-tauri/src/commands/mod.rs` — add `pub mod reports;`
- `src-tauri/src/lib.rs` — register `report_export` command

**`application/reports.rs` content:**

Central dispatch function:

```rust
pub async fn export_report(
    db: &sqlx::SqlitePool,
    reports_dir: &std::path::Path,
    fonts_dir: &std::path::Path,
    report_type: &str,
    filter: &ReportFilterInput,
    format: ExportFormat,
) -> Result<ReportExportResult, AppError> {
    match report_type {
        "sales_summary" => export_sales_summary(db, reports_dir, fonts_dir, filter, format).await,
        "stock_valuation" => export_stock_valuation(db, reports_dir, fonts_dir, filter, format).await,
        "customer_dues" => export_customer_dues(db, reports_dir, fonts_dir, filter, format).await,
        "supplier_payables" => export_supplier_payables(db, reports_dir, fonts_dir, filter, format).await,
        "profit_loss" => export_profit_loss(db, reports_dir, fonts_dir, filter, format).await,
        "expense_report" => export_expense_report(db, reports_dir, fonts_dir, filter, format).await,
        _ => Err(AppError::Validation(format!("Unknown report type: {}", report_type))),
    }
}
```

Each `export_xxx` function:
1. Queries data using existing SQL (or new SQL if aggregation is needed)
2. Builds a `CsvTable` (for CSV) or `ReportPdfInput` (for PDF)
3. Writes the file
4. Returns `ReportExportResult`

**`commands/reports.rs` content:**

```rust
#[tauri::command]
pub async fn report_export(
    session_id: String,
    report_type: String,
    filter: ReportFilterInput,
    format: ExportFormat,
    state: tauri::State<'_, AppState>,
) -> Result<ReportExportResult, AppError> {
    // 1. Authenticate
    let session = state.session_manager.authenticate(&session_id).await?;
    // 2. Permission check
    if !session.permissions.contains("reports.view") {
        return Err(AppError::PermissionDenied("reports.view required".into()));
    }
    // 3. Dispatch
    let result = reports::export_report(
        &state.db, &state.file_paths.reports_dir(), &state.file_paths.fonts_dir(),
        &report_type, &filter, format,
    ).await?;
    Ok(result)
}
```

**`lib.rs` changes:**
- Add `mod commands::reports;`
- Register `commands::reports::report_export` in the invoke handler

**Verification:** `cargo check` + `cargo clippy` pass.

---

### Step 5: Individual report query functions

**File:** `src-tauri/src/application/reports.rs` — extend with per-report functions

Each function queries data and builds the report table. Example for sales summary:

```rust
async fn export_sales_summary(
    db: &SqlitePool, reports_dir: &Path, fonts_dir: &Path,
    filter: &ReportFilterInput, format: ExportFormat,
) -> Result<ReportExportResult, AppError> {
    // Query sales with optional date filter
    let rows = query_sales_for_export(db, filter).await?;

    // Build columns: Date | # | Customer | Items | Total | Paid | Due | Method
    let columns = vec![
        ReportColumn { header: "Date", width: 2.0, align_left: false },
        ReportColumn { header: "Sale #", width: 1.5, align_left: false },
        ReportColumn { header: "Customer", width: 3.0, align_left: true },
        ReportColumn { header: "Items", width: 1.0, align_left: false },
        ReportColumn { header: "Total", width: 2.0, align_left: false },
        ReportColumn { header: "Paid", width: 2.0, align_left: false },
        ReportColumn { header: "Due", width: 2.0, align_left: false },
        ReportColumn { header: "Method", width: 2.0, align_left: true },
    ];

    // Format rows
    let data_rows: Vec<Vec<String>> = rows.iter().map(|r| vec![
        r.date.clone(),
        r.number.clone(),
        r.customer_name.clone().unwrap_or_default(),
        r.item_count.to_string(),
        format_minor(r.total_minor),
        format_minor(r.paid_minor),
        format_minor(r.due_minor),
        r.payment_method.clone().unwrap_or_default(),
    ]).collect();

    // Compute totals
    let totals = Some(vec![
        "".into(), "".into(), "TOTAL".into(),
        rows.iter().map(|r| r.item_count).sum::<i64>().to_string(),
        format_minor(rows.iter().map(|r| r.total_minor).sum()),
        format_minor(rows.iter().map(|r| r.paid_minor).sum()),
        format_minor(rows.iter().map(|r| r.due_minor).sum()),
        "".into(),
    ]);

    // Export
    let title = "Sales Summary".to_string();
    let filter_summary = format_filter(filter);
    match format {
        ExportFormat::Csv => {
            let path = reports_dir.join(format!("sales-summary-{}.csv", Uuid::now_v7()));
            let table = CsvTable { title, generated_at: now(), filter_summary, columns: columns.iter().map(|c| c.header.clone()).collect(), rows: data_rows, totals };
            write_csv(&table, &path)?;
            Ok(ReportExportResult { report_path: path.to_string_lossy().into(), format, row_count: rows.len() as i64, generated_at: now() })
        }
        ExportFormat::Pdf => {
            let path = reports_dir.join(format!("sales-summary-{}.pdf", Uuid::now_v7()));
            let input = ReportPdfInput { title, shop_name, shop_address, filter_summary, columns, rows: data_rows, totals };
            let pdf = generate_report_pdf(input, fonts_dir, &path)?;
            Ok(ReportExportResult { report_path: pdf.path, format, row_count: rows.len() as i64, generated_at: now() })
        }
    }
}
```

The other 5 reports follow the same pattern with their specific queries and column definitions.

**SQL queries** (inline in `reports.rs` or use existing functions from other modules):

| Report | Approach |
|--------|----------|
| Sales Summary | New SQL: `SELECT s.* FROM sales s WHERE s.sale_date BETWEEN ? AND ? ORDER BY s.sale_date DESC` (reuse existing columns from `sales` table) |
| Stock Valuation | Call `products::stock_valuation()` or inline equivalent |
| Customer Dues | Call `receivables::receivables()` or inline: `SELECT customer_name, SUM(total_minor), SUM(paid_minor), ... GROUP BY customer_id` |
| Supplier Payables | Call `payables::payable_aging()` or inline |
| Profit & Loss | Call `expenses::profit_summary()` |
| Expense Report | New SQL: `SELECT e.* FROM expenses e WHERE e.expense_date BETWEEN ? AND ? AND e.status = 'posted'` |

**Verification:** `cargo check` + `cargo clippy` + existing tests still pass.

---

### Step 6: Frontend — Reports page + shell integration

**Files:**
- `src/lib/shell.ts` — add `"reports"` to ShellView union
- `src/components/app-shell.tsx` — wire Reports nav item to setShellView("reports")
- `src/components/reports/reports-page.tsx` — new file (main page)
- `src/components/reports/filter-panel.tsx` — new file (shared filter UI)
- `src/components/reports/report-table.tsx` — new file (data table + export buttons)
- `src/lib/tauri/api.ts` — add `reportExport()` wrapper

**`shell.ts` change:**
```typescript
export type ShellView =
  | "dashboard" | "sales" | "purchases" | "customers" | "suppliers"
  | "products" | "inventory" | "expenses" | "fulfilment" | "admin"
  | "reports";  // ← add this
```

**`reports-page.tsx` structure:**

```tsx
// Main reports hub with 6 report cards
// Each card: icon + title + description
// Click → sets selectedReport → shows FilterPanel + ReportTable for that report
// Back button returns to card grid
```

**`filter-panel.tsx` structure:**

```tsx
// Date range: From date + To date (native <Input type="date">)
// Preset buttons: This Week | This Month | This Quarter | This Year | All Time
// Optional: Category dropdown (expenses), Status dropdown
// Apply button triggers refetch with new filter
```

**`report-table.tsx` structure:**

```tsx
// Receives: data rows, column definitions, totals
// Renders: <Table> with <TableHeader> + <TableBody> + totals row
// Export buttons: CSV + PDF (calls reportExport command)
// Loading skeleton, empty state, error state
// Auto-opens file on export success (via opener or shell.open)
```

**`api.ts` additions:**

```typescript
export interface ReportFilterInput {
  fromDate?: string | null;
  toDate?: string | null;
  cashAccountId?: number | null;
  categoryId?: number | null;
  status?: string | null;
}

export type ExportFormat = "csv" | "pdf";

export interface ReportExportResult {
  reportPath: string;
  format: ExportFormat;
  rowCount: number;
  generatedAt: string;
}

export async function reportExport(
  sessionId: string,
  reportType: string,
  filter: ReportFilterInput,
  format: ExportFormat
): Promise<ReportExportResult> {
  return invoke("report_export", {
    sessionId, reportType, filter, format,
  });
}
```

**Verification:** `npm run typecheck` passes.

---

### Step 7: Wire file opening after export

After export success, auto-open the generated file:
- Use `@tauri-apps/plugin-dialog` or `open` (the `opener` crate) via a Tauri command
- Or: add an `open_report_file(path: String)` command that calls `opener::open(&path)`

Simpler: reuse the existing `opener` crate. Add a small command:

```rust
#[tauri::command]
pub async fn open_file(path: String) -> Result<(), AppError> {
    opener::open(&path).map_err(|e| AppError::Io(e.to_string()))
}
```

Register in `lib.rs`. Frontend calls `reportExport(...).then(r => invoke("open_file", { path: r.reportPath }))`.

---

### Step 8: Tests

**Rust tests** (`application/reports.rs`):

Test each report export function with test fixtures:
- Empty data → CSV/PDF with 0 rows, no totals
- Single row → 1 data row + totals
- Multiple rows → correct totals
- Date filter applied → correct row count
- Unicode in customer/product names → renders correctly

**Frontend smoke test** (manual):
- Reports page loads with 6 cards
- Click each card → filter panel + empty table
- Set date range → Apply → data loads
- Click CSV → file downloads + opens
- Click PDF → file opens in viewer

**Verification:** `cargo test` passes, `cargo clippy` clean, `npm run typecheck` passes, `npm run lint` passes, `npm run build` succeeds.

---

### Step 9: Commit + push

```bash
git add -A
git -c user.name="Furniture Shop Devs" -c user.email="furnitureshop-dev@localhost" commit -m "Phase 11: report framework, 6 key reports with PDF/CSV export"
git push
```

---

## Implementation Order

| Step | Description | Depends On |
|------|-------------|------------|
| 1 | `csv` crate + shared DTOs | — |
| 2 | CSV export infrastructure | 1 |
| 3 | PDF tabular report template | — |
| 4 | Report export command dispatch | 1, 2, 3 |
| 5 | Individual report query functions | 4 |
| 6 | Frontend reports page | 4 |
| 7 | File-opening command | 4 |
| 8 | Tests | 5, 6, 7 |
| 9 | Commit + push | 8 |

Steps 2 and 3 are independent and can be done in parallel.

## File Summary

| File | Action |
|------|--------|
| `src-tauri/Cargo.toml` | Edit: add `csv = "1"` |
| `src-tauri/src/dto/reports.rs` | **New** |
| `src-tauri/src/dto/mod.rs` | Edit: add `pub mod reports;` |
| `src-tauri/src/infrastructure/csv_export.rs` | **New** |
| `src-tauri/src/infrastructure/mod.rs` | Edit: add `pub mod csv_export;` + re-export |
| `src-tauri/src/infrastructure/pdf.rs` | Edit: add `generate_report_pdf()`, `ReportPdfInput`, `ReportPdfColumn`, `ReportPdf` |
| `src-tauri/src/application/reports.rs` | **New** (dispatch + 6 query/export functions) |
| `src-tauri/src/application/mod.rs` | Edit: add `pub mod reports;` |
| `src-tauri/src/commands/reports.rs` | **New** (`report_export`, `open_file`) |
| `src-tauri/src/commands/mod.rs` | Edit: add `pub mod reports;` |
| `src-tauri/src/lib.rs` | Edit: add `mod commands::reports;` + register commands |
| `src/lib/shell.ts` | Edit: add `"reports"` to ShellView |
| `src/components/app-shell.tsx` | Edit: wire Reports nav item |
| `src/components/reports/reports-page.tsx` | **New** |
| `src/components/reports/filter-panel.tsx` | **New** |
| `src/components/reports/report-table.tsx` | **New** |
| `src/lib/tauri/api.ts` | Edit: add `reportExport`, `openFile`, `ReportFilterInput`, `ReportExportResult` |

## Out of Scope (Phase 11+)

- Background export jobs / progress bars
- Print preview / printer preferences dialog
- Landscape PDF templates
- Page numbers in PDFs (printpdf 0.7.0 limitation — two-pass needed)
- Drill-down links in exports
- Report-specific data aggregation commands (use existing commands for table display)
- Remaining 19 reports from §17 (sales by product/article/category/customer/user/payment method, stock movements/adjustments/damage/count, audit exceptions, etc.)
