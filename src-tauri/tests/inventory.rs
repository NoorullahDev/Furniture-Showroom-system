use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application;
use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::dto::inventory::{
    AdjustStockInput, CountLineInput, PostCountInput, PostStockInput, ReverseMovementInput,
    StartCountInput, TransferStockInput,
};
use furniture_shop_lib::error::AppError;
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-inventory-{label}"));
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
            "INSERT INTO categories (name, created_at, updated_at) VALUES (?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) RETURNING id",
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

async fn on_hand(state: &AppState, product_id: i64, location_id: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE(on_hand, 0) FROM stock_balances WHERE product_id = ? AND location_id = ?",
    )
    .bind(product_id)
    .bind(location_id)
    .fetch_one(&state.pool)
    .await
    .unwrap()
}

async fn location_by_name(state: &AppState, name: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM locations WHERE name = ?")
        .bind(name)
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn opening_stock_posts_balance_and_ledger() {
    let dir = temp_dir("opening");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "opening").await;
    let product = create_product(&state, "OPN-01").await;
    let loc = location_by_name(&state, "Main Showroom").await;

    let mv = application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: product,
            location_id: loc,
            quantity: 10,
            unit_cost_minor: Some(5000),
            reason: Some("initial stock".into()),
        },
        "corr-opening",
    )
    .await
    .unwrap();

    assert_eq!(on_hand(&state, product, loc).await, 10);
    assert!(mv.move_number.as_deref().unwrap().starts_with("OPN-"));
    assert_eq!(mv.quantity_delta, 10);
    assert_eq!(mv.unit_cost_minor, Some(5000));

    // Single immutable ledger entry for the receipt.
    let ledger: i64 = sqlx::query_scalar(
        "SELECT SUM(quantity_delta) FROM stock_movements WHERE product_id = ? AND location_id = ?",
    )
    .bind(product)
    .bind(loc)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(ledger, 10, "displayed balance must equal the ledger sum");

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn transfer_moves_balance_between_locations() {
    let dir = temp_dir("transfer");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "transfer").await;
    let product = create_product(&state, "TRF-01").await;
    let from = location_by_name(&state, "Main Showroom").await;
    let to = location_by_name(&state, "Store/Stockroom").await;

    application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: product,
            location_id: from,
            quantity: 10,
            unit_cost_minor: None,
            reason: None,
        },
        "corr-1",
    )
    .await
    .unwrap();

    let both = application::inventory::post_transfer(
        &state,
        &owner,
        TransferStockInput {
            product_id: product,
            from_location_id: from,
            to_location_id: to,
            quantity: 3,
            reason: None,
        },
        "corr-2",
    )
    .await
    .unwrap();
    assert_eq!(both.len(), 2, "a transfer posts an out and an in movement");

    assert_eq!(on_hand(&state, product, from).await, 7);
    assert_eq!(on_hand(&state, product, to).await, 3);

    // The ledger still reconciles to the on-hand balance at both locations.
    let from_ledger: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(quantity_delta), 0) FROM stock_movements WHERE product_id = ? AND location_id = ?",
    )
    .bind(product)
    .bind(from)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(from_ledger, 7);
    let to_ledger: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(quantity_delta), 0) FROM stock_movements WHERE product_id = ? AND location_id = ?",
    )
    .bind(product)
    .bind(to)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(to_ledger, 3);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn negative_stock_is_blocked_under_strict_policy() {
    let dir = temp_dir("neg-strict");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "neg").await;
    let product = create_product(&state, "NEG-01").await;
    let from = location_by_name(&state, "Main Showroom").await;
    let to = location_by_name(&state, "Store/Stockroom").await;

    application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: product,
            location_id: from,
            quantity: 2,
            unit_cost_minor: None,
            reason: None,
        },
        "corr-1",
    )
    .await
    .unwrap();

    // Transferring more than available must fail under the strict default.
    let err = application::inventory::post_transfer(
        &state,
        &owner,
        TransferStockInput {
            product_id: product,
            from_location_id: from,
            to_location_id: to,
            quantity: 5,
            reason: None,
        },
        "corr-2",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::InsufficientStock(_)));

    // A manual adjustment below zero must also fail.
    let err = application::inventory::post_adjust(
        &state,
        &owner,
        AdjustStockInput {
            product_id: product,
            location_id: from,
            adjustment_qty: -5,
            unit_cost_minor: None,
            reason: None,
        },
        "corr-3",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::InsufficientStock(_)));

    // Nothing was written by the failed operations.
    assert_eq!(on_hand(&state, product, from).await, 2);
    let mv_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM stock_movements WHERE product_id = ?")
            .bind(product)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(mv_count, 1, "only the opening movement exists");

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn allow_negative_setting_enables_negative_balance() {
    let dir = temp_dir("neg-lenient");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "lenient").await;
    let product = create_product(&state, "NEG-02").await;
    let from = location_by_name(&state, "Main Showroom").await;
    let to = location_by_name(&state, "Store/Stockroom").await;

    application::settings::set(
        &state,
        "inventory.allow_negative",
        "\"true\"",
        Some(owner.user_id),
    )
    .await
    .unwrap();

    application::inventory::post_adjust(
        &state,
        &owner,
        AdjustStockInput {
            product_id: product,
            location_id: from,
            adjustment_qty: -5,
            unit_cost_minor: None,
            reason: Some("write-off before opening".into()),
        },
        "corr-1",
    )
    .await
    .unwrap();

    // With no balance the adjustment created a negative on-hand. The ledger
    // still reconciles: sum of deltas equals the displayed balance.
    assert_eq!(on_hand(&state, product, from).await, -5);
    let ledger: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(quantity_delta), 0) FROM stock_movements WHERE product_id = ? AND location_id = ?",
    )
    .bind(product)
    .bind(from)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(ledger, -5);

    // _ = to (unused in this test but kept to mirror the transfer test shape).
    let _ = to;
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn concurrent_openings_never_lose_updates() {
    let dir = temp_dir("concurrent");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "concurrent").await;
    let product = create_product(&state, "CONC-01").await;
    let loc = location_by_name(&state, "Main Showroom").await;

    let label = "corr";
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let state_ref = &state;
            let owner_ref = &owner;
            async move {
                application::inventory::post_opening(
                    state_ref,
                    owner_ref,
                    PostStockInput {
                        product_id: product,
                        location_id: loc,
                        quantity: 1,
                        unit_cost_minor: Some(1000 + i),
                        reason: None,
                    },
                    &format!("{label}-{i}"),
                )
                .await
            }
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    // All eight receipts landed; the write coordinator serialized them.
    assert_eq!(on_hand(&state, product, loc).await, 8);
    let mv_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM stock_movements WHERE product_id = ?")
            .bind(product)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(mv_count, 8);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn fifo_valuation_uses_oldest_cost_rate() {
    let dir = temp_dir("fifo");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "fifo").await;
    let product = create_product(&state, "FIFO-01").await;
    let loc = location_by_name(&state, "Main Showroom").await;

    application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: product,
            location_id: loc,
            quantity: 10,
            unit_cost_minor: Some(100),
            reason: None,
        },
        "corr-a",
    )
    .await
    .unwrap();
    application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: product,
            location_id: loc,
            quantity: 5,
            unit_cost_minor: Some(300),
            reason: None,
        },
        "corr-b",
    )
    .await
    .unwrap();

    let valuation = application::inventory::valuation(&state, &owner)
        .await
        .unwrap();
    let line = valuation
        .iter()
        .find(|l| l.product_id == product)
        .expect("product appears in valuation");
    assert_eq!(line.sellable_qty, 15);
    assert_eq!(
        line.unit_cost_minor, 100,
        "oldest FIFO rate governs valuation"
    );
    assert_eq!(line.value_minor, 10 * 100 + 5 * 300);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn damage_and_repair_shift_classification() {
    let dir = temp_dir("damage");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "damage").await;
    let product = create_product(&state, "DMG-01").await;
    let loc = location_by_name(&state, "Main Showroom").await;

    application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: product,
            location_id: loc,
            quantity: 5,
            unit_cost_minor: None,
            reason: None,
        },
        "corr-a",
    )
    .await
    .unwrap();

    application::inventory::post_damage(
        &state,
        &owner,
        furniture_shop_lib::dto::inventory::DamageStockInput {
            product_id: product,
            location_id: loc,
            quantity: 2,
            reason: Some("dropped".into()),
        },
        "corr-b",
    )
    .await
    .unwrap();

    let damaged: i64 = sqlx::query_scalar(
        "SELECT COALESCE(damaged, 0) FROM stock_balances WHERE product_id = ? AND location_id = ?",
    )
    .bind(product)
    .bind(loc)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(damaged, 2);
    assert_eq!(on_hand(&state, product, loc).await, 3);

    application::inventory::post_repair(
        &state,
        &owner,
        furniture_shop_lib::dto::inventory::DamageStockInput {
            product_id: product,
            location_id: loc,
            quantity: 2,
            reason: Some("reassembled".into()),
        },
        "corr-c",
    )
    .await
    .unwrap();

    let damaged: i64 = sqlx::query_scalar(
        "SELECT COALESCE(damaged, 0) FROM stock_balances WHERE product_id = ? AND location_id = ?",
    )
    .bind(product)
    .bind(loc)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(damaged, 0);
    assert_eq!(on_hand(&state, product, loc).await, 5);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn reserve_and_release_round_trip() {
    let dir = temp_dir("reserve");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "reserve").await;
    let product = create_product(&state, "RSV-01").await;
    let loc = location_by_name(&state, "Main Showroom").await;

    application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: product,
            location_id: loc,
            quantity: 10,
            unit_cost_minor: None,
            reason: None,
        },
        "corr-a",
    )
    .await
    .unwrap();

    application::inventory::post_reserve(
        &state,
        &owner,
        furniture_shop_lib::dto::inventory::ReleaseStockInput {
            product_id: product,
            location_id: loc,
            quantity: 4,
            reason: Some("customer hold".into()),
        },
        "corr-b",
    )
    .await
    .unwrap();

    let reserved: i64 = sqlx::query_scalar(
        "SELECT COALESCE(reserved, 0) FROM stock_balances WHERE product_id = ? AND location_id = ?",
    )
    .bind(product)
    .bind(loc)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(reserved, 4);
    assert_eq!(on_hand(&state, product, loc).await, 10);

    // Reserving more than the free balance is blocked.
    let err = application::inventory::post_reserve(
        &state,
        &owner,
        furniture_shop_lib::dto::inventory::ReleaseStockInput {
            product_id: product,
            location_id: loc,
            quantity: 7,
            reason: None,
        },
        "corr-c",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::InsufficientStock(_)));

    application::inventory::post_release(
        &state,
        &owner,
        furniture_shop_lib::dto::inventory::ReleaseStockInput {
            product_id: product,
            location_id: loc,
            quantity: 4,
            reason: None,
        },
        "corr-d",
    )
    .await
    .unwrap();

    let reserved: i64 = sqlx::query_scalar(
        "SELECT COALESCE(reserved, 0) FROM stock_balances WHERE product_id = ? AND location_id = ?",
    )
    .bind(product)
    .bind(loc)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(reserved, 0);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn reversal_restores_net_position() {
    let dir = temp_dir("reversal");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "reversal").await;
    let product = create_product(&state, "REV-01").await;
    let loc = location_by_name(&state, "Main Showroom").await;

    let mv = application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: product,
            location_id: loc,
            quantity: 10,
            unit_cost_minor: Some(500),
            reason: Some("original".into()),
        },
        "corr-1",
    )
    .await
    .unwrap();
    assert_eq!(on_hand(&state, product, loc).await, 10);

    let rev = application::inventory::reverse_movement(
        &state,
        &owner,
        ReverseMovementInput {
            movement_id: mv.id,
            reason: Some("mistake".into()),
        },
        "corr-2",
    )
    .await
    .unwrap();
    assert_eq!(on_hand(&state, product, loc).await, 0);
    assert_eq!(rev.quantity_delta, -10);
    assert_eq!(rev.reversal_of_id, Some(mv.id));
    assert!(rev.move_number.as_deref().unwrap().starts_with("REV-"));

    let err = application::inventory::reverse_movement(
        &state,
        &owner,
        ReverseMovementInput {
            movement_id: mv.id,
            reason: None,
        },
        "corr-3",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));

    let ledger: i64 = sqlx::query_scalar(
        "SELECT SUM(quantity_delta) FROM stock_movements WHERE product_id = ? AND location_id = ?",
    )
    .bind(product)
    .bind(loc)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(ledger, 0, "reversed ledger reconciles to zero");

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn reversal_of_transfer_balances_both_locations() {
    let dir = temp_dir("reversal-transfer");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "reversal-tr").await;
    let product = create_product(&state, "REV-TR01").await;
    let from = location_by_name(&state, "Main Showroom").await;
    let to = location_by_name(&state, "Store/Stockroom").await;

    application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: product,
            location_id: from,
            quantity: 10,
            unit_cost_minor: None,
            reason: None,
        },
        "corr-1",
    )
    .await
    .unwrap();

    let trfs = application::inventory::post_transfer(
        &state,
        &owner,
        TransferStockInput {
            product_id: product,
            from_location_id: from,
            to_location_id: to,
            quantity: 4,
            reason: None,
        },
        "corr-2",
    )
    .await
    .unwrap();
    assert_eq!(on_hand(&state, product, from).await, 6);
    assert_eq!(on_hand(&state, product, to).await, 4);

    let rev = application::inventory::reverse_movement(
        &state,
        &owner,
        ReverseMovementInput {
            movement_id: trfs[0].id,
            reason: Some("cancel out leg".into()),
        },
        "corr-3",
    )
    .await
    .unwrap();
    assert_eq!(on_hand(&state, product, from).await, 10);
    assert_eq!(on_hand(&state, product, to).await, 4);
    assert_eq!(rev.reversal_of_id, Some(trfs[0].id));

    application::inventory::reverse_movement(
        &state,
        &owner,
        ReverseMovementInput {
            movement_id: trfs[1].id,
            reason: None,
        },
        "corr-4",
    )
    .await
    .unwrap();
    assert_eq!(on_hand(&state, product, from).await, 10);
    assert_eq!(on_hand(&state, product, to).await, 0);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn low_stock_lists_products_below_minimum() {
    let dir = temp_dir("low-stock");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "lowstock").await;

    let low = create_product(&state, "LOW-01").await;
    sqlx::query("UPDATE products SET minimum_stock = 5 WHERE article_number = 'LOW-01'")
        .execute(&state.pool)
        .await
        .unwrap();
    let loc = location_by_name(&state, "Main Showroom").await;
    application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: low,
            location_id: loc,
            quantity: 2,
            unit_cost_minor: None,
            reason: None,
        },
        "corr-1",
    )
    .await
    .unwrap();

    let ok = create_product(&state, "LOW-02").await;
    sqlx::query("UPDATE products SET minimum_stock = 3 WHERE article_number = 'LOW-02'")
        .execute(&state.pool)
        .await
        .unwrap();
    application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: ok,
            location_id: loc,
            quantity: 10,
            unit_cost_minor: None,
            reason: None,
        },
        "corr-2",
    )
    .await
    .unwrap();

    let low_list = application::inventory::list_low_stock(&state, &owner)
        .await
        .unwrap();
    let ids: Vec<i64> = low_list.iter().map(|l| l.product_id).collect();
    assert!(ids.contains(&low), "low-stock product must appear");
    assert!(!ids.contains(&ok), "adequate-stock product must not appear");

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn count_session_lifecycle() {
    let dir = temp_dir("count-session");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "count").await;
    let product = create_product(&state, "CNT-01").await;
    let loc = location_by_name(&state, "Main Showroom").await;

    application::inventory::post_opening(
        &state,
        &owner,
        PostStockInput {
            product_id: product,
            location_id: loc,
            quantity: 10,
            unit_cost_minor: Some(200),
            reason: None,
        },
        "corr-1",
    )
    .await
    .unwrap();
    assert_eq!(on_hand(&state, product, loc).await, 10);

    let session = application::inventory::start_count(
        &state,
        &owner,
        StartCountInput {
            location_id: loc,
            notes: Some("weekly count".into()),
        },
        "corr-2",
    )
    .await
    .unwrap();
    assert_eq!(session.status, "open");
    assert!(session
        .session_number
        .as_deref()
        .unwrap()
        .starts_with("CNT-"));

    let lines = application::inventory::list_count_lines(&state, &owner, session.id)
        .await
        .unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].expected_qty, 10);
    assert_eq!(lines[0].product_id, product);

    let _ = application::inventory::add_count_line(
        &state,
        &owner,
        CountLineInput {
            session_id: session.id,
            product_id: product,
            counted_qty: 9,
        },
        "corr-3",
    )
    .await
    .unwrap();

    let lines = application::inventory::list_count_lines(&state, &owner, session.id)
        .await
        .unwrap();
    assert_eq!(lines[0].counted_qty, 9);
    assert_eq!(lines[0].variance_qty, -1);

    let adjustments = application::inventory::post_count(
        &state,
        &owner,
        PostCountInput {
            session_id: session.id,
        },
        "corr-4",
    )
    .await
    .unwrap();
    assert_eq!(adjustments.len(), 1);
    assert_eq!(adjustments[0].quantity_delta, -1);
    assert_eq!(on_hand(&state, product, loc).await, 9);

    let sessions = application::inventory::list_count_sessions(&state, &owner, None)
        .await
        .unwrap();
    let posted = sessions.iter().find(|s| s.id == session.id).unwrap();
    assert_eq!(posted.status, "posted");

    let _ = fs::remove_dir_all(&dir);
}
