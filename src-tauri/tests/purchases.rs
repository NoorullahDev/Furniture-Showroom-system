use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application;
use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::dto::purchases::{
    CashAccountInput, PurchaseCreateInput, PurchaseItemInput, PurchasePostInput, SupplierInput,
    SupplierPaymentInput, SupplierPaymentVoidInput, SupplierReturnCreateInput,
    SupplierReturnItemInput, SupplierReturnPostInput,
};
use furniture_shop_lib::error::AppError;
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-purchases-{label}"));
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
    let owner_id: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE code = 'owner'")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
        .bind(user_id)
        .bind(owner_id)
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
    .bind(owner_id)
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

/// A funded cash account so paid purchases and payments can post.
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
    .unwrap_or(None)
    .unwrap_or(0)
}

async fn supplier_balance(state: &AppState, supplier_id: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT balance_after_minor FROM supplier_ledger_entries
         WHERE supplier_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(supplier_id)
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

async fn seq_value(state: &AppState, document_type: &str) -> i64 {
    sqlx::query_scalar("SELECT next_value FROM document_sequences WHERE document_type = ?")
        .bind(document_type)
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

fn draft_purchase(
    article: &str,
    supplier_id: i64,
    location_id: i64,
    qty: i64,
    cost: i64,
) -> PurchaseCreateInput {
    PurchaseCreateInput {
        supplier_id,
        location_id,
        invoice_number: format!("INV-{article}-1"),
        invoice_date: "2026-09-01".into(),
        purchase_date: Some("2026-09-01".into()),
        notes: None,
        items: vec![PurchaseItemInput {
            product_id: 0,
            quantity: qty,
            unit_cost_minor: cost,
        }],
    }
}

#[tokio::test]
async fn paid_purchase_posts_stock_cost_payable_cash_and_audit_exactly_once() {
    let dir = temp_dir("paid");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "paid").await;
    let product = create_product(&state, "PCH-01").await;
    let supplier = create_supplier(&state, &owner, "SUP-PAID").await;
    let location = main_location(&state).await;
    let cash = funded_cash(&state, &owner, "PAID").await;

    let mut draft = draft_purchase("PCH-01", supplier, location, 10, 5000);
    draft.items[0].product_id = product;
    let purchase = application::purchases::create_purchase(&state, &owner, draft, "corr-1")
        .await
        .unwrap();
    assert_eq!(purchase.status, "draft");
    assert_eq!(purchase.total_minor, 50_000);
    assert!(purchase.purchase_number.is_none());

    let posted = application::purchases::post_purchase(
        &state,
        &owner,
        PurchasePostInput {
            purchase_id: purchase.id,
            idempotency_key: Some("key-paid".into()),
            paid_minor: Some(30_000),
            cash_account_id: Some(cash),
            payment_method_id: Some(1),
        },
        "corr-2",
    )
    .await
    .expect("post paid purchase");

    assert_eq!(posted.status, "posted");
    assert!(posted
        .purchase_number
        .as_deref()
        .unwrap()
        .starts_with("PUR-"));
    assert_eq!(posted.paid_minor, 30_000);
    assert_eq!(posted.due_minor, 20_000);

    // Stock, cost, payable, cash each changed exactly once.
    assert_eq!(on_hand(&state, product, location).await, 10);
    let movement_sum: i64 = sqlx::query_scalar(
        "SELECT SUM(quantity_delta) FROM stock_movements
         WHERE product_id = ? AND movement_type = 'purchase_receipt'",
    )
    .bind(product)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(movement_sum, 10);
    let layers: i64 =
        sqlx::query_scalar("SELECT SUM(quantity) FROM inventory_cost_layers WHERE product_id = ?")
            .bind(product)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(layers, 10, "cost layers must match received quantity");
    assert_eq!(supplier_balance(&state, supplier).await, 20_000);
    assert_eq!(cash_balance(&state, cash).await, 1_000_000 - 30_000);

    let posted_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE action = 'purchase.post'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(
        posted_count, 1,
        "purchase.post must be audited exactly once"
    );

    // Sequences advanced exactly once per document.
    assert_eq!(seq_value(&state, "purchase").await, 2);
    assert_eq!(seq_value(&state, "supplier_payment").await, 2);

    // Idempotent replay with the same key must not post a second time.
    let replay = application::purchases::post_purchase(
        &state,
        &owner,
        PurchasePostInput {
            purchase_id: purchase.id,
            idempotency_key: Some("key-paid".into()),
            paid_minor: None,
            cash_account_id: None,
            payment_method_id: None,
        },
        "corr-2",
    )
    .await
    .unwrap();
    assert_eq!(replay.purchase_number, posted.purchase_number);
    assert_eq!(on_hand(&state, product, location).await, 10);
    assert_eq!(seq_value(&state, "purchase").await, 2);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn credit_purchase_leaves_full_due_and_partial_payment_reduces_it() {
    let dir = temp_dir("credit");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "credit").await;
    let product = create_product(&state, "PCH-02").await;
    let supplier = create_supplier(&state, &owner, "SUP-CREDIT").await;
    let location = main_location(&state).await;
    let cash = funded_cash(&state, &owner, "CREDIT").await;

    let mut draft = draft_purchase("PCH-02", supplier, location, 4, 2500);
    draft.items[0].product_id = product;
    let purchase = application::purchases::create_purchase(&state, &owner, draft, "corr-1")
        .await
        .unwrap();
    let posted = application::purchases::post_purchase(
        &state,
        &owner,
        PurchasePostInput {
            purchase_id: purchase.id,
            idempotency_key: Some("key-credit".into()),
            paid_minor: None,
            cash_account_id: None,
            payment_method_id: None,
        },
        "corr-2",
    )
    .await
    .unwrap();
    assert_eq!(posted.due_minor, 10_000);
    assert_eq!(supplier_balance(&state, supplier).await, 10_000);
    assert_eq!(cash_balance(&state, cash).await, 1_000_000);

    let payment = application::purchases::create_payment(
        &state,
        &owner,
        SupplierPaymentInput {
            supplier_id: supplier,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-05".into(),
            amount_minor: 6_000,
            notes: None,
            idempotency_key: Some("key-pay-credit".into()),
        },
        "corr-3",
    )
    .await
    .unwrap();
    assert_eq!(payment.allocations[0].amount_minor, 6_000);
    assert_eq!(supplier_balance(&state, supplier).await, 4_000);
    assert_eq!(cash_balance(&state, cash).await, 1_000_000 - 6_000);

    let after = application::purchases::get_purchase(&state, &owner, purchase.id)
        .await
        .unwrap();
    assert_eq!(after.paid_minor, 6_000);
    assert_eq!(after.due_minor, 4_000);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn duplicate_supplier_invoice_is_rejected() {
    let dir = temp_dir("dup");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "dup").await;
    let product = create_product(&state, "DUP-01").await;
    let supplier = create_supplier(&state, &owner, "SUP-DUP").await;
    let location = main_location(&state).await;

    let mut draft = draft_purchase("DUP-01", supplier, location, 1, 1000);
    draft.items[0].product_id = product;
    application::purchases::create_purchase(&state, &owner, draft.clone(), "corr-1")
        .await
        .unwrap();

    let err = application::purchases::create_purchase(&state, &owner, draft, "corr-2")
        .await
        .unwrap_err();
    assert!(
        matches!(err, AppError::Conflict(_)),
        "expected conflict, got {err:?}"
    );
    assert_eq!(err.code(), "CONFLICT");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM purchases")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn failed_paid_post_rolls_back_everything_including_document_number() {
    let dir = temp_dir("rollback");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "rollback").await;
    let product = create_product(&state, "RBK-01").await;
    let supplier = create_supplier(&state, &owner, "SUP-RBK").await;
    let location = main_location(&state).await;
    let cash = funded_cash(&state, &owner, "RBK").await;

    let mut draft = draft_purchase("RBK-01", supplier, location, 5, 1000);
    draft.items[0].product_id = product;
    let purchase = application::purchases::create_purchase(&state, &owner, draft, "corr-1")
        .await
        .unwrap();

    // Paid amount far above the cash balance drives the account into overdraft
    // after the document number, movements, and invoice ledger were written.
    let err = application::purchases::post_purchase(
        &state,
        &owner,
        PurchasePostInput {
            purchase_id: purchase.id,
            idempotency_key: Some("key-rbk".into()),
            paid_minor: Some(9_000_000),
            cash_account_id: Some(cash),
            payment_method_id: Some(1),
        },
        "corr-2",
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, AppError::Validation(_)),
        "expected validation, got {err:?}"
    );

    let seq = seq_value(&state, "purchase").await;
    assert_eq!(seq, 1, "document number must not be consumed on failure");
    assert_eq!(on_hand(&state, product, location).await, 0);
    let movements: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM stock_movements WHERE reference_type = 'purchase'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(movements, 0);
    let layers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inventory_cost_layers WHERE product_id = ?")
            .bind(product)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(layers, 0);
    let ledger: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM supplier_ledger_entries WHERE supplier_id = ?")
            .bind(supplier)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(ledger, 0);
    let cash_entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cash_entries")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(cash_entries, 0);

    let after = application::purchases::get_purchase(&state, &owner, purchase.id)
        .await
        .unwrap();
    assert_eq!(after.status, "draft");
    assert!(after.purchase_number.is_none());
    assert_eq!(after.paid_minor, 0);

    let posted_audits: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE action = 'purchase.post'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(posted_audits, 0);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn payment_void_reverses_allocations_cash_and_payable() {
    let dir = temp_dir("void");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "void").await;
    let product = create_product(&state, "VOID-01").await;
    let supplier = create_supplier(&state, &owner, "SUP-VOID").await;
    let location = main_location(&state).await;
    let cash = funded_cash(&state, &owner, "VOID").await;

    let mut draft = draft_purchase("VOID-01", supplier, location, 3, 2000);
    draft.items[0].product_id = product;
    let purchase = application::purchases::create_purchase(&state, &owner, draft, "corr-1")
        .await
        .unwrap();
    application::purchases::post_purchase(
        &state,
        &owner,
        PurchasePostInput {
            purchase_id: purchase.id,
            idempotency_key: None,
            paid_minor: None,
            cash_account_id: None,
            payment_method_id: None,
        },
        "corr-2",
    )
    .await
    .unwrap();

    let payment = application::purchases::create_payment(
        &state,
        &owner,
        SupplierPaymentInput {
            supplier_id: supplier,
            payment_method_id: 1,
            cash_account_id: cash,
            payment_date: "2026-09-06".into(),
            amount_minor: 3_000,
            notes: None,
            idempotency_key: None,
        },
        "corr-3",
    )
    .await
    .unwrap();
    assert_eq!(supplier_balance(&state, supplier).await, 3_000);
    assert_eq!(cash_balance(&state, cash).await, 1_000_000 - 3_000);

    let voided = application::purchases::void_payment(
        &state,
        &owner,
        SupplierPaymentVoidInput {
            payment_id: payment.id,
            reason: Some("entered twice".into()),
        },
        "corr-4",
    )
    .await
    .unwrap();
    assert_eq!(voided.status, "voided");

    assert_eq!(
        supplier_balance(&state, supplier).await,
        6_000,
        "payable restored"
    );
    assert_eq!(cash_balance(&state, cash).await, 1_000_000, "cash restored");
    let after = application::purchases::get_purchase(&state, &owner, purchase.id)
        .await
        .unwrap();
    assert_eq!(after.paid_minor, 0);
    assert_eq!(after.due_minor, 6_000);

    let void_audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs WHERE action = 'supplier.payment_void'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(void_audits, 1);

    // Voiding twice is rejected.
    let err = application::purchases::void_payment(
        &state,
        &owner,
        SupplierPaymentVoidInput {
            payment_id: payment.id,
            reason: None,
        },
        "corr-5",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::Conflict(_)));

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn return_reduces_stock_and_payable_and_cannot_exceed_net_received() {
    let dir = temp_dir("return");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "return").await;
    let product = create_product(&state, "RTN-01").await;
    let supplier = create_supplier(&state, &owner, "SUP-RTN").await;
    let location = main_location(&state).await;
    let _cash = funded_cash(&state, &owner, "RTN").await;

    let mut draft = draft_purchase("RTN-01", supplier, location, 10, 900);
    draft.items[0].product_id = product;
    let purchase = application::purchases::create_purchase(&state, &owner, draft, "corr-1")
        .await
        .unwrap();
    application::purchases::post_purchase(
        &state,
        &owner,
        PurchasePostInput {
            purchase_id: purchase.id,
            idempotency_key: None,
            paid_minor: None,
            cash_account_id: None,
            payment_method_id: None,
        },
        "corr-2",
    )
    .await
    .unwrap();
    assert_eq!(supplier_balance(&state, supplier).await, 9_000);

    let ret = application::purchases::create_return(
        &state,
        &owner,
        SupplierReturnCreateInput {
            supplier_id: supplier,
            purchase_id: Some(purchase.id),
            location_id: location,
            return_date: "2026-09-07".into(),
            refund_minor: Some(0),
            notes: None,
            items: vec![SupplierReturnItemInput {
                product_id: product,
                quantity: 4,
                unit_cost_minor: 900,
            }],
        },
        "corr-3",
    )
    .await
    .unwrap();
    assert_eq!(ret.status, "draft");

    let posted = application::purchases::post_return(
        &state,
        &owner,
        SupplierReturnPostInput {
            return_id: ret.id,
            idempotency_key: Some("key-return-1".into()),
        },
        "corr-4",
    )
    .await
    .unwrap();
    assert_eq!(posted.status, "posted");
    assert!(posted.return_number.as_deref().unwrap().starts_with("SRN-"));
    assert_eq!(posted.total_minor, 3_600); // 4 x FIFO cost 900
    assert_eq!(on_hand(&state, product, location).await, 6);
    assert_eq!(supplier_balance(&state, supplier).await, 5_400);

    // The remaining net received is 6; a return of 7 must be rejected.
    let ret2 = application::purchases::create_return(
        &state,
        &owner,
        SupplierReturnCreateInput {
            supplier_id: supplier,
            purchase_id: Some(purchase.id),
            location_id: location,
            return_date: "2026-09-08".into(),
            refund_minor: Some(0),
            notes: None,
            items: vec![SupplierReturnItemInput {
                product_id: product,
                quantity: 7,
                unit_cost_minor: 900,
            }],
        },
        "corr-5",
    )
    .await
    .unwrap();
    let err = application::purchases::post_return(
        &state,
        &owner,
        SupplierReturnPostInput {
            return_id: ret2.id,
            idempotency_key: Some("key-return-2".into()),
        },
        "corr-6",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
    assert_eq!(
        on_hand(&state, product, location).await,
        6,
        "rejected return changes nothing"
    );
    assert_eq!(supplier_balance(&state, supplier).await, 5_400);

    let _ = fs::remove_dir_all(&dir);
}
