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
    export_report_with_user(state, report_type, filter, format, None).await
}

pub async fn export_report_with_user(
    state: &AppState,
    report_type: &str,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    match report_type {
        "sales_summary" => export_sales_summary(state, filter, format, generated_by).await,
        "stock_valuation" => export_stock_valuation(state, filter, format, generated_by).await,
        "customer_dues" => export_customer_dues(state, filter, format, generated_by).await,
        "supplier_payables" => export_supplier_payables(state, filter, format, generated_by).await,
        "profit_loss" => export_profit_loss(state, filter, format, generated_by).await,
        "expense_report" => export_expense_report(state, filter, format, generated_by).await,
        "purchase_report" => export_purchases(state, filter, format, generated_by).await,
        "returns_report" => export_returns(state, filter, format, generated_by).await,
        "deliveries_report" => {
            export_deliveries_and_damage(state, filter, format, generated_by).await
        }
        "audit_report" => export_audit_log(state, filter, format, generated_by).await,
        "sales_by_product" => export_sales_by_product(state, filter, format, generated_by).await,
        "sales_by_customer" => export_sales_by_customer(state, filter, format, generated_by).await,
        "sales_by_category" => export_sales_by_category(state, filter, format, generated_by).await,
        "best_slow_sellers" => export_best_slow_sellers(state, filter, format, generated_by).await,
        "stock_movements" => export_stock_movements(state, filter, format, generated_by).await,
        "low_stock" => export_low_stock(state, filter, format, generated_by).await,
        "stock_by_location" => export_stock_by_location(state, filter, format, generated_by).await,
        "customer_statements" => {
            export_customer_statements(state, filter, format, generated_by).await
        }
        "customer_receipts" => export_customer_receipts(state, filter, format, generated_by).await,
        "customer_advances" => export_customer_advances(state, filter, format, generated_by).await,
        "credit_limit_exceptions" => {
            export_credit_limit_exceptions(state, filter, format, generated_by).await
        }
        "supplier_statements" => {
            export_supplier_statements(state, filter, format, generated_by).await
        }
        "supplier_payments" => export_supplier_payments(state, filter, format, generated_by).await,
        "cash_book" => export_cash_book(state, filter, format, generated_by).await,
        "account_balances" => export_account_balances(state, filter, format, generated_by).await,
        "user_activity" => export_user_activity(state, filter, format, generated_by).await,
        _ => Err(AppError::Validation(format!(
            "unknown report type: {report_type}"
        ))),
    }
}

async fn export_sales_summary(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
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
        generated_by,
        false,
    )
    .await
}

async fn export_stock_valuation(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
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
        generated_by,
        false,
    )
    .await
}

async fn export_customer_dues(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
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
        generated_by,
        false,
    )
    .await
}

async fn export_supplier_payables(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
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
        generated_by,
        false,
    )
    .await
}

async fn export_profit_loss(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
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
        generated_by,
        false,
    )
    .await
}

async fn export_expense_report(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let currency = crate::application::settings::get(state, "shop.currency")
        .await?
        .and_then(|raw| serde_json::from_str::<String>(&raw).ok())
        .filter(|value| value.len() == 3)
        .unwrap_or_else(|| "PKR".to_string());

    let mut bind_idx: u8 = 3;
    let mut query = String::from(
        "SELECT e.expense_number, c.name, e.amount_minor, e.expense_date,
                a.name, e.description, e.reference, pm.name, e.status
         FROM expenses e
         JOIN expense_categories c ON c.id = e.category_id
         JOIN cash_accounts a ON a.id = e.cash_account_id
         LEFT JOIN payment_methods pm ON pm.id = e.payment_method_id
         WHERE e.expense_date >= ?1 AND e.expense_date <= ?2",
    );

    let cat_id_binds: Vec<i64>;
    if let Some(cat_id) = filter.category_id {
        cat_id_binds = vec![cat_id];
        query.push_str(&format!(" AND e.category_id = ?{bind_idx}"));
        bind_idx += 1;
    } else {
        cat_id_binds = vec![];
    }
    let acc_id_binds: Vec<i64>;
    if let Some(acc_id) = filter.cash_account_id {
        acc_id_binds = vec![acc_id];
        query.push_str(&format!(" AND e.cash_account_id = ?{bind_idx}"));
        bind_idx += 1;
    } else {
        acc_id_binds = vec![];
    }
    let status_binds: Vec<String>;
    if let Some(ref status) = filter.status {
        status_binds = vec![status.clone()];
        query.push_str(&format!(" AND e.status = ?{bind_idx}"));
    } else {
        status_binds = vec![];
        query.push_str(" AND e.status = 'posted'");
    }
    query.push_str(" ORDER BY e.expense_date DESC, e.id DESC LIMIT 10000");

    let mut q = sqlx::query(&query).bind(from).bind(to);
    for v in &cat_id_binds {
        q = q.bind(v);
    }
    for v in &acc_id_binds {
        q = q.bind(v);
    }
    for v in &status_binds {
        q = q.bind(v);
    }
    let rows = q.fetch_all(&state.pool).await?;

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
            header: format!("Amount ({currency})"),
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
            header: "Reference".into(),
            width_ratio: 1.5,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Method".into(),
            width_ratio: 1.5,
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
        let reference: Option<String> = r.get(6);
        let payment_method: Option<String> = r.get(7);
        let status: String = r.get(8);

        total_amount += amount;

        data_rows.push(vec![
            number.unwrap_or_default(),
            category,
            format_minor(amount),
            date,
            account,
            description.unwrap_or_default(),
            reference.unwrap_or_default(),
            payment_method.unwrap_or_default(),
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
        String::new(),
        String::new(),
    ]);

    export_to_file(
        state,
        "Expenses",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        true,
    )
    .await
}

async fn shop_identity(db: &sqlx::SqlitePool) -> (String, Option<String>) {
    let name: String =
        sqlx::query_as::<_, (String,)>("SELECT value_json FROM settings WHERE key = 'shop.name'")
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
            .and_then(|(v,)| serde_json::from_str::<String>(&v).ok())
            .unwrap_or_else(|| "Furniture Shop".into());
    let addr: Option<String> = sqlx::query_as::<_, (String,)>(
        "SELECT value_json FROM settings WHERE key = 'shop.address'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|(v,)| serde_json::from_str::<String>(&v).ok());
    (name, addr)
}

#[allow(clippy::too_many_arguments)]
async fn export_to_file(
    state: &AppState,
    title: &str,
    filter: &ReportFilterInput,
    columns: &[ReportPdfColumn],
    data_rows: Vec<Vec<String>>,
    totals: Option<Vec<String>>,
    format: ExportFormat,
    generated_by: Option<String>,
    landscape: bool,
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
            let (shop_name, shop_address) = shop_identity(&state.pool).await;
            let input = ReportPdfInput {
                title: title.to_string(),
                shop_name,
                shop_address,
                filter_summary,
                generated_at,
                generated_by,
                landscape,
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

async fn export_purchases(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let rows = sqlx::query(
        "SELECT p.purchase_date, p.purchase_number, s.name, p.total_minor, p.paid_minor,
                p.due_minor, p.status
         FROM purchases p
         LEFT JOIN suppliers s ON s.id = p.supplier_id
         WHERE p.purchase_date >= ?1 AND p.purchase_date <= ?2
         ORDER BY p.purchase_date DESC, p.id DESC
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
            header: "Purchase #".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Supplier".into(),
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

    let mut data_rows = Vec::new();
    let mut total_val: i64 = 0;

    for r in &rows {
        let date: String = r.get(0);
        let num: Option<String> = r.get(1);
        let sup: Option<String> = r.get(2);
        let total: i64 = r.get(3);
        let paid: i64 = r.get(4);
        let due: i64 = r.get(5);
        let status: String = r.get(6);

        total_val += total;

        data_rows.push(vec![
            date,
            num.unwrap_or_default(),
            sup.unwrap_or_default(),
            format_minor(total),
            format_minor(paid),
            format_minor(due),
            status,
        ]);
    }

    let totals = Some(vec![
        String::new(),
        String::new(),
        "TOTAL".into(),
        format_minor(total_val),
        String::new(),
        String::new(),
        String::new(),
    ]);

    export_to_file(
        state,
        "Purchases Report",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_returns(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);

    let rows = sqlx::query(
        "SELECT r.return_date, 'Customer Return', r.return_number, s.sale_number, r.total_refund_minor, r.status
         FROM sales_returns r
         LEFT JOIN sales s ON s.id = r.sale_id
         WHERE r.return_date >= ?1 AND r.return_date <= ?2
         UNION ALL
         SELECT r.return_date, 'Supplier Return', r.return_number, p.purchase_number, r.total_minor, r.status
         FROM supplier_returns r
         LEFT JOIN purchases p ON p.id = r.purchase_id
         WHERE r.return_date >= ?1 AND r.return_date <= ?2
         ORDER BY 1 DESC
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
            header: "Type".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Return #".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Ref #".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Amount".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Status".into(),
            width_ratio: 1.5,
            align_left: true,
        },
    ];

    let mut data_rows = Vec::new();
    let mut total_val: i64 = 0;

    for r in &rows {
        let date: String = r.get(0);
        let rtype: String = r.get(1);
        let num: Option<String> = r.get(2);
        let ref_num: Option<String> = r.get(3);
        let total: i64 = r.get(4);
        let status: String = r.get(5);

        total_val += total;

        data_rows.push(vec![
            date,
            rtype,
            num.unwrap_or_default(),
            ref_num.unwrap_or_default(),
            format_minor(total),
            status,
        ]);
    }

    let totals = Some(vec![
        String::new(),
        String::new(),
        String::new(),
        "TOTAL".into(),
        format_minor(total_val),
        String::new(),
    ]);

    export_to_file(
        state,
        "Returns Report",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_deliveries_and_damage(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);

    let rows = sqlx::query(
        "SELECT d.delivery_date, 'Delivery', d.delivery_number, d.status, d.status
         FROM deliveries d
         WHERE d.delivery_date >= ?1 AND d.delivery_date <= ?2
         UNION ALL
         SELECT SUBSTR(d.created_at, 1, 10), 'Damage', p.name, d.reason, d.status
         FROM damage_records d
         LEFT JOIN products p ON p.id = d.product_id
         WHERE SUBSTR(d.created_at, 1, 10) >= ?1 AND SUBSTR(d.created_at, 1, 10) <= ?2
         ORDER BY 1 DESC
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
            header: "Type".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Ref/Product".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Details".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Status".into(),
            width_ratio: 1.5,
            align_left: true,
        },
    ];

    let mut data_rows = Vec::new();

    for r in &rows {
        let date: String = r.get(0);
        let rtype: String = r.get(1);
        let ref_num: Option<String> = r.get(2);
        let details: Option<String> = r.get(3);
        let status: String = r.get(4);

        data_rows.push(vec![
            date,
            rtype,
            ref_num.unwrap_or_default(),
            details.unwrap_or_default(),
            status,
        ]);
    }

    export_to_file(
        state,
        "Deliveries and Damage",
        filter,
        &columns,
        data_rows,
        None,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_audit_log(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);

    let rows = sqlx::query(
        "SELECT a.created_at, u.username, a.action, a.target_type, a.details_json
         FROM audit_logs a
         LEFT JOIN users u ON u.id = a.user_id
         WHERE SUBSTR(a.created_at, 1, 10) >= ?1 AND SUBSTR(a.created_at, 1, 10) <= ?2
         ORDER BY a.created_at DESC
         LIMIT 1000",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Time".into(),
            width_ratio: 2.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "User".into(),
            width_ratio: 1.5,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Action".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Target".into(),
            width_ratio: 1.5,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Details".into(),
            width_ratio: 3.0,
            align_left: true,
        },
    ];

    let mut data_rows = Vec::new();

    for r in &rows {
        let time: String = r.get(0);
        let user: Option<String> = r.get(1);
        let action: String = r.get(2);
        let target: String = r.get(3);
        let mut details: Option<String> = r.get(4);

        if let Some(d) = details.as_ref() {
            if d.len() > 50 {
                details = Some(format!("{}...", &d[..50]));
            }
        }

        data_rows.push(vec![
            time,
            user.unwrap_or_else(|| "System".into()),
            action,
            target,
            details.unwrap_or_default(),
        ]);
    }

    export_to_file(
        state,
        "Audit Log",
        filter,
        &columns,
        data_rows,
        None,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_sales_by_product(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let rows = sqlx::query(
        "SELECT si.article_number, si.product_name, SUM(si.quantity) as total_qty,
                SUM(si.line_total_minor) as total_revenue, SUM(si.line_cost_minor) as total_cost
         FROM sale_items si
         JOIN sales s ON s.id = si.sale_id
         WHERE s.sale_date >= ?1 AND s.sale_date <= ?2 AND s.status = 'confirmed'
         GROUP BY si.article_number, si.product_name
         ORDER BY total_revenue DESC
         LIMIT 1000",
    )
    .bind(from)
    .bind(to)
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
            header: "Qty Sold".into(),
            width_ratio: 1.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Revenue".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Cost".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Profit".into(),
            width_ratio: 2.0,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_qty: i64 = 0;
    let mut total_revenue: i64 = 0;
    let mut total_cost: i64 = 0;

    for r in &rows {
        let article: String = r.get(0);
        let name: String = r.get(1);
        let qty: i64 = r.get(2);
        let revenue: i64 = r.get(3);
        let cost: i64 = r.get(4);
        let profit = revenue - cost;

        total_qty += qty;
        total_revenue += revenue;
        total_cost += cost;

        data_rows.push(vec![
            article,
            name,
            qty.to_string(),
            format_minor(revenue),
            format_minor(cost),
            format_minor(profit),
        ]);
    }

    let totals = Some(vec![
        String::new(),
        "TOTAL".into(),
        total_qty.to_string(),
        format_minor(total_revenue),
        format_minor(total_cost),
        format_minor(total_revenue - total_cost),
    ]);

    export_to_file(
        state,
        "Sales by Product",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_sales_by_customer(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let rows = sqlx::query(
        "SELECT COALESCE(s.customer_name, 'Walk-in') as cust_name, COUNT(*) as sale_count,
                SUM(s.total_minor) as total_sales, SUM(s.paid_minor) as total_paid,
                SUM(s.due_minor) as total_due
         FROM sales s
         WHERE s.sale_date >= ?1 AND s.sale_date <= ?2 AND s.status = 'confirmed'
         GROUP BY cust_name
         ORDER BY total_sales DESC
         LIMIT 1000",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Customer".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Sales Count".into(),
            width_ratio: 1.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Total Sales".into(),
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
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_count: i64 = 0;
    let mut total_sales: i64 = 0;
    let mut total_paid: i64 = 0;
    let mut total_due: i64 = 0;

    for r in &rows {
        let cust: String = r.get(0);
        let count: i64 = r.get(1);
        let sales: i64 = r.get(2);
        let paid: i64 = r.get(3);
        let due: i64 = r.get(4);

        total_count += count;
        total_sales += sales;
        total_paid += paid;
        total_due += due;

        data_rows.push(vec![
            cust,
            count.to_string(),
            format_minor(sales),
            format_minor(paid),
            format_minor(due),
        ]);
    }

    let totals = Some(vec![
        "TOTAL".into(),
        total_count.to_string(),
        format_minor(total_sales),
        format_minor(total_paid),
        format_minor(total_due),
    ]);

    export_to_file(
        state,
        "Sales by Customer",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_sales_by_category(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let rows = sqlx::query(
        "SELECT c.name as category_name, SUM(si.quantity) as total_qty,
                SUM(si.line_total_minor) as total_revenue
         FROM sale_items si
         JOIN sales s ON s.id = si.sale_id
         JOIN products p ON p.id = si.product_id
         JOIN categories c ON c.id = p.category_id
         WHERE s.sale_date >= ?1 AND s.sale_date <= ?2 AND s.status = 'confirmed'
         GROUP BY c.name
         ORDER BY total_revenue DESC
         LIMIT 1000",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Category".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Qty Sold".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Revenue".into(),
            width_ratio: 2.0,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_qty: i64 = 0;
    let mut total_revenue: i64 = 0;

    for r in &rows {
        let category: String = r.get(0);
        let qty: i64 = r.get(1);
        let revenue: i64 = r.get(2);

        total_qty += qty;
        total_revenue += revenue;

        data_rows.push(vec![category, qty.to_string(), format_minor(revenue)]);
    }

    let totals = Some(vec![
        "TOTAL".into(),
        total_qty.to_string(),
        format_minor(total_revenue),
    ]);

    export_to_file(
        state,
        "Sales by Category",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_best_slow_sellers(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let rows = sqlx::query(
        "SELECT si.article_number, si.product_name, SUM(si.quantity) as total_qty,
                SUM(si.line_total_minor) as total_revenue
         FROM sale_items si
         JOIN sales s ON s.id = si.sale_id
         WHERE s.sale_date >= ?1 AND s.sale_date <= ?2 AND s.status = 'confirmed'
         GROUP BY si.article_number, si.product_name
         ORDER BY total_qty DESC
         LIMIT 50",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Rank".into(),
            width_ratio: 1.0,
            align_left: false,
        },
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
            header: "Qty Sold".into(),
            width_ratio: 1.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Revenue".into(),
            width_ratio: 2.0,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_qty: i64 = 0;
    let mut total_revenue: i64 = 0;

    for (i, r) in rows.iter().enumerate() {
        let article: String = r.get(0);
        let name: String = r.get(1);
        let qty: i64 = r.get(2);
        let revenue: i64 = r.get(3);

        total_qty += qty;
        total_revenue += revenue;

        data_rows.push(vec![
            (i + 1).to_string(),
            article,
            name,
            qty.to_string(),
            format_minor(revenue),
        ]);
    }

    let totals = Some(vec![
        String::new(),
        String::new(),
        "TOTAL".into(),
        total_qty.to_string(),
        format_minor(total_revenue),
    ]);

    export_to_file(
        state,
        "Best & Slow Sellers",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_stock_movements(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let rows = sqlx::query(
        "SELECT sm.created_at, sm.movement_type, p.article_number, p.name,
                l.name as location, sm.quantity_delta, sm.reason, u.username
         FROM stock_movements sm
         JOIN products p ON p.id = sm.product_id
         JOIN locations l ON l.id = sm.location_id
         JOIN users u ON u.id = sm.created_by
         WHERE sm.created_at >= ?1 AND sm.created_at <= ?2
         ORDER BY sm.created_at DESC
         LIMIT 1000",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Date/Time".into(),
            width_ratio: 2.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Type".into(),
            width_ratio: 1.5,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Article".into(),
            width_ratio: 1.5,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Product".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Location".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Qty Delta".into(),
            width_ratio: 1.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Reason".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "User".into(),
            width_ratio: 1.5,
            align_left: true,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();

    for r in &rows {
        let datetime: String = r.get(0);
        let mtype: String = r.get(1);
        let article: String = r.get(2);
        let name: String = r.get(3);
        let location: String = r.get(4);
        let qty_delta: i64 = r.get(5);
        let reason: Option<String> = r.get(6);
        let user: String = r.get(7);

        data_rows.push(vec![
            datetime,
            mtype,
            article,
            name,
            location,
            qty_delta.to_string(),
            reason.unwrap_or_default(),
            user,
        ]);
    }

    export_to_file(
        state,
        "Stock Movement Ledger",
        filter,
        &columns,
        data_rows,
        None,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_low_stock(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let rows = sqlx::query(
        "SELECT p.article_number, p.name, c.name as category,
                COALESCE(SUM(sb.on_hand - sb.reserved - sb.damaged), 0) as available,
                p.minimum_stock
         FROM products p
         LEFT JOIN stock_balances sb ON sb.product_id = p.id
         LEFT JOIN categories c ON c.id = p.category_id
         WHERE p.archived_at IS NULL AND p.track_stock = 1
         GROUP BY p.id
         HAVING available <= p.minimum_stock
         ORDER BY available ASC, p.name
         LIMIT 1000",
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
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Category".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Available Stock".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Min Stock".into(),
            width_ratio: 1.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Status".into(),
            width_ratio: 2.0,
            align_left: true,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();

    for r in &rows {
        let article: String = r.get(0);
        let name: String = r.get(1);
        let category: Option<String> = r.get(2);
        let available: i64 = r.get(3);
        let min_stock: i64 = r.get(4);

        let status = if available == 0 {
            "Out of Stock"
        } else {
            "Low Stock"
        };

        data_rows.push(vec![
            article,
            name,
            category.unwrap_or_default(),
            available.to_string(),
            min_stock.to_string(),
            status.into(),
        ]);
    }

    export_to_file(
        state,
        "Low / Out of Stock",
        filter,
        &columns,
        data_rows,
        None,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_stock_by_location(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let rows = sqlx::query(
        "SELECT l.name as location, p.article_number, p.name,
                sb.on_hand, sb.reserved, sb.damaged,
                (sb.on_hand - sb.reserved - sb.damaged) as available
         FROM stock_balances sb
         JOIN products p ON p.id = sb.product_id
         JOIN locations l ON l.id = sb.location_id
         WHERE p.archived_at IS NULL
         ORDER BY l.name, p.name
         LIMIT 1000",
    )
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Location".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Article".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Product".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "On Hand".into(),
            width_ratio: 1.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Reserved".into(),
            width_ratio: 1.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Damaged".into(),
            width_ratio: 1.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Available".into(),
            width_ratio: 1.5,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();

    for r in &rows {
        let location: String = r.get(0);
        let article: String = r.get(1);
        let name: String = r.get(2);
        let on_hand: i64 = r.get(3);
        let reserved: i64 = r.get(4);
        let damaged: i64 = r.get(5);
        let available: i64 = r.get(6);

        data_rows.push(vec![
            location,
            article,
            name,
            on_hand.to_string(),
            reserved.to_string(),
            damaged.to_string(),
            available.to_string(),
        ]);
    }

    export_to_file(
        state,
        "Stock by Location",
        filter,
        &columns,
        data_rows,
        None,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_customer_statements(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let rows = sqlx::query(
        "SELECT c.name, c.phone, c.code,
                COALESCE(SUM(CASE WHEN ce.entry_type IN ('sale','opening_balance')
                                  THEN ce.amount_minor ELSE 0 END), 0) as total_due,
                COALESCE(SUM(CASE WHEN ce.entry_type IN ('payment','advance_used')
                                  THEN -ce.amount_minor ELSE 0 END), 0) as total_paid,
                c.opening_balance_minor
         FROM customers c
         LEFT JOIN customer_ledger_entries ce ON ce.customer_id = c.id
         WHERE c.is_active = 1
         GROUP BY c.id
         HAVING total_due > 0 OR total_paid > 0
         ORDER BY total_due DESC
         LIMIT 1000",
    )
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Code".into(),
            width_ratio: 1.5,
            align_left: true,
        },
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
            header: "Total Due".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Total Paid".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Opening Balance".into(),
            width_ratio: 2.0,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_due: i64 = 0;
    let mut total_paid: i64 = 0;

    for r in &rows {
        let name: String = r.get(0);
        let phone: Option<String> = r.get(1);
        let code: Option<String> = r.get(2);
        let due: i64 = r.get(3);
        let paid: i64 = r.get(4);
        let opening: i64 = r.get(5);

        total_due += due;
        total_paid += paid;

        data_rows.push(vec![
            code.unwrap_or_default(),
            name,
            phone.unwrap_or_default(),
            format_minor(due),
            format_minor(paid),
            format_minor(opening),
        ]);
    }

    let totals = Some(vec![
        String::new(),
        "TOTAL".into(),
        String::new(),
        format_minor(total_due),
        format_minor(total_paid),
        String::new(),
    ]);

    export_to_file(
        state,
        "Customer Statements",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_customer_receipts(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let rows = sqlx::query(
        "SELECT cp.receipt_number, cp.payment_date, c.name as customer_name,
                pm.name as method, cp.amount_minor, cp.advance_alloc_minor, cp.status
         FROM customer_payments cp
         JOIN customers c ON c.id = cp.customer_id
         JOIN payment_methods pm ON pm.id = cp.payment_method_id
         WHERE cp.payment_date >= ?1 AND cp.payment_date <= ?2
         ORDER BY cp.payment_date DESC
         LIMIT 1000",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Receipt #".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Date".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Customer".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Method".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Amount".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Advance".into(),
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
    let mut total_amount: i64 = 0;
    let mut total_advance: i64 = 0;

    for r in &rows {
        let receipt: Option<String> = r.get(0);
        let date: String = r.get(1);
        let customer: String = r.get(2);
        let method: String = r.get(3);
        let amount: i64 = r.get(4);
        let advance: i64 = r.get(5);
        let status: String = r.get(6);

        total_amount += amount;
        total_advance += advance;

        data_rows.push(vec![
            receipt.unwrap_or_default(),
            date,
            customer,
            method,
            format_minor(amount),
            format_minor(advance),
            status,
        ]);
    }

    let totals = Some(vec![
        String::new(),
        String::new(),
        "TOTAL".into(),
        String::new(),
        format_minor(total_amount),
        format_minor(total_advance),
        String::new(),
    ]);

    export_to_file(
        state,
        "Customer Receipts",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_customer_advances(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let rows = sqlx::query(
        "SELECT c.code, c.name, c.phone, c.advance_minor
         FROM customers c
         WHERE c.is_active = 1 AND c.advance_minor > 0
         ORDER BY c.advance_minor DESC
         LIMIT 1000",
    )
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Code".into(),
            width_ratio: 1.5,
            align_left: true,
        },
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
            header: "Advance Balance".into(),
            width_ratio: 2.0,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_advance: i64 = 0;

    for r in &rows {
        let code: Option<String> = r.get(0);
        let name: String = r.get(1);
        let phone: Option<String> = r.get(2);
        let advance: i64 = r.get(3);

        total_advance += advance;

        data_rows.push(vec![
            code.unwrap_or_default(),
            name,
            phone.unwrap_or_default(),
            format_minor(advance),
        ]);
    }

    let totals = Some(vec![
        String::new(),
        "TOTAL".into(),
        String::new(),
        format_minor(total_advance),
    ]);

    export_to_file(
        state,
        "Customer Advances",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_credit_limit_exceptions(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let rows = sqlx::query(
        "SELECT c.code, c.name, c.phone, c.credit_limit_minor, c.opening_balance_minor,
                COALESCE(SUM(ce.amount_minor), 0) as current_balance
         FROM customers c
         LEFT JOIN customer_ledger_entries ce ON ce.customer_id = c.id
         WHERE c.is_active = 1 AND c.credit_limit_minor > 0
         GROUP BY c.id
         HAVING current_balance > c.credit_limit_minor
         ORDER BY (current_balance - c.credit_limit_minor) DESC
         LIMIT 1000",
    )
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Code".into(),
            width_ratio: 1.5,
            align_left: true,
        },
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
            header: "Credit Limit".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Current Balance".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Over Limit".into(),
            width_ratio: 2.0,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();

    for r in &rows {
        let code: Option<String> = r.get(0);
        let name: String = r.get(1);
        let phone: Option<String> = r.get(2);
        let limit: i64 = r.get(3);
        let _opening: i64 = r.get(4);
        let current: i64 = r.get(5);
        let over = current - limit;

        data_rows.push(vec![
            code.unwrap_or_default(),
            name,
            phone.unwrap_or_default(),
            format_minor(limit),
            format_minor(current),
            format_minor(over),
        ]);
    }

    export_to_file(
        state,
        "Credit-Limit Exceptions",
        filter,
        &columns,
        data_rows,
        None,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_supplier_statements(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let rows = sqlx::query(
        "SELECT s.code, s.name, s.phone,
                COALESCE(SUM(CASE WHEN se.entry_type IN ('invoice','opening_balance')
                                  THEN se.amount_minor ELSE 0 END), 0) as total_payable,
                COALESCE(SUM(CASE WHEN se.entry_type = 'payment'
                                  THEN -se.amount_minor ELSE 0 END), 0) as total_paid
         FROM suppliers s
         LEFT JOIN supplier_ledger_entries se ON se.supplier_id = s.id
         WHERE s.is_active = 1
         GROUP BY s.id
         HAVING total_payable > 0 OR total_paid > 0
         ORDER BY total_payable DESC
         LIMIT 1000",
    )
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Code".into(),
            width_ratio: 1.5,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Supplier".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Phone".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Total Payable".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Total Paid".into(),
            width_ratio: 2.0,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_payable: i64 = 0;
    let mut total_paid: i64 = 0;

    for r in &rows {
        let code: Option<String> = r.get(0);
        let name: String = r.get(1);
        let phone: Option<String> = r.get(2);
        let payable: i64 = r.get(3);
        let paid: i64 = r.get(4);

        total_payable += payable;
        total_paid += paid;

        data_rows.push(vec![
            code.unwrap_or_default(),
            name,
            phone.unwrap_or_default(),
            format_minor(payable),
            format_minor(paid),
        ]);
    }

    let totals = Some(vec![
        String::new(),
        "TOTAL".into(),
        String::new(),
        format_minor(total_payable),
        format_minor(total_paid),
    ]);

    export_to_file(
        state,
        "Supplier Statements",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_supplier_payments(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let rows = sqlx::query(
        "SELECT sp.payment_number, sp.payment_date, s.name as supplier_name,
                pm.name as method, sp.amount_minor, sp.status
         FROM supplier_payments sp
         JOIN suppliers s ON s.id = sp.supplier_id
         JOIN payment_methods pm ON pm.id = sp.payment_method_id
         WHERE sp.payment_date >= ?1 AND sp.payment_date <= ?2
         ORDER BY sp.payment_date DESC
         LIMIT 1000",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Payment #".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Date".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Supplier".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Method".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Amount".into(),
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
    let mut total_amount: i64 = 0;

    for r in &rows {
        let number: Option<String> = r.get(0);
        let date: String = r.get(1);
        let supplier: String = r.get(2);
        let method: String = r.get(3);
        let amount: i64 = r.get(4);
        let status: String = r.get(5);

        total_amount += amount;

        data_rows.push(vec![
            number.unwrap_or_default(),
            date,
            supplier,
            method,
            format_minor(amount),
            status,
        ]);
    }

    let totals = Some(vec![
        String::new(),
        String::new(),
        "TOTAL".into(),
        String::new(),
        format_minor(total_amount),
        String::new(),
    ]);

    export_to_file(
        state,
        "Supplier Payments",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_cash_book(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let rows = sqlx::query(
        "SELECT ce.created_at, ca.name as account, ce.entry_type, ce.amount_minor,
                ce.reference_type, ce.reference_id, ce.reason, u.username
         FROM cash_entries ce
         JOIN cash_accounts ca ON ca.id = ce.cash_account_id
         JOIN users u ON u.id = ce.created_by
         WHERE ce.created_at >= ?1 AND ce.created_at <= ?2
         ORDER BY ce.created_at DESC
         LIMIT 1000",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Date/Time".into(),
            width_ratio: 2.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Account".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Type".into(),
            width_ratio: 1.5,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Amount".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Reference".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Reason".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "User".into(),
            width_ratio: 1.5,
            align_left: true,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();

    for r in &rows {
        let datetime: String = r.get(0);
        let account: String = r.get(1);
        let etype: String = r.get(2);
        let amount: i64 = r.get(3);
        let ref_type: Option<String> = r.get(4);
        let ref_id: Option<i64> = r.get(5);
        let reason: Option<String> = r.get(6);
        let user: String = r.get(7);

        let reference = match (ref_type, ref_id) {
            (Some(rt), Some(ri)) => format!("{rt} #{ri}"),
            (Some(rt), None) => rt,
            _ => String::new(),
        };

        data_rows.push(vec![
            datetime,
            account,
            etype,
            format_minor(amount),
            reference,
            reason.unwrap_or_default(),
            user,
        ]);
    }

    export_to_file(
        state,
        "Cash Book",
        filter,
        &columns,
        data_rows,
        None,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_account_balances(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let rows = sqlx::query(
        "SELECT ca.code, ca.name, ca.kind, ca.opening_balance_minor, ca.balance_minor
         FROM cash_accounts ca
         WHERE ca.is_active = 1
         ORDER BY ca.name",
    )
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "Code".into(),
            width_ratio: 1.5,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Account".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Kind".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Opening Balance".into(),
            width_ratio: 2.0,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Current Balance".into(),
            width_ratio: 2.0,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();

    for r in &rows {
        let code: Option<String> = r.get(0);
        let name: String = r.get(1);
        let kind: String = r.get(2);
        let opening: i64 = r.get(3);
        let balance: i64 = r.get(4);

        data_rows.push(vec![
            code.unwrap_or_default(),
            name,
            kind,
            format_minor(opening),
            format_minor(balance),
        ]);
    }

    export_to_file(
        state,
        "Account Balances",
        filter,
        &columns,
        data_rows,
        None,
        format,
        generated_by,
        false,
    )
    .await
}

async fn export_user_activity(
    state: &AppState,
    filter: &ReportFilterInput,
    format: ExportFormat,
    generated_by: Option<String>,
) -> Result<ReportExportResult, AppError> {
    let (from, to) = date_range_filter(filter);
    let rows = sqlx::query(
        "SELECT u.username, a.action, COUNT(*) as action_count,
                MIN(a.created_at) as first_action, MAX(a.created_at) as last_action
         FROM audit_logs a
         JOIN users u ON u.id = a.user_id
         WHERE a.created_at >= ?1 AND a.created_at <= ?2
         GROUP BY u.username, a.action
         ORDER BY action_count DESC
         LIMIT 1000",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.pool)
    .await?;

    let columns = vec![
        ReportPdfColumn {
            header: "User".into(),
            width_ratio: 2.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Action".into(),
            width_ratio: 3.0,
            align_left: true,
        },
        ReportPdfColumn {
            header: "Count".into(),
            width_ratio: 1.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "First Action".into(),
            width_ratio: 2.5,
            align_left: false,
        },
        ReportPdfColumn {
            header: "Last Action".into(),
            width_ratio: 2.5,
            align_left: false,
        },
    ];

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut total_count: i64 = 0;

    for r in &rows {
        let user: String = r.get(0);
        let action: String = r.get(1);
        let count: i64 = r.get(2);
        let first: String = r.get(3);
        let last: String = r.get(4);

        total_count += count;

        data_rows.push(vec![user, action, count.to_string(), first, last]);
    }

    let totals = Some(vec![
        String::new(),
        "TOTAL".into(),
        total_count.to_string(),
        String::new(),
        String::new(),
    ]);

    export_to_file(
        state,
        "User Activity",
        filter,
        &columns,
        data_rows,
        totals,
        format,
        generated_by,
        false,
    )
    .await
}
