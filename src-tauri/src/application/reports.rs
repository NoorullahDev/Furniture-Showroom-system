use chrono::NaiveDate;
use sqlx::Row;

use crate::dto::reports::{ExportFormat, ReportExportResult, ReportFilterInput};
use crate::error::AppError;
use crate::infrastructure::csv_export::{write_csv, CsvTable};
use crate::infrastructure::{generate_report_pdf, ReportPdfColumn, ReportPdfInput};
use crate::state::AppState;

fn format_minor(v: i64) -> String {
    let major = v.div_euclid(100);
    let paisa = (v % 100).abs();
    let digits = major.abs().to_string();
    let mut grouped = String::new();
    let len = digits.len();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let sign = if major < 0 { "-" } else { "" };
    format!("{sign}{grouped}.{paisa:02}")
}

fn filter_summary(f: &ReportFilterInput) -> String {
    let from = f.from_date.as_deref().unwrap_or("all");
    let to = f.to_date.as_deref().unwrap_or("all");
    if from == "all" && to == "all" {
        "All time".to_string()
    } else if from == "all" {
        format!("Up to {to}")
    } else if to == "all" {
        format!("From {from}")
    } else {
        format!("{from} to {to}")
    }
}

fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string()
}

fn date_range_filter(filter: &ReportFilterInput) -> (&str, &str) {
    static ALL_MIN: &str = "0001-01-01";
    static ALL_MAX: &str = "9999-12-31";
    (
        filter.from_date.as_deref().unwrap_or(ALL_MIN),
        filter.to_date.as_deref().unwrap_or(ALL_MAX),
    )
}

pub async fn export_report(
    state: &AppState,
    report_type: &str,
    filter: &ReportFilterInput,
    format: ExportFormat,
) -> Result<ReportExportResult, AppError> {
    match report_type {
        "sales_summary" => export_sales_summary(state, filter, format).await,
        "stock_valuation" => export_stock_valuation(state, filter, format).await,
        "customer_dues" => export_customer_dues(state, filter, format).await,
        "supplier_payables" => export_supplier_payables(state, filter, format).await,
        "profit_loss" => export_profit_loss(state, filter, format).await,
        "expense_report" => export_expense_report(state, filter, format).await,
        _ => Err(AppError::Validation(format!(
            "unknown report type: {report_type}"
        ))),
    }
}

async fn export_sales_summary(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let rows = sqlx::query(
        "SELECT sale_date, sale_number, customer_name, total_minor, paid_minor,
                due_minor, cost_minor, status
         FROM sales
         WHERE sale_date >= ?1 AND sale_date <= ?2
         ORDER BY sale_date DESC, id DESC
         LIMIT 1000",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Date".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Sale #".into(),
            width_ratio: 1.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Customer".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Total".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Paid".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Due".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Status".into(),
            width_ratio: 1.5,
            align_left: true,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_total: i64 = 0;
    let mut total_paid: i64 = 0;
    let mut total_due: i64 = 0;

    for r in &rows {
        let sale_date: String = r.get(0);
        let sale_number: Option<String> = r.get(1);
        let customer_name: Option<String> = r.get(2);
        let total_minor: i64 = r.get(3);
        let paid_minor: i64 = r.get(4);
        let due_minor: i64 = r.get(5);
        let status: String = r.get(7);

        total_total += total_minor;
        total_paid += paid_minor;
        total_due += due_minor;

        data_rows.push(vec![
            sale_date,
            sale_number.unwrap_or_default(),
            customer_name.unwrap_or_else(|| "Walk-in".into()),
            format_minor(total_minor),
            format_minor(paid_minor),
            format_minor(due_minor),
            status,
        ]);
    }

    let totals = Some(vec![
        String::new(),
        String::new(),
        "TOTAL".into(),
        format_minor(total_total),
        format_minor(total_paid),
        format_minor(total_due),
        String::new(),
    ]);

    export_to_file(
        state,
        "Sales Summary",
        filter,
        &columns,
        data_rows,
        totals,
        format,
    )
    .await
}

async fn export_stock_valuation(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
) -> Result<ReportExportResult, AppError> {
    let rows = sqlx::query(
        "SELECT v.article_number, v.product_name, v.unit_cost_minor,
                v.sellable_qty, v.value_minor
         FROM current_valuation v
         ORDER BY v.product_name",
    )
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Article".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Product".into(),
            width_ratio: 4.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Qty".into(),
            width_ratio: 1.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Unit Cost".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Value".into(),
            width_ratio: 2.0,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_qty: i64 = 0;
    let mut total_value: i64 = 0;

    for r in &rows {
        let article: String = r.get(0);
        let name: String = r.get(1);
        let unit_cost: i64 = r.get(2);
        let qty: i64 = r.get(3);
        let value: i64 = r.get(4);

        total_qty += qty;
        total_value += value;

        data_rows.push(vec![
            article,
            name,
            qty.to_string(),
            format_minor(unit_cost),
            format_minor(value),
        ]);
    }

    let totals = Some(vec![
        String::new(),
        "TOTAL".into(),
        total_qty.to_string(),
        String::new(),
        format_minor(total_value),
    ]);

    export_to_file(
        state,
        "Stock Valuation",
        filter,
        &columns,
        data_rows,
        totals,
        format,
    )
    .await
}

async fn export_customer_dues(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
) -> Result<ReportExportResult, AppError> {
    let rows = sqlx::query(
        "SELECT c.name, c.phone,
                COALESCE((SELECT balance_after_minor FROM customer_ledger_entries e
                          WHERE e.customer_id = c.id ORDER BY e.id DESC LIMIT 1),
                         c.opening_balance_minor) AS balance_minor,
                COALESCE(SUM(CASE WHEN s.status = 'confirmed' AND s.due_minor > 0
                                  THEN s.due_minor END), 0) AS due_total,
                COALESCE(SUM(CASE WHEN s.status = 'confirmed' AND s.due_minor > 0
                                   AND s.due_date IS NOT NULL AND s.due_date < date('now')
                                 THEN s.due_minor END), 0) AS overdue_total
         FROM customers c
         LEFT JOIN sales s ON s.customer_id = c.id
         WHERE c.is_active = 1
         GROUP BY c.id
         HAVING balance_minor > 0
         ORDER BY balance_minor DESC
         LIMIT 500",
    )
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Customer".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Phone".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Balance".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Due".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Overdue".into(),
            width_ratio: 2.0,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_balance: i64 = 0;
    let mut total_due: i64 = 0;
    let mut total_overdue: i64 = 0;

    for r in &rows {
        let name: String = r.get(0);
        let phone: Option<String> = r.get(1);
        let balance: i64 = r.get(2);
        let due: i64 = r.get(3);
        let overdue: i64 = r.get(4);

        total_balance += balance;
        total_due += due;
        total_overdue += overdue;

        data_rows.push(vec![
            name,
            phone.unwrap_or_default(),
            format_minor(balance),
            format_minor(due),
            format_minor(overdue),
        ]);
    }

    let totals = Some(vec![
        "TOTAL".into(),
        String::new(),
        format_minor(total_balance),
        format_minor(total_due),
        format_minor(total_overdue),
    ]);

    export_to_file(
        state,
        "Customer Dues",
        filter,
        &columns,
        data_rows,
        totals,
        format,
    )
    .await
}

async fn export_supplier_payables(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
) -> Result<ReportExportResult, AppError> {
    let rows = sqlx::query(
        "SELECT p.supplier_name, p.purchase_number, p.invoice_date, p.due_minor
         FROM purchases p
         WHERE p.status = 'posted' AND p.due_minor > 0
         ORDER BY p.invoice_date ASC, p.id ASC
         LIMIT 500",
    )
    .fetch_all(&state.pool)
    .await?;

    let today = chrono::Utc::now().date_naive();
    let columns = vec![
        ReportPdfColumn {
            header: "Supplier".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Purchase #".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Invoice Date".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Due".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Days".into(),
            width_ratio: 1.0,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_due: i64 = 0;

    for r in &rows {
        let supplier: String = r.get(0);
        let purchase_number: String = r.get(1);
        let invoice_date: String = r.get(2);
        let due: i64 = r.get(3);

        total_due += due;

        let days = NaiveDate::parse_from_str(&invoice_date, "%Y-%m-%d")
            .ok()
            .map(|d| (today - d).num_days().max(0))
            .unwrap_or(0);

        data_rows.push(vec![
            supplier,
            purchase_number,
            invoice_date,
            format_minor(due),
            days.to_string(),
        ]);
    }

    let totals = Some(vec![
        "TOTAL".into(),
        String::new(),
        String::new(),
        format_minor(total_due),
        String::new(),
    ]);

    export_to_file(
        state,
        "Supplier Payables",
        filter,
        &columns,
        data_rows,
        totals,
        format,
    )
    .await
}

async fn export_profit_loss(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);

    let revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE((
            SELECT SUM(total_minor) FROM sales
             WHERE status = 'confirmed' AND sale_date BETWEEN ?1 AND ?2
        ) - (
            SELECT COALESCE(SUM(total_minor), 0) FROM sales_returns
             WHERE status = 'posted' AND return_date BETWEEN ?1 AND ?2
        ), 0)",
    )
    .bind(from)
    .bind(to)
    .fetch_one(&state.pool)
    .await?;

    let cogs: i64 = sqlx::query_scalar(
        "SELECT COALESCE((
            SELECT SUM(cost_minor) FROM sales
             WHERE status = 'confirmed' AND sale_date BETWEEN ?1 AND ?2
        ) - (
            SELECT COALESCE(SUM(ri.quantity * si.unit_cost_minor), 0)
              FROM sales_return_items ri
              JOIN sales_returns sr ON sr.id = ri.return_id AND sr.status = 'posted'
              JOIN sale_items si ON si.id = ri.sale_item_id
             WHERE ri.classification = 'sellable' AND sr.return_date BETWEEN ?1 AND ?2
        ), 0)",
    )
    .bind(from)
    .bind(to)
    .fetch_one(&state.pool)
    .await?;

    let expenses: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_minor), 0)
         FROM expenses WHERE status = 'posted' AND expense_date BETWEEN ?1 AND ?2",
    )
    .bind(from)
    .bind(to)
    .fetch_one(&state.pool)
    .await?;

    let damage_loss: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(estimated_loss_minor), 0)
         FROM damage_records
         WHERE status = 'resolved' AND decision = 'write_off'
           AND damage_date BETWEEN ?1 AND ?2",
    )
    .bind(from)
    .bind(to)
    .fetch_one(&state.pool)
    .await?;

    let gross_profit = revenue - cogs;
    let operational_profit = gross_profit - expenses - damage_loss;
    let margin = if revenue > 0 {
        format!(
            "{:.1}%",
            (operational_profit as f64 / revenue as f64) * 100.0
        )
    } else {
        "N/A".to_string()
    };

    let columns = vec![
        ReportPdfColumn {
            header: "Line Item".into(),
            width_ratio: 4.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Amount".into(),
            width_ratio: 2.0,
            align_left: false,
        },
    ];

    let data_rows = vec![
        vec!["Revenue".into(), format_minor(revenue)],
        vec!["Cost of Goods Sold".into(), format_minor(cogs)],
        vec!["Gross Profit".into(), format_minor(gross_profit)],
        vec!["Expenses".into(), format_minor(expenses)],
        vec!["Damage Loss".into(), format_minor(damage_loss)],
        vec!["Net Profit".into(), format_minor(operational_profit)],
        vec!["Margin".into(), margin],
    ];

    export_to_file(
        state,
        "Profit & Loss",
        filter,
        &columns,
        data_rows,
        None,
        format,
    )
    .await
}

async fn export_expense_report(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);

    let mut query = String::from(
        "SELECT e.expense_number, c.name, e.amount_minor, e.expense_date,
                a.name, e.description, e.status
         FROM expenses e
         JOIN expense_categories c ON c.id = e.category_id
         JOIN cash_accounts a ON a.id = e.cash_account_id
         WHERE e.expense_date >= ?1 AND e.expense_date <= ?2",
    );

    if let Some(cat_id) = filter.category_id {
        query.push_str(&format!(" AND e.category_id = {cat_id}"));
    }
    if let Some(acc_id) = filter.cash_account_id {
        query.push_str(&format!(" AND e.cash_account_id = {acc_id}"));
    }
    if let Some(ref status) = filter.status {
        query.push_str(&format!(" AND e.status = '{status}'"));
    }
    query.push_str(" ORDER BY e.expense_date DESC, e.id DESC LIMIT 1000");

    let rows = sqlx::query(&query)
        .bind(from)
        .bind(to)
        .fetch_all(&state.pool)
        .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Expense #".into(),
            width_ratio: 1.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Category".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Amount".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Date".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Account".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Description".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Status".into(),
            width_ratio: 1.0,
            align_left: true,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_amount: i64 = 0;

    for r in &rows {
        let number: Option<String> = r.get(0);
        let category: String = r.get(1);
        let amount: i64 = r.get(2);
        let date: String = r.get(3);
        let account: String = r.get(4);
        let description: Option<String> = r.get(5);
        let status: String = r.get(6);

        total_amount += amount;

        data_rows.push(vec![
            number.unwrap_or_default(),
            category,
            format_minor(amount),
            date,
            account,
            description.unwrap_or_default(),
            status,
        ]);
    }

    let totals = Some(vec![
        String::new(),
        "TOTAL".into(),
        format_minor(total_amount),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    ]);

    export_to_file(
        state,
        "Expense Report",
        filter,
        &columns,
        data_rows,
        totals,
        format,
    )
    .await
}

async fn export_to_file(
    state: &AppState,
    title: &str,
    filter: &ReportFilterInput,
    columns: &[ReportPdfColumn],
    data_rows: Vec<Vec<String>>,
    totals: Option<Vec<String>>,
    format: ExportFormat,
) -> Result<ReportExportResult, AppError> {
    let generated_at = now_iso();
    let filter_summary = filter_summary(filter);
    let row_count = data_rows.len() as i64;

    let filename_base = title.to_lowercase().replace(' ', "-");
    let id = uuid::Uuid::now_v7();

    match format {
        ExportFormat::Csv => {
            let path = state
                .paths
                .reports_dir
                .join(format!("{filename_base}-{id}.csv"));
            let table = CsvTable {
                title: title.to_string(),
                generated_at: generated_at.clone(),
                filter_summary: filter_summary.clone(),
                columns: columns.iter().map(|c| c.header.clone()).collect(),
                rows: data_rows,
                totals,
            };
            write_csv(&table, &path)
                .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?;
            Ok(ReportExportResult {
                report_path: path.to_string_lossy().into_owned(),
                format: "csv".to_string(),
                row_count,
                generated_at,
            })
        }
        ExportFormat::Pdf => {
            let input = ReportPdfInput {
                title: title.to_string(),
                shop_name: "Furniture Shop".to_string(),
                shop_address: None,
                filter_summary,
                generated_at,
                columns: columns.to_vec(),
                rows: data_rows,
                totals,
            };
            let pdf =
                generate_report_pdf(&input, &state.paths.fonts_dir, &state.paths.reports_dir)?;
            Ok(ReportExportResult {
                report_path: pdf.path,
                format: "pdf".to_string(),
                row_count,
                generated_at: now_iso(),
            })
        }
    }
}
