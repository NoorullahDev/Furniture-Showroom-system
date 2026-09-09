use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application;
use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::dto::receivables::{CustomerReceiptPreviewInput, CustomerStatementInput};
use furniture_shop_lib::dto::sales::{
    CustomerInput, CustomerReceiptAllocationInput, CustomerReceiptInput, SaleConfirmInput,
    SaleCreateInput, SaleItemInput,
};
use furniture_shop_lib::error::AppError;
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-receivables-{label}"));
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
        furniture_shop_lib::dto::purchases::SupplierInput {
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
    let draft = furniture_shop_lib::dto::purchases::PurchaseCreateInput {
        supplier_id: supplier,
        location_id: location,
        invoice_number: format!("INV-{article}"),
        invoice_date: "2026-09-01".into(),
        purchase_date: Some("2026-09-01".into()),
        notes: None,
        items: vec![furniture_shop_lib::dto::purchases::PurchaseItemInput {
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
        furniture_shop_lib::dto::purchases::PurchasePostInput {
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
        furniture_shop_lib::dto::purchases::CashAccountInput {
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

async fn create_customer(
    state: &AppState,
    owner: &Principal,
    code: &str,
    credit_days: Option<i64>,
    credit_limit_minor: Option<i64>,
) -> i64 {
    application::customers::create(
        state,
        owner,
        CustomerInput {
            code: code.into(),
            name: format!("Customer {code}"),
            phone: None,
            email: None,
            address: None,
            credit_limit_minor,
            credit_days,
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

async fn make_open_sale(
    state: &AppState,
    owner: &Principal,
    customer_id: Option<i64>,
    product_id: i64,
    quantity: i64,
    sale_date: &str,
) -> furniture_shop_lib::dto::sales::SaleDto {
    let location = main_location(state).await;
    let sale = application::sales::create_sale(
        state,
        owner,
        SaleCreateInput {
            location_id: location,
            customer_id,
            kind: Some("sale".into()),
            sale_date: Some(sale_date.into()),
            discount_minor: Some(0),
            delivery_charge_minor: Some(0),
            notes: None,
            items: vec![line(product_id, quantity)],
        },
        "corr-create",
    )
    .await
    .unwrap();
    application::sales::confirm_sale(
        state,
        owner,
        SaleConfirmInput {
            sale_id: sale.id,
            idempotency_key: None,
            paid_minor: Some(0),
            cash_account_id: None,
            payment_method_id: None,
            advance_used_minor: Some(0),
        },
        "corr-confirm",
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn confirm_sale_records_due_date_from_credit_terms() {
    let dir = temp_dir("due-date");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "due-date").await;
    let product = stock_product(&state, &owner, "RC-01", 50, 1000).await;
    set_price(&state, product, 10_000).await;
    let customer = create_customer(&state, &owner, "CUST-DUE", Some(15), None).await;

    let s = make_open_sale(&state, &owner, Some(customer), product, 2, "2026-09-01").await;
    assert_eq!(s.due_minor, 20_000);

    let due_date: String = sqlx::query_scalar("SELECT due_date FROM sales WHERE id = ?")
        .bind(s.id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    let expected: String = sqlx::query_scalar::<_, String>("SELECT date('2026-09-01', '+15 days')")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(due_date, expected);
}

#[tokio::test]
async fn statement_balance_reconciles_with_ledger_for_date_ranges() {
    let dir = temp_dir("statement");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "stmnt").await;
    let product = stock_product(&state, &owner, "RC-02", 50, 1000).await;
    set_price(&state, product, 10_000).await;
    let customer = create_customer(&state, &owner, "CUST-STMT", None, None).await;
    let cash = funded_cash(&state, &owner, "STMTCASH").await;

    let s1 = make_open_sale(&state, &owner, Some(customer), product, 2, "2026-09-01").await;
    let s2 = make_open_sale(&state, &owner, Some(customer), product, 3, "2026-09-02").await;
    let _ = s1;
    let _ = s2;

    // Pay 20,000: clears s1 exactly and 50,000 balance remains (s2 = 30,000).
    application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: customer,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-03".into(),
            amount_minor: 20_000,
            notes: None,
            idempotency_key: Some("rc-stmt".into()),
            allocations: None,
        },
        "corr-stmt",
    )
    .await
    .unwrap();

    let expected_balance = customer_balance(&state, customer).await;
    assert_eq!(expected_balance, 30_000);

    let ranges: Vec<(String, String)> = sqlx::query_as(
        "SELECT date(created_at), date(created_at) FROM customer_ledger_entries
         WHERE customer_id = ?",
    )
    .bind(customer)
    .fetch_all(&state.pool)
    .await
    .unwrap();

    for (from, to) in ranges {
        let stmt = application::receivables::customer_statement(
            &state,
            &owner,
            CustomerStatementInput {
                customer_id: customer,
                from_date: from.clone(),
                to_date: to.clone(),
            },
        )
        .await
        .unwrap();

        let mut running = stmt.opening_balance_minor;
        let mut entries_sum = 0i64;
        for e in &stmt.entries {
            entries_sum += e.amount_minor;
            running += e.amount_minor;
            assert_eq!(running, e.balance_after_minor);
        }
        assert_eq!(
            stmt.opening_balance_minor + entries_sum,
            stmt.closing_balance_minor
        );
        assert_eq!(stmt.closing_balance_minor, expected_balance);
    }
}

#[tokio::test]
async fn receipt_preview_matches_automatic_posting_and_explicit_allocation_is_honored() {
    let dir = temp_dir("preview");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "preview").await;
    let product = stock_product(&state, &owner, "RC-03", 50, 1000).await;
    set_price(&state, product, 10_000).await;
    let customer = create_customer(&state, &owner, "CUST-PRV", None, None).await;
    let cash = funded_cash(&state, &owner, "PREVCASH").await;

    let s1 = make_open_sale(&state, &owner, Some(customer), product, 2, "2026-09-01").await;
    let s2 = make_open_sale(&state, &owner, Some(customer), product, 3, "2026-09-02").await;

    // Total open due = 50,000. Pay 45,000 → preview clears s1 (20k) and most
    // of s2 (25k), leaving no advance and a 5,000 overdue balance.
    let preview = application::receivables::receipt_preview(
        &state,
        &owner,
        CustomerReceiptPreviewInput {
            customer_id: customer,
            amount_minor: 45_000,
        },
    )
    .await
    .unwrap();
    assert_eq!(preview.advance_minor, 0);
    assert_eq!(preview.allocations.len(), 2);
    assert_eq!(preview.allocations[0].sale_id, s1.id);
    assert_eq!(preview.allocations[0].allocated_minor, 20_000);
    assert_eq!(preview.allocations[1].sale_id, s2.id);
    assert_eq!(preview.allocations[1].allocated_minor, 25_000);

    // Automatic posting matches the preview exactly.
    let auto = application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: customer,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-03".into(),
            amount_minor: 45_000,
            notes: None,
            idempotency_key: Some("rc-auto".into()),
            allocations: None,
        },
        "corr-auto",
    )
    .await
    .unwrap();
    assert_eq!(auto.advance_alloc_minor, 0);
    assert_eq!(
        auto.allocations.iter().map(|a| a.amount_minor).sum::<i64>(),
        45_000
    );
    assert_eq!(customer_balance(&state, customer).await, 5_000);

    // Now pay 30,000 allocated deliberately to the second invoice's remaining
    // 5,000 only: the rest (25,000) must become an advance.
    let manual = application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: customer,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-04".into(),
            amount_minor: 30_000,
            notes: Some("manual".into()),
            idempotency_key: Some("rc-manual".into()),
            allocations: Some(vec![CustomerReceiptAllocationInput {
                sale_id: s2.id,
                amount_minor: 5_000,
            }]),
        },
        "corr-manual",
    )
    .await
    .unwrap();
    assert_eq!(manual.advance_alloc_minor, 25_000);
    assert_eq!(manual.allocations.len(), 1);
    assert_eq!(manual.allocations[0].sale_id, s2.id);

    let s2d = application::sales::get_sale(&state, &owner, s2.id)
        .await
        .unwrap();
    assert_eq!(s2d.due_minor, 0);
    assert_eq!(s2d.paid_minor, 30_000);
    let s1d = application::sales::get_sale(&state, &owner, s1.id)
        .await
        .unwrap();
    assert_eq!(s1d.due_minor, 0);
    assert_eq!(s1d.paid_minor, 20_000);
    assert_eq!(customer_balance(&state, customer).await, -25_000);
}

#[tokio::test]
async fn manual_allocation_rejects_over_payment_and_non_customer_sales() {
    let dir = temp_dir("manual-bad");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "manual-bad").await;
    let product = stock_product(&state, &owner, "RC-04", 50, 1000).await;
    set_price(&state, product, 10_000).await;
    let c1 = create_customer(&state, &owner, "CUST-M1", None, None).await;
    let c2 = create_customer(&state, &owner, "CUST-M2", None, None).await;
    let cash = funded_cash(&state, &owner, "MANCASH").await;
    let s1 = make_open_sale(&state, &owner, Some(c1), product, 2, "2026-09-01").await;
    let s2 = make_open_sale(&state, &owner, Some(c2), product, 1, "2026-09-01").await;

    let over = application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: c1,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-03".into(),
            amount_minor: 10_000,
            notes: None,
            idempotency_key: None,
            allocations: Some(vec![CustomerReceiptAllocationInput {
                sale_id: s1.id,
                amount_minor: 20_001,
            }]),
        },
        "corr-over",
    )
    .await;
    assert!(matches!(over, Err(AppError::Validation(_))));

    // Allocating to another customer's invoice must be rejected.
    let wrong = application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: c1,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-03".into(),
            amount_minor: 10_000,
            notes: None,
            idempotency_key: None,
            allocations: Some(vec![CustomerReceiptAllocationInput {
                sale_id: s2.id,
                amount_minor: 5_000,
            }]),
        },
        "corr-wrong",
    )
    .await;
    assert!(matches!(wrong, Err(AppError::Validation(_))));

    // Nothing was posted.
    let payments: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM customer_payments")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(payments, 0);
    assert_eq!(customer_balance(&state, c1).await, 20_000);
}

#[tokio::test]
async fn receipt_posts_single_ledger_and_cash_entries() {
    let dir = temp_dir("once");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "once").await;
    let product = stock_product(&state, &owner, "RC-05", 50, 1000).await;
    set_price(&state, product, 10_000).await;
    let customer = create_customer(&state, &owner, "CUST-ONCE", None, None).await;
    let cash = funded_cash(&state, &owner, "ONCECASH").await;
    let before_cash = cash_balance(&state, cash).await;

    let s = make_open_sale(&state, &owner, Some(customer), product, 4, "2026-09-01").await;

    let receipt = application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: customer,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-02".into(),
            amount_minor: 15_000,
            notes: None,
            idempotency_key: Some("rc-once".into()),
            allocations: None,
        },
        "corr-once",
    )
    .await
    .unwrap();

    let cash_hits: Vec<(String, i64)> = sqlx::query_as(
        "SELECT entry_type, amount_minor FROM cash_entries
         WHERE reference_type = 'customer_payment' AND reference_id = ?",
    )
    .bind(receipt.id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(cash_hits.len(), 1);
    assert_eq!(cash_hits[0], ("customer_receipt".into(), 15_000));

    let ledger_hits: Vec<(String, i64)> = sqlx::query_as(
        "SELECT entry_type, amount_minor FROM customer_ledger_entries
         WHERE document_type = 'customer_payment' AND document_id = ?",
    )
    .bind(receipt.id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(ledger_hits.len(), 1);
    assert_eq!(ledger_hits[0], ("payment".into(), -15_000));

    assert_eq!(cash_balance(&state, cash).await, before_cash + 15_000);
    assert_eq!(
        customer_balance(&state, customer).await,
        s.total_minor - 15_000
    );
}

#[tokio::test]
async fn voiding_receipt_restores_due_advance_and_cash_position() {
    let dir = temp_dir("void-restore");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "void-restore").await;
    let product = stock_product(&state, &owner, "RC-06", 50, 1000).await;
    set_price(&state, product, 10_000).await;
    let customer = create_customer(&state, &owner, "CUST-VOID", None, None).await;
    let cash = funded_cash(&state, &owner, "VOIDCASH").await;
    let before_cash = cash_balance(&state, cash).await;

    // Advance only: no open invoices.
    let receipt = application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: customer,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-01".into(),
            amount_minor: 10_000,
            notes: None,
            idempotency_key: Some("rc-adv".into()),
            allocations: Some(vec![]),
        },
        "corr-adv",
    )
    .await
    .unwrap();
    assert_eq!(receipt.advance_alloc_minor, 10_000);
    assert_eq!(customer_balance(&state, customer).await, -10_000);
    assert_eq!(cash_balance(&state, cash).await, before_cash + 10_000);

    application::customers::void_receipt(
        &state,
        &owner,
        furniture_shop_lib::dto::sales::CustomerPaymentVoidInput {
            payment_id: receipt.id,
            reason: Some("wrong amount".into()),
        },
        "corr-void",
    )
    .await
    .unwrap();

    assert_eq!(customer_balance(&state, customer).await, 0);
    assert_eq!(cash_balance(&state, cash).await, before_cash);
}

#[tokio::test]
async fn receivables_lists_overdue_due_soon_high_balance_and_credit_exceptions() {
    let dir = temp_dir("exceptions");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "excp").await;
    let product = stock_product(&state, &owner, "RC-07", 50, 1000).await;
    set_price(&state, product, 10_000).await;

    // Terms of 30 days: an invoice dated 60 days ago is overdue; one dated 25
    // days ago falls due in the next seven days.
    let c1 = create_customer(&state, &owner, "CUST-DUE1", Some(30), Some(1_000_000)).await;
    let sale_old = make_open_sale(
        &state,
        &owner,
        Some(c1),
        product,
        2,
        &sql_now_minus(&state, 60).await,
    )
    .await;
    let sale_soon = make_open_sale(
        &state,
        &owner,
        Some(c1),
        product,
        3,
        &sql_now_minus(&state, 25).await,
    )
    .await;

    // A customer whose balance exceeds a tight credit limit.
    let c2 = create_customer(&state, &owner, "CUST-XLIM", Some(30), Some(10_000)).await;
    let sale_x = make_open_sale(
        &state,
        &owner,
        Some(c2),
        product,
        4,
        &sql_now_minus(&state, 60).await,
    )
    .await;

    let r = application::receivables::receivables(&state, &owner)
        .await
        .unwrap();

    assert!(r
        .overdue
        .iter()
        .any(|s| s.sale_id == sale_old.id && s.due_minor == 20_000));
    assert!(r.overdue.iter().any(|s| s.sale_id == sale_x.id));
    assert!(r.overdue.iter().all(|s| s.days >= 1));
    assert!(r.due_soon.iter().any(|s| s.sale_id == sale_soon.id));
    assert!(r.due_soon.iter().all(|s| s.days >= 0));

    let c1_row = r.high_balance.iter().find(|b| b.customer_id == c1).unwrap();
    assert_eq!(c1_row.balance_minor, 50_000);
    assert_eq!(c1_row.due_minor_total, 50_000);
    assert_eq!(c1_row.overdue_minor_total, 20_000);
    let c2_row = r.high_balance.iter().find(|b| b.customer_id == c2).unwrap();
    assert_eq!(c2_row.balance_minor, 40_000);
    assert!(r
        .credit_limit_exceptions
        .iter()
        .any(|b| b.customer_id == c2));
    assert!(r
        .credit_limit_exceptions
        .iter()
        .all(|b| b.customer_id != c1));
}

/// SQLite's date() mirrors the UTC clock used by the app; computing the offset
/// in SQL keeps the test deterministic relative to the receivables queries.
async fn sql_now_minus(state: &AppState, days: i64) -> String {
    if days > 0 {
        sqlx::query_scalar::<_, String>(&format!("SELECT date('now', '-{days} days')"))
            .fetch_one(&state.pool)
            .await
            .unwrap()
    } else {
        sqlx::query_scalar::<_, String>("SELECT date('now')")
            .fetch_one(&state.pool)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn receipt_pdf_generates_for_posted_receipt_and_rejects_voided() {
    let dir = temp_dir("receipt-pdf");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "rcpd").await;
    let product = stock_product(&state, &owner, "RC-08", 50, 1000).await;
    set_price(&state, product, 10_000).await;
    let customer = create_customer(&state, &owner, "CUST-PDF", None, None).await;
    let cash = funded_cash(&state, &owner, "PDFCASH").await;
    let sale = make_open_sale(&state, &owner, Some(customer), product, 2, "2026-09-01").await;

    let receipt = application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: customer,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-02".into(),
            amount_minor: 20_000,
            notes: None,
            idempotency_key: Some("rc-pdf".into()),
            allocations: None,
        },
        "corr-pdf",
    )
    .await
    .unwrap();
    assert_eq!(receipt.allocations.len(), 1);
    assert_eq!(receipt.allocations[0].sale_id, sale.id);

    let pdf = application::receivables::customer_receipt_pdf(&state, &owner, receipt.id)
        .await
        .unwrap();
    assert!(!pdf.report_path.is_empty());
    assert!(std::path::Path::new(&pdf.report_path).exists());
    assert!(pdf.bytes > 100);

    application::customers::void_receipt(
        &state,
        &owner,
        furniture_shop_lib::dto::sales::CustomerPaymentVoidInput {
            payment_id: receipt.id,
            reason: Some("reprint-after-void".into()),
        },
        "corr-void-pdf",
    )
    .await
    .unwrap();
    let after_void =
        application::receivables::customer_receipt_pdf(&state, &owner, receipt.id).await;
    assert!(matches!(after_void, Err(AppError::Validation(_))));
}

#[tokio::test]
async fn statement_requires_customer_view_and_accountant_can_read_it() {
    let dir = temp_dir("stmt-perm");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "stmt-perm").await;
    let accountant = make_role_principal(&state, "acct-stmt", "accountant").await;
    let product = stock_product(&state, &owner, "RC-09", 50, 1000).await;
    set_price(&state, product, 10_000).await;
    let customer = create_customer(&state, &owner, "CUST-PERM", None, None).await;
    make_open_sale(&state, &owner, Some(customer), product, 1, "2026-09-01").await;

    let stmt = application::receivables::customer_statement(
        &state,
        &accountant,
        CustomerStatementInput {
            customer_id: customer,
            from_date: "2020-01-01".into(),
            to_date: "2099-01-01".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(stmt.opening_balance_minor, 0);
    assert_eq!(stmt.closing_balance_minor, 10_000);
    assert!(!stmt.entries.is_empty());
}
