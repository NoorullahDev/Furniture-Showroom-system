use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application;
use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::dto::purchases::{
    PurchaseCreateInput, PurchaseItemInput, PurchasePostInput, SupplierInput,
};
use furniture_shop_lib::dto::sales::{
    CustomerInput, SaleConfirmInput, SaleCreateInput, SaleItemInput,
};
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-search-{label}"));
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

async fn create_customer(state: &AppState, owner: &Principal, code: &str, phone: Option<&str>) {
    application::customers::create(
        state,
        owner,
        CustomerInput {
            code: code.into(),
            name: format!("Customer {code}"),
            phone: phone.map(str::to_string),
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
    .unwrap();
}

async fn main_location(state: &AppState) -> i64 {
    sqlx::query_scalar("SELECT id FROM locations WHERE name = 'Main Showroom'")
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

async fn stock_product(
    state: &AppState,
    owner: &Principal,
    article: &str,
    qty: i64,
    cost: i64,
) -> i64 {
    let product = create_product(state, article).await;
    let supplier = application::suppliers::create(
        state,
        owner,
        SupplierInput {
            code: format!("SUP-S-{article}"),
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
        supplier_id: supplier,
        location_id: location,
        invoice_number: format!("INV-S-{article}"),
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
            idempotency_key: Some(format!("sp-s-{article}")),
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

async fn set_price(state: &AppState, product_id: i64, price: i64) {
    sqlx::query("UPDATE products SET sale_price_minor = ? WHERE id = ?")
        .bind(price)
        .bind(product_id)
        .execute(&state.pool)
        .await
        .unwrap();
}

async fn confirm_credit_sale(
    state: &AppState,
    owner: &Principal,
    customer_id: i64,
    product_id: i64,
) {
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
            idempotency_key: Some("s-search-1".into()),
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
}

// ---------------------------------------------------------------------------
// Empty and whitespace queries return an empty set immediately, no DB hit.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn global_search_empty_query_returns_empty() {
    let dir = temp_dir("empty");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;

    let r = application::search::global_search(&state, &owner, "")
        .await
        .unwrap();
    assert!(r.results.is_empty());
    assert_eq!(r.query, "");

    let r = application::search::global_search(&state, &owner, "   ")
        .await
        .unwrap();
    assert!(r.results.is_empty());

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Product by exact article match ranks first and shows article subtitle.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_products_exact_match_first() {
    let dir = temp_dir("prod-exact");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;

    stock_product(&state, &owner, "SOFA-LUX", 2, 500).await;
    stock_product(&state, &owner, "SOFA-STD", 2, 300).await;

    let r = application::search::global_search(&state, &owner, "SOFA-LUX")
        .await
        .unwrap();
    let hits: Vec<_> = r.results.iter().filter(|h| h.kind == "product").collect();
    assert!(!hits.is_empty());
    let expected_id: i64 =
        sqlx::query_scalar("SELECT id FROM products WHERE article_number = 'SOFA-LUX'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(hits[0].id, expected_id);
    assert_eq!(hits[0].rank, 0); // exact
    assert!(hits[0].subtitle.as_deref().unwrap().contains("SOFA-LUX"));
    // No cost/profit leaks in the search DTO.
    let payload = serde_json::to_string(&hits[0]).unwrap();
    assert!(!payload.contains("cost_minor"));
    assert!(!payload.contains("unit_cost_minor"));

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Customer and phone search are permission-gated: a storekeeper without
// customer.view only sees products, never customer records.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_customers_requires_customer_view() {
    let dir = temp_dir("cust-gate");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let storekeeper = make_role_principal(&state, "sk", "storekeeper").await;

    create_customer(&state, &owner, "C-GATE", Some("0300-1234567")).await;

    let r = application::search::global_search(&state, &storekeeper, "C-GATE")
        .await
        .unwrap();
    let customer_hits: Vec<_> = r.results.iter().filter(|h| h.kind == "customer").collect();
    assert!(
        customer_hits.is_empty(),
        "storekeeper must not see customer results"
    );

    let r = application::search::global_search(&state, &storekeeper, "0300-1234567")
        .await
        .unwrap();
    let phone_hits: Vec<_> = r.results.iter().filter(|h| h.kind == "customer").collect();
    assert!(
        phone_hits.is_empty(),
        "storekeeper must not see customer by phone"
    );

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Sale and invoice-number search require sale.create/invoice.print.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_sales_requires_sale_permission() {
    let dir = temp_dir("sale-gate");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let salesperson = make_role_principal(&state, "sp", "salesperson").await;

    let product = stock_product(&state, &owner, "S-GATE", 1, 300).await;
    set_price(&state, product, 4000).await;
    application::customers::create(
        &state,
        &owner,
        CustomerInput {
            code: "INV-S-GATE".into(),
            name: "Sale search customer".into(),
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
    .unwrap();
    let cust_id: i64 = sqlx::query_scalar("SELECT id FROM customers WHERE code = 'INV-S-GATE'")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    confirm_credit_sale(&state, &owner, cust_id, product).await;

    // Owner sees the sale result (matched on the customer snapshot name, since
    // sale_number is auto-issued at confirm time, e.g. S-000001).
    let r = application::search::global_search(&state, &owner, "Sale search customer")
        .await
        .unwrap();
    assert!(r.results.iter().any(|h| h.kind == "sale"));

    // Salesperson (who has sale.create) also sees sales.
    let r = application::search::global_search(&state, &salesperson, "Sale search customer")
        .await
        .unwrap();
    assert!(r.results.iter().any(|h| h.kind == "sale"));

    // A storekeeper (no sale.create/invoice.print) must not see sales.
    let storekeeper = make_role_principal(&state, "sk", "storekeeper").await;
    let r = application::search::global_search(&state, &storekeeper, "Sale search customer")
        .await
        .unwrap();
    assert!(
        r.results.iter().all(|h| h.kind != "sale"),
        "storekeeper must not see sales"
    );

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}
