use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application;
use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::dto::purchases::{
    CashAccountInput, PurchaseCreateInput, PurchaseItemInput, PurchasePostInput, SupplierInput,
};
use furniture_shop_lib::dto::sales::{
    CustomerInput, CustomerReceiptInput, SaleConfirmInput, SaleCreateInput, SaleItemInput,
};
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-dash-{label}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

async fn open_state(dir: &std::path::Path) -> AppState {
    let paths = infra::FilePaths::init(dir).unwrap();
    let (pool, _info) = infra::db::open(&paths).await.unwrap();
    AppState::new(pool, paths)
}

async fn make_role_principal(state: &AppState, username: &str, role_code: &str) -> Principal {
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, full_name) VALUES (?, 'x', ?) RETURNING id",
    )
    .bind(username)
    .bind(username)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    let role_id: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE code = ?")
        .bind(role_code)
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
        roles: vec![role_code.to_string()],
        permissions,
    }
}

async fn make_owner(state: &AppState, username: &str) -> Principal {
    make_role_principal(state, username, "owner").await
}

async fn create_product(state: &AppState, article: &str) -> i64 {
    let category_id: i64 = match sqlx::query_scalar("SELECT id FROM categories ORDER BY id LIMIT 1")
        .fetch_optional(&state.pool)
        .await
        .unwrap()
    {
        Some(id) => id,
        None => sqlx::query_scalar(
            "INSERT INTO categories (name, created_at, updated_at)
                 VALUES (?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) RETURNING id",
        )
        .bind(article)
        .fetch_one(&state.pool)
        .await
        .unwrap(),
    };
    sqlx::query(
        "INSERT INTO products (
            article_number, article_number_norm, name, category_id, sale_price_minor,
            minimum_stock, track_stock, created_at, updated_at
         ) VALUES (?, UPPER(?), ?, ?, 1000, 0, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .bind(article)
    .bind(article)
    .bind(article)
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

async fn create_supplier(state: &AppState, owner: &Principal, code: &str) -> i64 {
    application::suppliers::create(
        state,
        owner,
        SupplierInput {
            code: code.into(),
            name: format!("Supplier {code}"),
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
    .id
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
    let supplier = create_supplier(state, owner, &format!("SUP-{article}")).await;
    let location = main_location(state).await;
    let draft = PurchaseCreateInput {
        supplier_id: supplier,
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
            phone: Some(format!("0300-{}", code)),
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

async fn confirm_sale(
    state: &AppState,
    owner: &Principal,
    customer_id: i64,
    product_id: i64,
) -> i64 {
    let location = main_location(state).await;
    let sale = application::sales::create_sale(
        state,
        owner,
        SaleCreateInput {
            location_id: location,
            customer_id: Some(customer_id),
            kind: None,
            sale_date: Some("2026-09-10".into()),
            discount_minor: None,
            delivery_charge_minor: None,
            notes: None,
            items: vec![SaleItemInput {
                product_id: Some(product_id),
                bundle_id: None,
                quantity: 1,
            }],
        },
        "corr-sale",
    )
    .await
    .unwrap();
    application::sales::confirm_sale(
        state,
        owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: Some("dash-conf-1".into()),
            paid_minor: Some(0),
            cash_account_id: None,
            payment_method_id: None,
            advance_used_minor: None,
            credit_note_id: None,
        },
        "corr-confirm",
    )
    .await
    .unwrap();
    sale.id
}

// ---------------------------------------------------------------------------
// Owner sees every metric reconciled: today's sale, cash, dues, payables,
// stock value and month gross profit align with the underlying modules.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dashboard_reconciles_with_module_figures() {
    let dir = temp_dir("reconcile");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let account = funded_cash(&state, &owner, "DASH01").await;
    let _pay_method = payment_method(&state).await;

    // Stock a product at 2 units @ 300 each, and post a rent expense today.
    let product = stock_product(&state, &owner, "D-REC", 2, 300).await;
    set_price(&state, product, 2000).await;
    let customer = create_customer(&state, &owner, "D-REC").await;
    confirm_sale(&state, &owner, customer, product).await;

    // Today's expense via application service so it lands in cash + report.
    application::expenses::expense_post(
        &state,
        &owner,
        furniture_shop_lib::dto::expenses::ExpenseInput {
            category_id: 1,
            amount_minor: 40_000,
            expense_date: "2026-09-10".into(),
            cash_account_id: account,
            payment_method_id: 1,
            description: "Rent".into(),
            payee: None,
            reference: None,
            attachment_path: None,
            idempotency_key: Some("dash-rent".into()),
        },
        "corr-exp",
    )
    .await
    .unwrap();

    let s = application::dashboard::dashboard_summary(&state, &owner)
        .await
        .unwrap();

    // Today's confirmed sale (customer carries 2000 due).
    assert_eq!(s.today_sales_count, 1);
    assert_eq!(s.today_sales_minor, 2000);

    // Stock value = 1 remaining unit @ FIFO 300.
    assert_eq!(s.stock_value_minor, Some(300));

    // Dues = the customer balance after the confirmed credit sale (2000).
    assert_eq!(s.dues_minor, 2000);
    assert_eq!(s.overdue_dues_minor, 0);

    // Payables = the supplier balance left by the credit purchase (2 @ 300).
    assert_eq!(s.payables_minor, 600);

    // Net cash = funded opening (1,000,000) - rent expense (40,000)
    // (the credit sale left cash untouched).
    assert_eq!(s.net_cash_minor, 1_000_000 - 40_000);

    // Today's expense and month expenses both reflect the rent.
    assert_eq!(s.today_expenses_minor, 40_000);
    assert_eq!(s.month_expenses_minor, 40_000);

    // Month gross profit = revenue (2000) - cogs (300);
    // the rent expense is excluded from gross (it lands in operational profit).
    let gp = s.month_gross_profit_minor.expect("owner sees profit");
    assert_eq!(gp, 2000 - 300);

    // Trend has exactly 7 daily buckets.
    assert_eq!(s.trend.len(), 7);

    // Low stock is 0 (minimum_stock is 0 for our product).
    assert_eq!(s.low_stock_count, 0);

    // Activity stream does not blow up and is non-empty (owner audits).
    assert!(!s.recent_activity.is_empty());

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// A salesperson card-bearer is denied cost-derived metrics but still sees the
// operational summary: cost/profit/stock-value stay absent from the DTO.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dashboard_hides_cost_and_profit_for_unauthorized() {
    let dir = temp_dir("rbac");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let salesperson = make_role_principal(&state, "sp", "salesperson").await;

    let product = stock_product(&state, &owner, "D-RBAC", 1, 500).await;
    set_price(&state, product, 1500).await;
    let customer = create_customer(&state, &owner, "D-RBAC").await;
    confirm_sale(&state, &owner, customer, product).await;

    let s = application::dashboard::dashboard_summary(&state, &salesperson)
        .await
        .unwrap();

    // Cost-derived figures are hidden, not zeroed — owner could distinguish
    // "no stock" from "cannot see stock" by None vs Some(0).
    assert_eq!(s.stock_value_minor, None);
    assert_eq!(s.month_gross_profit_minor, None);

    // The salesperson (sale.create) still sees today's sale count/value.
    assert_eq!(s.today_sales_count, 1);
    assert_eq!(s.today_sales_minor, 1500);

    // Salesperson has no audit.view -> no activity, no error.
    assert!(s.recent_activity.is_empty());

    // Salesperson retains customer.view, so customer-facing dues are visible.
    assert_eq!(s.dues_minor, 1500);

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// A receipt posts today's cash in and the dashboard receives it exactly once.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dashboard_receives_receipts_into_today() {
    let dir = temp_dir("receipt");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let account = funded_cash(&state, &owner, "DASH-R").await;
    let pm = payment_method(&state).await;

    let product = stock_product(&state, &owner, "D-RECP", 1, 200).await;
    set_price(&state, product, 5000).await;
    let customer = create_customer(&state, &owner, "D-RECP").await;
    confirm_sale(&state, &owner, customer, product).await;

    // Collect part of the invoice (credit sale -> 5000 due), pay 2000 today.
    application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: customer,
            payment_method_id: pm,
            cash_account_id: account,
            payment_date: "2026-09-10".into(),
            amount_minor: 2000,
            notes: None,
            idempotency_key: Some("dash-recip".into()),
            allocations: None,
        },
        "corr-recip",
    )
    .await
    .unwrap();

    let s = application::dashboard::dashboard_summary(&state, &owner)
        .await
        .unwrap();
    assert_eq!(s.today_receipts_minor, 2000);
    // Dues drop to 3000 after the 2000 receipt.
    assert_eq!(s.dues_minor, 3000);

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}
