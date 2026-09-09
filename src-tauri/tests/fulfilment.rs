use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application;
use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::dto::fulfilment::{
    DamageDecisionInput, DamageRecordInput, DeliveryCreateInput, DeliveryItemInput,
    DeliveryTransitionInput, ReturnItemInput, ReturnVoidInput, SaleReturnInput,
};
use furniture_shop_lib::dto::purchases::{
    CashAccountInput, PurchaseCreateInput, PurchaseItemInput, PurchasePostInput, SupplierInput,
};
use furniture_shop_lib::dto::sales::{
    CustomerInput, SaleConfirmInput, SaleCreateInput, SaleItemInput,
};
use furniture_shop_lib::error::AppError;
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-fulfilment-{label}"));
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

async fn damaged(state: &AppState, product_id: i64, location_id: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE(damaged, 0) FROM stock_balances WHERE product_id = ? AND location_id = ?",
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

async fn sold_sale_item(state: &AppState, sale_id: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM sale_items WHERE sale_id = ? ORDER BY id LIMIT 1")
        .bind(sale_id)
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

async fn confirm_paid(
    state: &AppState,
    owner: &Principal,
    sale_id: i64,
    paid: i64,
    cash_account_id: i64,
) {
    application::sales::confirm_sale(
        state,
        owner,
        SaleConfirmInput {
            sale_id,
            idempotency_key: Some(format!("cnf-{sale_id}")),
            paid_minor: Some(paid),
            cash_account_id: Some(cash_account_id),
            payment_method_id: Some(1),
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-confirm",
    )
    .await
    .expect("confirm sale");
}

// ---------------------------------------------------------------------------
// Deliveries
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delivery_lifecycle_tracks_status_and_blocks_over_delivery() {
    let dir = temp_dir("delivery");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "delivery").await;
    let product = stock_product(&state, &owner, "DLV-01", 10, 5_000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let customer = create_customer(&state, &owner, "CUST-DLV").await;
    let cash = funded_cash(&state, &owner, "DLVCASH").await;

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
    confirm_paid(&state, &owner, sale.id, 30_000, cash).await;

    let sale_item = sold_sale_item(&state, sale.id).await;

    let delivery = application::fulfilment::create_delivery(
        &state,
        &owner,
        DeliveryCreateInput {
            sale_id: sale.id,
            scheduled_at: Some("2026-09-06".into()),
            address: None,
            contact_name: Some("Ahmed".into()),
            contact_phone: None,
            driver_note: None,
            vehicle_note: None,
            delivery_charge_minor: Some(500),
            notes: None,
            items: vec![DeliveryItemInput {
                sale_item_id: sale_item,
                quantity: 2,
            }],
        },
        "corr-deliv",
    )
    .await
    .expect("create delivery");

    assert!(delivery
        .delivery_number
        .as_deref()
        .unwrap()
        .starts_with("DLV-"));
    assert_eq!(delivery.status, "pending");
    assert_eq!(delivery.delivery_charge_minor, 500);

    let mark = |action: &str| DeliveryTransitionInput {
        delivery_id: delivery.id,
        action: action.into(),
        reason: None,
        scheduled_at: None,
        receiver_name: None,
        proof_reference: None,
    };

    let ready = application::fulfilment::transition_delivery(&state, &owner, mark("ready"), "c2")
        .await
        .expect("ready");
    assert_eq!(ready.status, "ready");

    let dispatched =
        application::fulfilment::transition_delivery(&state, &owner, mark("dispatched"), "c3")
            .await
            .expect("dispatched");
    assert_eq!(dispatched.status, "dispatched");

    let mut delivered_input = mark("delivered");
    delivered_input.receiver_name = Some("Bilal".into());
    let delivered =
        application::fulfilment::transition_delivery(&state, &owner, delivered_input, "c4")
            .await
            .expect("delivered");
    assert_eq!(delivered.status, "delivered");
    assert_eq!(delivered.receiver_name.as_deref(), Some("Bilal"));

    // A delivered delivery cannot be rescheduled or cancelled.
    let resched =
        application::fulfilment::transition_delivery(&state, &owner, mark("cancelled"), "c5").await;
    assert!(matches!(resched, Err(AppError::Conflict(_))));

    // The remaining unit can still be delivered.
    let d2 = application::fulfilment::create_delivery(
        &state,
        &owner,
        DeliveryCreateInput {
            sale_id: sale.id,
            scheduled_at: None,
            address: None,
            contact_name: None,
            contact_phone: None,
            driver_note: None,
            vehicle_note: None,
            delivery_charge_minor: None,
            notes: None,
            items: vec![DeliveryItemInput {
                sale_item_id: sale_item,
                quantity: 1,
            }],
        },
        "corr-deliv2",
    )
    .await
    .expect("second delivery");
    assert_eq!(d2.status, "pending");

    // Any further delivery over the net sold quantity is rejected.
    let over = application::fulfilment::create_delivery(
        &state,
        &owner,
        DeliveryCreateInput {
            sale_id: sale.id,
            scheduled_at: None,
            address: None,
            contact_name: None,
            contact_phone: None,
            driver_note: None,
            vehicle_note: None,
            delivery_charge_minor: None,
            notes: None,
            items: vec![DeliveryItemInput {
                sale_item_id: sale_item,
                quantity: 1,
            }],
        },
        "corr-over",
    )
    .await;
    assert!(matches!(over, Err(AppError::Validation(_))));

    // Cancelling a delivery frees its quantity for a replacement.
    let cancel = application::fulfilment::transition_delivery(
        &state,
        &owner,
        DeliveryTransitionInput {
            delivery_id: d2.id,
            action: "cancelled".into(),
            reason: Some("customer moved".into()),
            scheduled_at: None,
            receiver_name: None,
            proof_reference: None,
        },
        "c6",
    )
    .await
    .expect("cancel pending delivery");
    assert_eq!(cancel.status, "cancelled");

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn delivery_failed_can_be_rescheduled() {
    let dir = temp_dir("reschedule");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "reschedule").await;
    let product = stock_product(&state, &owner, "DLV-02", 10, 5_000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let cash = funded_cash(&state, &owner, "RSCHCASH").await;

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
            items: vec![line(product, 2)],
        },
        "corr-1",
    )
    .await
    .unwrap();
    confirm_paid(&state, &owner, sale.id, 20_000, cash).await;
    let sale_item = sold_sale_item(&state, sale.id).await;

    let delivery = application::fulfilment::create_delivery(
        &state,
        &owner,
        DeliveryCreateInput {
            sale_id: sale.id,
            scheduled_at: Some("2026-09-06".into()),
            address: None,
            contact_name: None,
            contact_phone: None,
            driver_note: None,
            vehicle_note: None,
            delivery_charge_minor: None,
            notes: None,
            items: vec![DeliveryItemInput {
                sale_item_id: sale_item,
                quantity: 1,
            }],
        },
        "corr-d",
    )
    .await
    .unwrap();

    let fail = application::fulfilment::transition_delivery(
        &state,
        &owner,
        DeliveryTransitionInput {
            delivery_id: delivery.id,
            action: "failed".into(),
            reason: Some("truck broke down".into()),
            scheduled_at: None,
            receiver_name: None,
            proof_reference: None,
        },
        "c2",
    )
    .await
    .expect("fail");
    assert_eq!(fail.status, "failed");
    assert_eq!(fail.failed_reason.as_deref(), Some("truck broke down"));

    let rescheduled = application::fulfilment::reschedule_delivery(
        &state,
        &owner,
        furniture_shop_lib::dto::fulfilment::DeliveryRescheduleInput {
            delivery_id: delivery.id,
            scheduled_at: "2026-09-08".into(),
            reason: Some("retry".into()),
        },
        "c3",
    )
    .await
    .expect("reschedule");
    assert_eq!(rescheduled.status, "pending");
    assert_eq!(rescheduled.reschedule_count, 1);

    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Returns, refunds, credit notes, exchanges
// ---------------------------------------------------------------------------

#[tokio::test]
async fn partial_cash_return_refunds_exactly_and_rebuilds_stock() {
    let dir = temp_dir("return-cash");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "return-cash").await;
    let product = stock_product(&state, &owner, "RET-01", 10, 5_000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let customer = create_customer(&state, &owner, "CUST-RET").await;
    let cash = funded_cash(&state, &owner, "RETCASH").await;

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
    confirm_paid(&state, &owner, sale.id, 30_000, cash).await;

    assert_eq!(on_hand(&state, product, location).await, 7);
    assert_eq!(cash_balance(&state, cash).await, 1_030_000);
    assert_eq!(customer_balance(&state, customer).await, 0);

    let sale_item = sold_sale_item(&state, sale.id).await;

    let ret = application::fulfilment::post_return(
        &state,
        &owner,
        SaleReturnInput {
            sale_id: sale.id,
            refund_type: "cash".into(),
            return_date: "2026-09-10".into(),
            items: vec![ReturnItemInput {
                sale_item_id: sale_item,
                quantity: 1,
                classification: "sellable".into(),
            }],
            cash_account_id: Some(cash),
            notes: None,
            idempotency_key: Some("key-ret-1".into()),
        },
        "corr-ret",
    )
    .await
    .expect("post cash return");

    // 1 unit returned: refund = per-unit price of the returned unit, with any
    // discount allocated proportionally across the line (none here).
    assert!(ret.return_number.as_deref().unwrap().starts_with("RET-"));
    assert_eq!(ret.total_minor, 10_000);
    assert_eq!(ret.total_refund_minor, ret.total_minor);
    assert_eq!(ret.cash_refund_minor, ret.total_minor);
    assert_eq!(ret.status, "posted");

    assert_eq!(on_hand(&state, product, location).await, 8);
    assert_eq!(
        cash_balance(&state, cash).await,
        1_030_000 - ret.total_minor
    );
    assert_eq!(customer_balance(&state, customer).await, 0);

    // Ledger recorded the chargeback and the refund separately.
    let ledger_entries: Vec<String> = sqlx::query_scalar(
        "SELECT entry_type FROM customer_ledger_entries WHERE customer_id = ? ORDER BY id",
    )
    .bind(customer)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(
        ledger_entries,
        vec!["sale", "payment", "sales_return", "payment_refund"]
    );

    // A second return of 3 units exceeds the remaining 2 and is rejected.
    let over = application::fulfilment::post_return(
        &state,
        &owner,
        SaleReturnInput {
            sale_id: sale.id,
            refund_type: "cash".into(),
            return_date: "2026-09-10".into(),
            items: vec![ReturnItemInput {
                sale_item_id: sale_item,
                quantity: 3,
                classification: "sellable".into(),
            }],
            cash_account_id: Some(cash),
            notes: None,
            idempotency_key: None,
        },
        "corr-over",
    )
    .await;
    assert!(matches!(over, Err(AppError::Validation(_))));

    // Replaying the same idempotency key returns the original document.
    let replay = application::fulfilment::post_return(
        &state,
        &owner,
        SaleReturnInput {
            sale_id: sale.id,
            refund_type: "cash".into(),
            return_date: "2026-09-10".into(),
            items: vec![ReturnItemInput {
                sale_item_id: sale_item,
                quantity: 1,
                classification: "sellable".into(),
            }],
            cash_account_id: Some(cash),
            notes: None,
            idempotency_key: Some("key-ret-1".into()),
        },
        "corr-ret2",
    )
    .await
    .expect("retry is idempotent");
    assert_eq!(replay.id, ret.id);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn credit_return_creates_credit_note_and_exchange_applies_it() {
    let dir = temp_dir("exchange");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "exchange").await;
    let product = stock_product(&state, &owner, "RET-02", 10, 5_000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let customer = create_customer(&state, &owner, "CUST-EXC").await;
    let cash = funded_cash(&state, &owner, "EXCASH").await;

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
    confirm_paid(&state, &owner, sale.id, 30_000, cash).await;
    let sale_item = sold_sale_item(&state, sale.id).await;

    // Return 1 unit as an exchange: refund becomes a credit note.
    let ret = application::fulfilment::post_return(
        &state,
        &owner,
        SaleReturnInput {
            sale_id: sale.id,
            refund_type: "exchange".into(),
            return_date: "2026-09-10".into(),
            items: vec![ReturnItemInput {
                sale_item_id: sale_item,
                quantity: 1,
                classification: "sellable".into(),
            }],
            cash_account_id: None,
            notes: Some("exchange".into()),
            idempotency_key: Some("key-exc".into()),
        },
        "corr-ret",
    )
    .await
    .expect("exchange return");

    let refund = ret.total_minor;
    assert_eq!(refund, 10_000);
    assert_eq!(ret.cash_refund_minor, 0);
    assert_eq!(ret.credit_note_minor, refund);
    // The returned value was reduced from the customer's account: a credit balance.
    assert_eq!(customer_balance(&state, customer).await, -refund);
    assert_eq!(cash_balance(&state, cash).await, 1_030_000, "no cash left");

    let notes: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, status FROM credit_notes WHERE return_id = ?")
            .bind(ret.id)
            .fetch_all(&state.pool)
            .await
            .unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].1, "open");
    let (credit_note_id, _) = notes[0];

    // Exchange: sell a replacement charged against the credit note.
    let exchange = application::sales::create_sale(
        &state,
        &owner,
        SaleCreateInput {
            location_id: location,
            customer_id: Some(customer),
            kind: Some("sale".into()),
            sale_date: Some("2026-09-11".into()),
            discount_minor: Some(0),
            delivery_charge_minor: Some(0),
            notes: None,
            items: vec![line(product, 1)],
        },
        "corr-2",
    )
    .await
    .unwrap();
    assert_eq!(exchange.total_minor, 10_000);

    // Asking the exchange to consume more credit than the note holds fails.
    let mismatched = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: exchange.id,
            idempotency_key: Some("cnf-exc-bad".into()),
            paid_minor: Some(0),
            cash_account_id: Some(cash),
            payment_method_id: Some(1),
            advance_used_minor: Some(refund + 1),
            credit_note_id: Some(credit_note_id),
        },
        "corr-bad",
    )
    .await;
    assert!(matches!(mismatched, Err(AppError::Validation(_))));

    let applied = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: exchange.id,
            idempotency_key: Some("cnf-exc".into()),
            paid_minor: Some(0),
            cash_account_id: Some(cash),
            payment_method_id: Some(1),
            advance_used_minor: Some(refund),
            credit_note_id: Some(credit_note_id),
        },
        "corr-3",
    )
    .await
    .expect("apply credit note");
    assert_eq!(applied.due_minor, 10_000 - refund, "partial exchange");

    let note_status: String = sqlx::query_scalar("SELECT status FROM credit_notes WHERE id = ?")
        .bind(credit_note_id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(note_status, "applied");
    // The credit balance was consumed by the exchange.
    assert_eq!(customer_balance(&state, customer).await, -(10_000 - refund));

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn void_return_reverses_stock_cash_ledger_and_due() {
    let dir = temp_dir("void-return");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "void-return").await;
    let product = stock_product(&state, &owner, "RET-03", 10, 5_000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let customer = create_customer(&state, &owner, "CUST-VOID").await;
    let cash = funded_cash(&state, &owner, "VOIDCASH").await;

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
    confirm_paid(&state, &owner, sale.id, 30_000, cash).await;
    let sale_item = sold_sale_item(&state, sale.id).await;

    let ret = application::fulfilment::post_return(
        &state,
        &owner,
        SaleReturnInput {
            sale_id: sale.id,
            refund_type: "cash".into(),
            return_date: "2026-09-10".into(),
            items: vec![ReturnItemInput {
                sale_item_id: sale_item,
                quantity: 1,
                classification: "sellable".into(),
            }],
            cash_account_id: Some(cash),
            notes: None,
            idempotency_key: Some("key-void".into()),
        },
        "corr-ret",
    )
    .await
    .unwrap();
    let refund = ret.total_minor;

    assert_eq!(on_hand(&state, product, location).await, 8);
    assert_eq!(cash_balance(&state, cash).await, 1_030_000 - refund);

    application::fulfilment::void_return(
        &state,
        &owner,
        ReturnVoidInput {
            return_id: ret.id,
            reason: Some("customer brought it back".into()),
        },
        "corr-void",
    )
    .await
    .expect("void return");

    let status: String = sqlx::query_scalar("SELECT status FROM sales_returns WHERE id = ?")
        .bind(ret.id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(status, "voided");

    assert_eq!(
        on_hand(&state, product, location).await,
        7,
        "stock restored to post-sale level"
    );
    assert_eq!(
        cash_balance(&state, cash).await,
        1_030_000,
        "cash refund reversed"
    );
    assert_eq!(
        customer_balance(&state, customer).await,
        0,
        "ledger restored"
    );
    let due: i64 = sqlx::query_scalar("SELECT due_minor FROM sales WHERE id = ?")
        .bind(sale.id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(due, 0, "paid sale due stays zero after void");

    // A voided return cannot be voided twice.
    let again = application::fulfilment::void_return(
        &state,
        &owner,
        ReturnVoidInput {
            return_id: ret.id,
            reason: None,
        },
        "corr-void2",
    )
    .await;
    assert!(matches!(again, Err(AppError::Conflict(_))));

    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Damage records
// ---------------------------------------------------------------------------

#[tokio::test]
async fn damage_record_and_resolution_reconcile_ledger() {
    let dir = temp_dir("damage");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "damage").await;
    let product = stock_product(&state, &owner, "DMG-01", 10, 5_000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;

    let rec = application::fulfilment::record_damage(
        &state,
        &owner,
        DamageRecordInput {
            product_id: product,
            location_id: location,
            quantity: 2,
            damage_date: "2026-09-10".into(),
            source: "in_hand".into(),
            reason: Some("cracked during handling".into()),
            estimated_loss_minor: Some(10_000),
            photo_path: None,
        },
        "corr-dmg",
    )
    .await
    .expect("record damage");

    assert!(rec.damage_number.as_deref().unwrap().starts_with("DMG-"));
    assert_eq!(rec.status, "open");
    assert_eq!(on_hand(&state, product, location).await, 8);
    assert_eq!(damaged(&state, product, location).await, 2);

    // Repair recovers the units back to sellable stock.
    let repaired = application::fulfilment::decide_damage(
        &state,
        &owner,
        DamageDecisionInput {
            damage_id: rec.id,
            decision: "repair".into(),
            decision_note: Some("refurbished".into()),
            linked_sale_id: None,
        },
        "corr-repair",
    )
    .await
    .expect("repair damage");
    assert_eq!(repaired.status, "resolved");
    assert_eq!(repaired.decision.as_deref(), Some("repair"));
    assert_eq!(on_hand(&state, product, location).await, 10);
    assert_eq!(damaged(&state, product, location).await, 0);

    // Second record resolved as a write-off removes the units entirely.
    let rec2 = application::fulfilment::record_damage(
        &state,
        &owner,
        DamageRecordInput {
            product_id: product,
            location_id: location,
            quantity: 3,
            damage_date: "2026-09-11".into(),
            source: "count".into(),
            reason: Some("shelf collapse".into()),
            estimated_loss_minor: Some(15_000),
            photo_path: None,
        },
        "corr-dmg2",
    )
    .await
    .expect("second damage");
    assert_eq!(on_hand(&state, product, location).await, 7);
    assert_eq!(damaged(&state, product, location).await, 3);

    application::fulfilment::decide_damage(
        &state,
        &owner,
        DamageDecisionInput {
            damage_id: rec2.id,
            decision: "write_off".into(),
            decision_note: Some("unsalvageable".into()),
            linked_sale_id: None,
        },
        "corr-wo",
    )
    .await
    .expect("write off");

    assert_eq!(on_hand(&state, product, location).await, 7);
    assert_eq!(damaged(&state, product, location).await, 0);

    // Inventory ledger reconciles with the displayed total balance.
    let recovered: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(quantity_delta), 0) FROM stock_movements
         WHERE product_id = ? AND location_id = ? AND movement_type = 'repair_recovery'",
    )
    .bind(product)
    .bind(location)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(recovered, 2, "repair restored both units");
    let repair_moves: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM stock_movements
         WHERE reference_type = 'damage_record' AND product_id = ? AND location_id = ?",
    )
    .bind(product)
    .bind(location)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(
        repair_moves, 2,
        "repair + write-off carry the damage reference"
    );
    let record_moves: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM stock_movements
         WHERE movement_type = 'damage' AND product_id = ? AND location_id = ?",
    )
    .bind(product)
    .bind(location)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(record_moves, 2, "one damage movement per record");

    // Resolving an already-resolved record is rejected.
    let again = application::fulfilment::decide_damage(
        &state,
        &owner,
        DamageDecisionInput {
            damage_id: rec2.id,
            decision: "supplier_return".into(),
            decision_note: None,
            linked_sale_id: None,
        },
        "corr-again",
    )
    .await;
    assert!(matches!(again, Err(AppError::Conflict(_))));

    // Recording more damage than is on hand is rejected.
    let over = application::fulfilment::record_damage(
        &state,
        &owner,
        DamageRecordInput {
            product_id: product,
            location_id: location,
            quantity: 99,
            damage_date: "2026-09-11".into(),
            source: "other".into(),
            reason: None,
            estimated_loss_minor: None,
            photo_path: None,
        },
        "corr-over",
    )
    .await;
    assert!(matches!(over, Err(AppError::InsufficientStock(_))));

    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Permission gating
// ---------------------------------------------------------------------------

#[tokio::test]
async fn phase8_permissions_are_enforced() {
    let dir = temp_dir("perms");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "perm-owner").await;
    let product = stock_product(&state, &owner, "PER-01", 10, 5_000).await;
    set_price(&state, product, 10_000).await;
    let location = main_location(&state).await;
    let customer = create_customer(&state, &owner, "CUST-PERM").await;
    let cash = funded_cash(&state, &owner, "PERMCASH").await;

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
    confirm_paid(&state, &owner, sale.id, 10_000, cash).await;
    let sale_item = sold_sale_item(&state, sale.id).await;

    let storekeeper = make_role_principal(&state, "storekeeper", "storekeeper").await;
    let salesperson = make_role_principal(&state, "salesperson", "salesperson").await;

    // storekeeper: damage in-scope, deliveries in-scope, returns out-of-scope.
    assert!(storekeeper
        .permissions
        .contains(&"damage.record".to_string()));
    assert!(!storekeeper.permissions.contains(&"sale.return".to_string()));
    let rejected_return = application::fulfilment::post_return(
        &state,
        &storekeeper,
        SaleReturnInput {
            sale_id: sale.id,
            refund_type: "cash".into(),
            return_date: "2026-09-10".into(),
            items: vec![ReturnItemInput {
                sale_item_id: sale_item,
                quantity: 1,
                classification: "sellable".into(),
            }],
            cash_account_id: Some(cash),
            notes: None,
            idempotency_key: None,
        },
        "corr-perm",
    )
    .await;
    assert!(matches!(rejected_return, Err(AppError::Unauthorized(_))));

    // salesperson: delivery create is out-of-scope, updating is in-scope.
    assert!(salesperson
        .permissions
        .contains(&"delivery.update".to_string()));
    assert!(!salesperson
        .permissions
        .contains(&"delivery.create".to_string()));
    let create_delivery = application::fulfilment::create_delivery(
        &state,
        &salesperson,
        DeliveryCreateInput {
            sale_id: sale.id,
            scheduled_at: None,
            address: None,
            contact_name: None,
            contact_phone: None,
            driver_note: None,
            vehicle_note: None,
            delivery_charge_minor: None,
            notes: None,
            items: vec![DeliveryItemInput {
                sale_item_id: sale_item,
                quantity: 1,
            }],
        },
        "corr-perm2",
    )
    .await;
    assert!(matches!(create_delivery, Err(AppError::Unauthorized(_))));

    // Owner still holds every Phase 8 permission via the role grant.
    for p in [
        "delivery.create",
        "delivery.view",
        "delivery.update",
        "sale.return",
        "credit.note.use",
        "damage.record",
    ] {
        assert!(
            owner.permissions.contains(&p.to_string()),
            "owner missing {p}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}
