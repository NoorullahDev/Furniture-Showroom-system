use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application;
use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::dto::purchases::{
    CashAccountInput, PurchaseCreateInput, PurchaseItemInput, PurchasePostInput, SupplierInput,
};
use furniture_shop_lib::dto::sales::{
    BundleInput, BundleItemInput, CustomerInput, CustomerReceiptInput, SaleCancelInput,
    SaleConfirmInput, SaleCreateInput, SaleItemInput,
};
use furniture_shop_lib::error::AppError;
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-sales-{label}"));
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

async fn set_price(state: &AppState, product_id: i64, price: i64) {
    sqlx::query("UPDATE products SET sale_price_minor = ? WHERE id = ?")
        .bind(price)
        .bind(product_id)
        .execute(&state.pool)
        .await
        .unwrap();
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

async fn main_location(state: &AppState) -> i64 {
    sqlx::query_scalar("SELECT id FROM locations WHERE name = 'Main Showroom'")
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

async fn on_hand(state: &AppState, product_id: i64, location_id: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE(on_hand, 0) FROM stock_balances WHERE product_id = ? AND location_id = ?",
    )
    .bind(product_id)
    .bind(location_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap()
    .unwrap_or(0)
}

async fn customer_balance(state: &AppState, customer_id: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT balance_after_minor FROM customer_ledger_entries
         WHERE customer_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(customer_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap()
    .unwrap_or(0)
}

async fn cash_balance(state: &AppState, account_id: i64) -> i64 {
    sqlx::query_scalar("SELECT balance_minor FROM cash_accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

async fn sale_seq(state: &AppState) -> i64 {
    sqlx::query_scalar("SELECT next_value FROM document_sequences WHERE document_type = 'sale'")
        .fetch_one(&state.pool)
        .await
        .unwrap()
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
            credit_limit_minor: Some(200_000),
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

fn line(product_id: i64, quantity: i64) -> SaleItemInput {
    SaleItemInput {
        product_id: Some(product_id),
        bundle_id: None,
        quantity,
    }
}

#[tokio::test]
async fn cash_sale_posts_stock_cogs_customer_ledger_and_cash() {
    let dir = temp_dir("cash-sale");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "cash-sale").await;
    let product = stock_product(&state, &owner, "SALE-01", 10, 5000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let customer = create_customer(&state, &owner, "CUST-CASH").await;
    let cash = funded_cash(&state, &owner, "SALESCASH").await;

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
            items: vec![line(product, 3)],
        },
        "corr-1",
    )
    .await
    .unwrap();
    assert_eq!(sale.status, "draft");
    assert_eq!(sale.total_minor, 30_000);
    assert!(sale.sale_number.is_none());

    let confirmed = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: Some("key-cash".into()),
            paid_minor: Some(30_000),
            cash_account_id: Some(cash),
            payment_method_id: Some(1),
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-2",
    )
    .await
    .expect("confirm cash sale");

    assert_eq!(confirmed.status, "confirmed");
    assert!(confirmed
        .sale_number
        .as_deref()
        .unwrap()
        .starts_with("INV-"));
    assert_eq!(confirmed.paid_minor, 30_000);
    assert_eq!(confirmed.due_minor, 0);
    assert_eq!(confirmed.cost_minor, 15_000);

    assert_eq!(on_hand(&state, product, location).await, 7);
    assert_eq!(cash_balance(&state, cash).await, 1_030_000);
    assert_eq!(customer_balance(&state, customer).await, 0);
}

#[tokio::test]
async fn idempotent_confirm_consumes_exactly_one_number() {
    let dir = temp_dir("idem");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "idem").await;
    let product = stock_product(&state, &owner, "SALE-02", 10, 1000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let before = sale_seq(&state).await;

    let sale = application::sales::create_sale(
        &state,
        &owner,
        SaleCreateInput {
            location_id: location,
            customer_id: None,
            kind: None,
            sale_date: None,
            discount_minor: None,
            delivery_charge_minor: None,
            notes: None,
            items: vec![line(product, 1)],
        },
        "corr-1",
    )
    .await
    .unwrap();

    let first = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: Some("dup-key".into()),
            paid_minor: Some(10_000),
            cash_account_id: Some(1),
            payment_method_id: Some(1),
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-2",
    )
    .await
    .unwrap();
    let second = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: Some("dup-key".into()),
            paid_minor: Some(10_000),
            cash_account_id: Some(1),
            payment_method_id: Some(1),
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-3",
    )
    .await
    .unwrap();

    assert_eq!(first.sale_number, second.sale_number);
    assert_eq!(sale_seq(&state).await, before + 1);
    // Stock issued exactly once.
    let issues: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM stock_movements WHERE movement_type = 'sale_issue'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(issues, 1);
}

#[tokio::test]
async fn walkin_paid_sale_cancel_refunds_cash() {
    let dir = temp_dir("walkin-cancel");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "walkin-cancel").await;
    let product = stock_product(&state, &owner, "SALE-W", 10, 1000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let cash = funded_cash(&state, &owner, "WCASH").await;

    let sale = application::sales::create_sale(
        &state,
        &owner,
        SaleCreateInput {
            location_id: location,
            customer_id: None,
            kind: None,
            sale_date: None,
            discount_minor: None,
            delivery_charge_minor: None,
            notes: None,
            items: vec![line(product, 1)],
        },
        "corr-1",
    )
    .await
    .unwrap();
    application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: Some("walkin-cnf".into()),
            paid_minor: Some(10_000),
            cash_account_id: Some(cash),
            payment_method_id: Some(1),
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-2",
    )
    .await
    .unwrap();

    assert_eq!(
        cash_balance(&state, cash).await,
        1_010_000,
        "walk-in sale payment credits cash"
    );
    assert_eq!(on_hand(&state, product, location).await, 9);

    application::sales::cancel_sale(
        &state,
        &owner,
        SaleCancelInput {
            sale_id: sale.id,
            reason: Some("customer changed mind".into()),
        },
        "corr-3",
    )
    .await
    .unwrap();

    assert_eq!(
        cash_balance(&state, cash).await,
        1_000_000,
        "cancelling a walk-in sale refunds the cash"
    );
    assert_eq!(on_hand(&state, product, location).await, 10);
    let paid: i64 = sqlx::query_scalar("SELECT paid_minor FROM sales WHERE id = ?")
        .bind(sale.id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(paid, 0);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn credit_sale_receipt_allocation_cancellation_keeps_ledger_consistent() {
    let dir = temp_dir("credit");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "credit").await;
    let product = stock_product(&state, &owner, "SALE-03", 10, 5000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let customer = create_customer(&state, &owner, "CUST-CREDIT").await;
    let cash = funded_cash(&state, &owner, "CREDITCASH").await;

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
            items: vec![line(product, 3)],
        },
        "corr-1",
    )
    .await
    .unwrap();

    let confirmed = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: None,
            paid_minor: Some(0),
            cash_account_id: None,
            payment_method_id: None,
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-2",
    )
    .await
    .unwrap();
    assert_eq!(confirmed.due_minor, 30_000);
    assert_eq!(customer_balance(&state, customer).await, 30_000);

    // Customer pays 20,000 cash: allocated oldest-first, 10,000 remains due.
    let receipt = application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: customer,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-06".into(),
            amount_minor: 20_000,
            notes: None,
            idempotency_key: Some("rc-cash".into()),
            allocations: None,
        },
        "corr-3",
    )
    .await
    .unwrap();
    assert_eq!(receipt.advance_alloc_minor, 0);
    assert_eq!(receipt.allocations.len(), 1);
    assert_eq!(receipt.allocations[0].amount_minor, 20_000);
    assert_eq!(customer_balance(&state, customer).await, 10_000);

    let after_receipt = application::sales::get_sale(&state, &owner, sale.id)
        .await
        .unwrap();
    assert_eq!(after_receipt.due_minor, 10_000);
    assert_eq!(after_receipt.paid_minor, 20_000);

    // Cancelling the invoice turns the 20,000 received into a customer advance
    // (money owed back), which the ledger reflects.
    let cancelled = application::sales::cancel_sale(
        &state,
        &owner,
        SaleCancelInput {
            sale_id: sale.id,
            reason: Some("customer changed mind".into()),
        },
        "corr-4",
    )
    .await
    .unwrap();
    assert_eq!(cancelled.status, "cancelled");
    assert_eq!(cancelled.due_minor, 0);
    // 30,000 sale - 20,000 received = 20,000 credit (advance).
    assert_eq!(customer_balance(&state, customer).await, -20_000);
    assert_eq!(cash_balance(&state, cash).await, 1_000_000);
}

#[tokio::test]
async fn bundle_sale_respects_availability_and_restores_stock_on_cancel() {
    let dir = temp_dir("bundle");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "bundle").await;
    let a = stock_product(&state, &owner, "SOFA-01", 8, 20_000).await;
    let b = stock_product(&state, &owner, "TBL-01", 2, 5_000).await;
    let location = main_location(&state).await;

    let bundle = application::sets::create(
        &state,
        &owner,
        BundleInput {
            code: "SET-LIVING".into(),
            name: "Living Room Set".into(),
            description: Some("Sofa pair + table".into()),
            cover_image_path: None,
            default_price_minor: Some(150_000),
            is_active: Some(true),
            items: vec![
                BundleItemInput {
                    product_id: a,
                    quantity: 2,
                },
                BundleItemInput {
                    product_id: b,
                    quantity: 1,
                },
            ],
        },
        "corr-b",
    )
    .await
    .unwrap();
    assert_eq!(bundle.cost_estimate_minor, 45_000);

    let avail = application::sets::availability(&state, &owner, bundle.id, location)
        .await
        .unwrap();
    assert_eq!(avail.available_count, 2); // min(8/2, 2)

    let sale = application::sales::create_sale(
        &state,
        &owner,
        SaleCreateInput {
            location_id: location,
            customer_id: None,
            kind: Some("sale".into()),
            sale_date: Some("2026-09-05".into()),
            discount_minor: Some(0),
            delivery_charge_minor: Some(0),
            notes: None,
            items: vec![SaleItemInput {
                product_id: None,
                bundle_id: Some(bundle.id),
                quantity: 1,
            }],
        },
        "corr-1",
    )
    .await
    .unwrap();
    assert_eq!(sale.total_minor, 150_000);

    let confirmed = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: Some("key-bundle".into()),
            paid_minor: Some(150_000),
            cash_account_id: Some(1),
            payment_method_id: Some(1),
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-2",
    )
    .await
    .unwrap();
    assert_eq!(confirmed.cost_minor, 45_000);
    assert_eq!(confirmed.items[0].components.len(), 2);
    assert_eq!(on_hand(&state, a, location).await, 6);
    assert_eq!(on_hand(&state, b, location).await, 1);

    let cancelled = application::sales::cancel_sale(
        &state,
        &owner,
        SaleCancelInput {
            sale_id: sale.id,
            reason: Some("damaged on delivery".into()),
        },
        "corr-3",
    )
    .await
    .unwrap();
    assert_eq!(cancelled.status, "cancelled");
    assert_eq!(on_hand(&state, a, location).await, 8);
    assert_eq!(on_hand(&state, b, location).await, 2);
}

#[tokio::test]
async fn insufficient_stock_is_rejected_atomically() {
    let dir = temp_dir("shortage");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "shortage").await;
    let product = stock_product(&state, &owner, "SALE-04", 2, 1000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let seq_before = sale_seq(&state).await;

    let sale = application::sales::create_sale(
        &state,
        &owner,
        SaleCreateInput {
            location_id: location,
            customer_id: None,
            kind: None,
            sale_date: None,
            discount_minor: None,
            delivery_charge_minor: None,
            notes: None,
            items: vec![line(product, 5)],
        },
        "corr-1",
    )
    .await
    .unwrap();

    let err = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: Some("key-short".into()),
            paid_minor: Some(50_000),
            cash_account_id: Some(1),
            payment_method_id: Some(1),
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-2",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));

    // Nothing moved: still draft, no number burned, no stock movement.
    let after = application::sales::get_sale(&state, &owner, sale.id)
        .await
        .unwrap();
    assert_eq!(after.status, "draft");
    assert!(after.sale_number.is_none());
    assert_eq!(sale_seq(&state).await, seq_before);
    assert_eq!(on_hand(&state, product, location).await, 2);
    let moves: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM stock_movements WHERE movement_type = 'sale_issue'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(moves, 0);
}

#[tokio::test]
async fn discount_and_below_cost_pricing_require_override() {
    let dir = temp_dir("discount");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "discount").await;
    let salesperson = make_role_principal(&state, "discount-sp", "salesperson").await;
    let product = stock_product(&state, &owner, "SALE-05", 10, 5_000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;

    // Discount without the override permission is rejected.
    let sale = application::sales::create_sale(
        &state,
        &salesperson,
        SaleCreateInput {
            location_id: location,
            customer_id: None,
            kind: None,
            sale_date: None,
            discount_minor: Some(1_000),
            delivery_charge_minor: None,
            notes: None,
            items: vec![line(product, 1)],
        },
        "corr-1",
    )
    .await
    .unwrap();
    let err = application::sales::confirm_sale(
        &state,
        &salesperson,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: None,
            paid_minor: Some(9_000),
            cash_account_id: Some(1),
            payment_method_id: Some(1),
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-2",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::Unauthorized(_)));

    // Below-cost price (1,000 vs 5,000 cost) also requires the permission.
    set_price(&state, product, 1_000).await;
    let sale2 = application::sales::create_sale(
        &state,
        &owner,
        SaleCreateInput {
            location_id: location,
            customer_id: None,
            kind: None,
            sale_date: None,
            discount_minor: Some(0),
            delivery_charge_minor: None,
            notes: None,
            items: vec![line(product, 1)],
        },
        "corr-3",
    )
    .await
    .unwrap();
    let err2 = application::sales::confirm_sale(
        &state,
        &salesperson,
        SaleConfirmInput {
            sale_id: sale2.id,
            idempotency_key: None,
            paid_minor: Some(1_000),
            cash_account_id: Some(1),
            payment_method_id: Some(1),
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-4",
    )
    .await
    .unwrap_err();
    assert!(matches!(err2, AppError::Unauthorized(_)));
}

#[tokio::test]
async fn an_advance_receipt_is_consumed_when_a_later_sale_confirms() {
    let dir = temp_dir("advance");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "advance").await;
    let product = stock_product(&state, &owner, "SALE-06", 10, 1_000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let customer = create_customer(&state, &owner, "CUST-ADV").await;
    let cash = funded_cash(&state, &owner, "ADVCASH").await;

    application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: customer,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-03".into(),
            amount_minor: 5_000,
            notes: Some("advance".into()),
            idempotency_key: Some("rc-adv".into()),
            allocations: None,
        },
        "corr-1",
    )
    .await
    .unwrap();
    assert_eq!(customer_balance(&state, customer).await, -5_000);

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
            items: vec![line(product, 2)],
        },
        "corr-2",
    )
    .await
    .unwrap();

    let confirmed = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: Some("key-adv".into()),
            paid_minor: Some(0),
            cash_account_id: None,
            payment_method_id: None,
            advance_used_minor: Some(5_000),
            credit_note_id: None,
        },
        "corr-3",
    )
    .await
    .unwrap();
    assert_eq!(confirmed.due_minor, 15_000);
    assert_eq!(confirmed.advance_used_minor, 5_000);
    assert_eq!(customer_balance(&state, customer).await, 15_000);

    // An advance larger than the available balance is rejected.
    let sale2 = application::sales::create_sale(
        &state,
        &owner,
        SaleCreateInput {
            location_id: location,
            customer_id: Some(customer),
            kind: None,
            sale_date: None,
            discount_minor: None,
            delivery_charge_minor: None,
            notes: None,
            items: vec![line(product, 2)],
        },
        "corr-4",
    )
    .await
    .unwrap();
    let err = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: sale2.id,
            idempotency_key: None,
            paid_minor: Some(0),
            cash_account_id: None,
            payment_method_id: None,
            advance_used_minor: Some(9_000),
            credit_note_id: None,
        },
        "corr-5",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
}

#[tokio::test]
async fn confirmed_sale_generates_an_invoice_pdf() {
    let dir = temp_dir("invoice");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "invoice").await;
    let product = stock_product(&state, &owner, "SALE-07", 10, 1_000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;

    // A draft cannot produce an invoice.
    let draft = application::sales::create_sale(
        &state,
        &owner,
        SaleCreateInput {
            location_id: location,
            customer_id: None,
            kind: None,
            sale_date: None,
            discount_minor: None,
            delivery_charge_minor: None,
            notes: None,
            items: vec![line(product, 2)],
        },
        "corr-1",
    )
    .await
    .unwrap();
    let err = application::sales::invoice_pdf(&state, &owner, draft.id)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));

    let confirmed = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: draft.id,
            idempotency_key: Some("key-inv".into()),
            paid_minor: Some(20_000),
            cash_account_id: Some(1),
            payment_method_id: Some(1),
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-2",
    )
    .await
    .unwrap();

    let pdf = application::sales::invoice_pdf(&state, &owner, confirmed.id)
        .await
        .expect("invoice pdf");
    assert!(pdf.bytes > 0);
    assert!(std::path::Path::new(&pdf.report_path).exists());
    assert!(pdf.report_path.ends_with(".pdf"));
}

#[tokio::test]
async fn sale_snapshots_the_selected_customer_name() {
    let dir = temp_dir("cust-name");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "cust-name").await;
    let product = stock_product(&state, &owner, "SALE-CN", 10, 1_000).await;
    set_price(&state, product, 3_000).await;
    let location = main_location(&state).await;
    let customer = create_customer(&state, &owner, "CUST-NAME").await;

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
            items: vec![line(product, 1)],
        },
        "corr-1",
    )
    .await
    .unwrap();

    assert_eq!(sale.customer_name.as_deref(), Some("Customer CUST-NAME"));
}

#[tokio::test]
async fn customer_update_persists_credit_limit() {
    let dir = temp_dir("cust-limit");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "cust-limit").await;
    let customer = create_customer(&state, &owner, "CUST-LIMIT").await;

    let updated = application::customers::update(
        &state,
        &owner,
        customer,
        CustomerInput {
            code: "CUST-LIMIT".into(),
            name: "Customer CUST-LIMIT".into(),
            phone: None,
            email: None,
            address: None,
            credit_limit_minor: Some(500_000),
            credit_days: None,
            opening_balance_minor: Some(0),
            is_active: Some(true),
        },
        "corr-limit",
    )
    .await
    .unwrap();

    assert_eq!(updated.credit_limit_minor, 500_000);
    assert_eq!(updated.balance_minor, 0);
}
