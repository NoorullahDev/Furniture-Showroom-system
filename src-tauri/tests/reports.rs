use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application;
use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::dto::expenses::ExpenseInput;
use furniture_shop_lib::dto::purchases::{
    CashAccountInput, PurchaseCreateInput, PurchaseItemInput, PurchasePostInput, SupplierInput,
};
use furniture_shop_lib::dto::reports::{ExportFormat, ReportFilterInput};
use furniture_shop_lib::dto::sales::{
    CustomerInput, SaleConfirmInput, SaleCreateInput, SaleItemInput,
};
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-reports-{label}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

async fn open_state(dir: &std::path::Path) -> AppState {
    let paths = infra::FilePaths::init(dir).unwrap();
    let (pool, _info) = infra::db::open(&paths).await.unwrap();
    AppState::new(pool, paths)
}

async fn make_owner(state: &AppState, username: &str) -> Principal {
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, full_name) VALUES (?, 'x', ?) RETURNING id",
    )
    .bind(username)
    .bind(username)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    let role_id: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE code = 'owner'")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
        .bind(user_id)
        .bind(role_id)
        .execute(&state.pool)
        .await
        .unwrap();
    let permissions: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT p.code
           FROM role_permissions rp
           JOIN permissions p ON p.id = rp.permission_id
          WHERE rp.role_id = ?
          ORDER BY p.code",
    )
    .bind(role_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    Principal {
        session_id: format!("sess-{username}"),
        user_id,
        username: username.to_string(),
        full_name: username.to_string(),
        roles: vec!["owner".to_string()],
        permissions,
    }
}

async fn create_product(state: &AppState, article: &str) -> i64 {
    let category_id: i64 = sqlx::query_scalar(
        "INSERT INTO categories (name, created_at, updated_at)
             VALUES (?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) RETURNING id",
    )
    .bind(article)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO products (
            article_number, article_number_norm, name, category_id, sale_price_minor,
            minimum_stock, track_stock, created_at, updated_at
         ) VALUES (?, UPPER(?), ?, ?, 10000, 0, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .bind(article)
    .bind(article)
    .bind(format!("Product {article}"))
    .bind(category_id)
    .execute(&state.pool)
    .await
    .unwrap();
    sqlx::query_scalar("SELECT id FROM products WHERE article_number = ?")
        .bind(article)
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

async fn main_location(state: &AppState) -> i64 {
    sqlx::query_scalar("SELECT id FROM locations WHERE name = 'Main Showroom'")
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

async fn funded_cash(state: &AppState, owner: &Principal, code: &str) -> i64 {
    application::cash::create_cash_account(
        state,
        owner,
        CashAccountInput {
            code: code.into(),
            name: format!("Test cash {code}"),
            kind: Some("cash".into()),
            opening_balance_minor: Some(1_000_000),
        },
        "corr-cash",
    )
    .await
    .unwrap()
    .id
}

async fn stock_product(
    state: &AppState,
    owner: &Principal,
    article: &str,
    qty: i64,
    cost: i64,
) -> i64 {
    let product = create_product(state, article).await;
    let supplier_id = application::suppliers::create(
        state,
        owner,
        SupplierInput {
            code: format!("SUP-{article}"),
            name: format!("Supplier {article}"),
            phone: None,
            email: None,
            address: None,
            opening_balance_minor: Some(0),
            is_active: Some(true),
        },
        "corr-sup",
    )
    .await
    .unwrap()
    .id;
    let location = main_location(state).await;
    let draft = PurchaseCreateInput {
        supplier_id,
        location_id: location,
        invoice_number: format!("INV-{article}"),
        invoice_date: "2026-09-01".into(),
        purchase_date: Some("2026-09-01".into()),
        notes: None,
        items: vec![PurchaseItemInput {
            product_id: product,
            quantity: qty,
            unit_cost_minor: cost,
        }],
    };
    let purchase = application::purchases::create_purchase(state, owner, draft, "corr-p")
        .await
        .unwrap();
    application::purchases::post_purchase(
        state,
        owner,
        PurchasePostInput {
            purchase_id: purchase.id,
            idempotency_key: Some(format!("sp-{article}")),
            paid_minor: Some(0),
            cash_account_id: None,
            payment_method_id: None,
        },
        "corr-ppost",
    )
    .await
    .unwrap();
    product
}

async fn create_customer(state: &AppState, owner: &Principal, code: &str) -> i64 {
    application::customers::create(
        state,
        owner,
        CustomerInput {
            code: code.into(),
            name: format!("Customer {code}"),
            phone: None,
            email: None,
            address: None,
            credit_limit_minor: Some(500_000),
            credit_days: None,
            opening_balance_minor: Some(0),
            is_active: Some(true),
        },
        "corr-cust",
    )
    .await
    .unwrap()
    .id
}

async fn set_price(state: &AppState, product_id: i64, price: i64) {
    sqlx::query("UPDATE products SET sale_price_minor = ? WHERE id = ?")
        .bind(price)
        .bind(product_id)
        .execute(&state.pool)
        .await
        .unwrap();
}

async fn payment_method(state: &AppState) -> i64 {
    sqlx::query_scalar("SELECT id FROM payment_methods WHERE code = 'cash'")
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sales_summary_csv_export_empty() {
    let dir = temp_dir("sales-csv-empty");
    let state = open_state(&dir).await;
    let _owner = make_owner(&state, "owner").await;

    let result = application::reports::export_report(
        &state,
        "sales_summary",
        &ReportFilterInput {
            from_date: None,
            to_date: None,
            cash_account_id: None,
            category_id: None,
            status: None,
        },
        ExportFormat::Csv,
    )
    .await
    .unwrap();

    assert_eq!(result.row_count, 0);
    assert!(result.report_path.ends_with(".csv"));
    let content = fs::read_to_string(&result.report_path).unwrap();
    assert!(content.contains("Sales Summary"));
    assert!(content.contains("All time"));

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sales_summary_pdf_export_empty() {
    let dir = temp_dir("sales-pdf-empty");
    let state = open_state(&dir).await;
    let _owner = make_owner(&state, "owner").await;

    let result = application::reports::export_report(
        &state,
        "sales_summary",
        &ReportFilterInput {
            from_date: None,
            to_date: None,
            cash_account_id: None,
            category_id: None,
            status: None,
        },
        ExportFormat::Pdf,
    )
    .await
    .unwrap();

    assert_eq!(result.row_count, 0);
    assert!(result.report_path.ends_with(".pdf"));
    let meta = fs::metadata(&result.report_path).unwrap();
    assert!(meta.len() > 0);

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sales_summary_with_data() {
    let dir = temp_dir("sales-data");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let product = stock_product(&state, &owner, "RPT-01", 10, 5000).await;
    set_price(&state, product, 15_000).await;
    let location = main_location(&state).await;
    let customer = create_customer(&state, &owner, "C-RPT").await;
    let cash = funded_cash(&state, &owner, "RPTCASH").await;
    let pm = payment_method(&state).await;

    // Create and confirm a sale
    let sale = application::sales::create_sale(
        &state,
        &owner,
        SaleCreateInput {
            location_id: location,
            customer_id: Some(customer),
            kind: Some("sale".into()),
            sale_date: Some("2026-09-05".into()),
            discount_minor: Some(0),
            delivery_charge_minor: Some(0),
            notes: None,
            items: vec![SaleItemInput {
                product_id: Some(product),
                bundle_id: None,
                quantity: 2,
            }],
        },
        "corr-sale",
    )
    .await
    .unwrap();
    application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: Some("rpt-sale-1".into()),
            paid_minor: Some(30_000),
            cash_account_id: Some(cash),
            payment_method_id: Some(pm),
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-confirm",
    )
    .await
    .unwrap();

    // Export with date filter covering the sale
    let result = application::reports::export_report(
        &state,
        "sales_summary",
        &ReportFilterInput {
            from_date: Some("2026-09-01".into()),
            to_date: Some("2026-09-30".into()),
            cash_account_id: None,
            category_id: None,
            status: None,
        },
        ExportFormat::Csv,
    )
    .await
    .unwrap();

    assert_eq!(result.row_count, 1);
    let content = fs::read_to_string(&result.report_path).unwrap();
    assert!(content.contains("RPT"));
    assert!(content.contains("Customer C-RPT"));

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn stock_valuation_export() {
    let dir = temp_dir("stock-val");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    stock_product(&state, &owner, "VAL-01", 5, 8000).await;
    stock_product(&state, &owner, "VAL-02", 3, 12000).await;

    let result = application::reports::export_report(
        &state,
        "stock_valuation",
        &ReportFilterInput {
            from_date: None,
            to_date: None,
            cash_account_id: None,
            category_id: None,
            status: None,
        },
        ExportFormat::Csv,
    )
    .await
    .unwrap();

    assert_eq!(result.row_count, 2);
    let content = fs::read_to_string(&result.report_path).unwrap();
    assert!(content.contains("VAL-01"));
    assert!(content.contains("VAL-02"));

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn customer_dues_export() {
    let dir = temp_dir("cust-dues");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let product = stock_product(&state, &owner, "DUE-01", 5, 5000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let customer = create_customer(&state, &owner, "DUE-C").await;

    let sale = application::sales::create_sale(
        &state,
        &owner,
        SaleCreateInput {
            location_id: location,
            customer_id: Some(customer),
            kind: Some("sale".into()),
            sale_date: Some("2026-09-10".into()),
            discount_minor: Some(0),
            delivery_charge_minor: Some(0),
            notes: None,
            items: vec![SaleItemInput {
                product_id: Some(product),
                bundle_id: None,
                quantity: 1,
            }],
        },
        "corr-due",
    )
    .await
    .unwrap();
    application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: Some("due-1".into()),
            paid_minor: Some(0),
            cash_account_id: None,
            payment_method_id: None,
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-due-c",
    )
    .await
    .unwrap();

    let result = application::reports::export_report(
        &state,
        "customer_dues",
        &ReportFilterInput {
            from_date: None,
            to_date: None,
            cash_account_id: None,
            category_id: None,
            status: None,
        },
        ExportFormat::Csv,
    )
    .await
    .unwrap();

    assert!(result.row_count >= 1);
    let content = fs::read_to_string(&result.report_path).unwrap();
    assert!(content.contains("Customer DUE-C"));

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn profit_loss_export() {
    let dir = temp_dir("pl");
    let state = open_state(&dir).await;
    let _owner = make_owner(&state, "owner").await;

    let result = application::reports::export_report(
        &state,
        "profit_loss",
        &ReportFilterInput {
            from_date: None,
            to_date: None,
            cash_account_id: None,
            category_id: None,
            status: None,
        },
        ExportFormat::Pdf,
    )
    .await
    .unwrap();

    assert!(result.report_path.ends_with(".pdf"));
    let meta = fs::metadata(&result.report_path).unwrap();
    assert!(meta.len() > 100);

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn expense_report_export() {
    let dir = temp_dir("exp-rpt");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let cash = funded_cash(&state, &owner, "EXPRPT").await;

    let category: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'rent'")
        .fetch_one(&state.pool)
        .await
        .unwrap();

    application::expenses::expense_post(
        &state,
        &owner,
        ExpenseInput {
            category_id: category,
            amount_minor: 25_000,
            expense_date: "2026-09-10".into(),
            cash_account_id: cash,
            description: "Office rent".into(),
            payee: Some("Landlord".into()),
            reference: None,
            attachment_path: None,
            idempotency_key: Some("exp-rpt-1".into()),
        },
        "corr-exp",
    )
    .await
    .unwrap();

    let result = application::reports::export_report(
        &state,
        "expense_report",
        &ReportFilterInput {
            from_date: Some("2026-09-01".into()),
            to_date: Some("2026-09-30".into()),
            cash_account_id: None,
            category_id: None,
            status: None,
        },
        ExportFormat::Csv,
    )
    .await
    .unwrap();

    assert_eq!(result.row_count, 1);
    let content = fs::read_to_string(&result.report_path).unwrap();
    assert!(content.contains("Office rent"));

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn supplier_payables_export() {
    let dir = temp_dir("sup-pay");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    stock_product(&state, &owner, "PAY-01", 5, 5000).await;

    let result = application::reports::export_report(
        &state,
        "supplier_payables",
        &ReportFilterInput {
            from_date: None,
            to_date: None,
            cash_account_id: None,
            category_id: None,
            status: None,
        },
        ExportFormat::Csv,
    )
    .await
    .unwrap();

    // The purchase was created with paid_minor=0, so there should be a payable row
    assert!(result.row_count >= 1);
    let content = fs::read_to_string(&result.report_path).unwrap();
    assert!(content.contains("Supplier PAY-01"));

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn unknown_report_type_returns_error() {
    let dir = temp_dir("unknown");
    let state = open_state(&dir).await;
    let _owner = make_owner(&state, "owner").await;

    let result = application::reports::export_report(
        &state,
        "nonexistent_report",
        &ReportFilterInput {
            from_date: None,
            to_date: None,
            cash_account_id: None,
            category_id: None,
            status: None,
        },
        ExportFormat::Csv,
    )
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, furniture_shop_lib::error::AppError::Validation(_)),
        "expected Validation error, got: {err:?}"
    );

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}
