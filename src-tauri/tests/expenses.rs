use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application;
use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::dto::expenses::{
    ExpenseCategoryInput, ExpenseCategoryUpdateInput, ExpenseInput, ExpensePageInput,
    ExpenseReverseInput, OwnerTransactionInput,
};
use furniture_shop_lib::dto::purchases::{
    CashAccountInput, PurchaseCreateInput, PurchaseItemInput, PurchasePostInput, SupplierInput,
    SupplierPaymentInput,
};
use furniture_shop_lib::dto::reports::{ExportFormat, ReportFilterInput};
use furniture_shop_lib::dto::sales::{
    CustomerInput, CustomerReceiptInput, SaleConfirmInput, SaleCreateInput, SaleItemInput,
};
use furniture_shop_lib::error::AppError;
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-expenses-{label}"));
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

async fn cash_balance_sql(state: &AppState, account_id: i64) -> i64 {
    sqlx::query_scalar("SELECT balance_minor FROM cash_accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

async fn cash_entry_count(state: &AppState, entry_type: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM cash_entries WHERE entry_type = ?")
        .bind(entry_type)
        .fetch_one(&state.pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Exit criterion 1: an expense appears exactly once in the profit report and
// exactly once in the cash account book, and the posting is idempotent.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn expense_posts_once_in_report_and_cash_with_idempotency() {
    let dir = temp_dir("once");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let account = funded_cash(&state, &owner, "EXP01").await;
    let before = cash_balance_sql(&state, account).await;

    let category: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'rent'")
        .fetch_one(&state.pool)
        .await
        .unwrap();

    let expense = application::expenses::expense_post(
        &state,
        &owner,
        ExpenseInput {
            category_id: category,
            amount_minor: 50_000,
            expense_date: "2026-09-05".into(),
            cash_account_id: account,
            payment_method_id: 1,
            description: "September showroom rent".into(),
            payee: Some("Landlord".into()),
            reference: None,
            attachment_path: None,
            idempotency_key: Some("exp-once-1".into()),
        },
        "corr-e1",
    )
    .await
    .unwrap();

    assert_eq!(expense.status, "posted");
    assert_eq!(expense.expense_number.as_deref(), Some("EXP-000001"));
    assert_eq!(expense.amount_minor, 50_000);

    // Cash fell by exactly the expense once, and refs a single 'expense' entry.
    assert_eq!(cash_balance_sql(&state, account).await, before - 50_000);
    assert_eq!(cash_entry_count(&state, "expense").await, 1);

    // Replay the same idempotency key: same document, nothing duplicated.
    let replay = application::expenses::expense_post(
        &state,
        &owner,
        ExpenseInput {
            category_id: category,
            amount_minor: 50_000,
            expense_date: "2026-09-05".into(),
            cash_account_id: account,
            payment_method_id: 1,
            description: "September showroom rent".into(),
            payee: None,
            reference: None,
            attachment_path: None,
            idempotency_key: Some("exp-once-1".into()),
        },
        "corr-e2",
    )
    .await
    .unwrap();
    assert_eq!(replay.id, expense.id);
    assert_eq!(cash_balance_sql(&state, account).await, before - 50_000);
    assert_eq!(cash_entry_count(&state, "expense").await, 1);

    let posted_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM expenses WHERE status = 'posted'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(posted_rows, 1);

    // The profit report shows it once, on the document date.
    let profit = application::expenses::profit_summary(
        &state,
        &owner,
        Some("2026-09-01".into()),
        Some("2026-09-30".into()),
    )
    .await
    .unwrap();
    assert_eq!(profit.expenses_minor, 50_000);
    assert_eq!(profit.operational_profit_minor, -50_000);

    // Cash movements are bracketed by created_at, so use the full range.
    let profit_all = application::expenses::profit_summary(&state, &owner, None, None)
        .await
        .unwrap();
    assert_eq!(profit_all.cash_outflow_minor, 50_000);

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Reversing a posted expense refunds the cash, writes a reversal entry, and
// removes the expense from the profit report for that range.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reverse_expense_refunds_cash_and_leaves_profit() {
    let dir = temp_dir("reverse");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let account = funded_cash(&state, &owner, "REV01").await;
    let before = cash_balance_sql(&state, account).await;

    let expense = application::expenses::expense_post(
        &state,
        &owner,
        ExpenseInput {
            category_id: 1,
            amount_minor: 20_000,
            expense_date: "2026-09-06".into(),
            cash_account_id: account,
            payment_method_id: 1,
            description: "Electricity bill".into(),
            payee: None,
            reference: None,
            attachment_path: None,
            idempotency_key: Some("exp-rev-1".into()),
        },
        "corr-r1",
    )
    .await
    .unwrap();
    assert_eq!(cash_balance_sql(&state, account).await, before - 20_000);

    let reversed = application::expenses::expense_reverse(
        &state,
        &owner,
        ExpenseReverseInput {
            expense_id: expense.id,
            reason: "overpaid, corrected".into(),
        },
        "corr-r2",
    )
    .await
    .unwrap();
    assert_eq!(reversed.status, "reversed");
    assert_eq!(
        reversed.reversal_reason.as_deref(),
        Some("overpaid, corrected")
    );

    // Cash is back, reversal writes a matching +cash entry.
    assert_eq!(cash_balance_sql(&state, account).await, before);
    assert_eq!(cash_entry_count(&state, "expense_reversal").await, 1);

    // Reversing twice is rejected.
    let err = application::expenses::expense_reverse(
        &state,
        &owner,
        ExpenseReverseInput {
            expense_id: expense.id,
            reason: "again".into(),
        },
        "corr-r3",
    )
    .await
    .expect_err("second reverse must fail");
    assert!(matches!(err, AppError::Validation(_)));

    // Profit no longer includes the expense.
    let profit = application::expenses::profit_summary(&state, &owner, None, None)
        .await
        .unwrap();
    assert_eq!(profit.expenses_minor, 0);
    assert_eq!(profit.cash_outflow_minor, 20_000); // money moved, but not a loss
    assert_eq!(profit.operational_profit_minor, 0);

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Exit criteria 2 & 3: a customer receipt and a supplier payment move cash but
// are never counted as revenue or cost again; the profit fixture matches the
// manual calculation.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn profit_matches_manual_calculation_and_cash_is_separate() {
    let dir = temp_dir("profit");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let account = funded_cash(&state, &owner, "PFT01").await;
    let location = main_location(&state).await;

    // Stock at cost 500/unit, retail at 2000.
    let product = stock_product(&state, &owner, "P-PFT", 10, 500).await;
    set_price(&state, product, 2_000).await;
    let supplier: i64 = sqlx::query_scalar("SELECT id FROM suppliers WHERE code = 'SUP-P-PFT'")
        .fetch_one(&state.pool)
        .await
        .unwrap();

    // Credit sale of 2 units @ 2000 + 300 delivery = 4,300 confirmed (paid 0).
    let customer = create_customer(&state, &owner, "C-PFT").await;
    let draft = application::sales::create_sale(
        &state,
        &owner,
        SaleCreateInput {
            location_id: location,
            customer_id: Some(customer),
            kind: Some("sale".into()),
            sale_date: Some("2026-09-10".into()),
            discount_minor: Some(0),
            delivery_charge_minor: Some(300),
            notes: None,
            items: vec![SaleItemInput {
                product_id: Some(product),
                bundle_id: None,
                quantity: 2,
            }],
        },
        "corr-s1",
    )
    .await
    .unwrap();
    let sale = application::sales::confirm_sale(
        &state,
        &owner,
        SaleConfirmInput {
            sale_id: draft.id,
            idempotency_key: Some("sale-pft".into()),
            paid_minor: Some(0),
            cash_account_id: None,
            payment_method_id: None,
            advance_used_minor: Some(0),
            credit_note_id: None,
        },
        "corr-s2",
    )
    .await
    .unwrap();
    assert_eq!(sale.total_minor, 4_300);

    // Collect the 4,300 later as a standalone receipt.
    let method = payment_method(&state).await;
    let receipt = application::customers::create_receipt(
        &state,
        &owner,
        CustomerReceiptInput {
            customer_id: customer,
            payment_method_id: method,
            cash_account_id: account,
            payment_date: "2026-09-12".into(),
            amount_minor: 4_300,
            notes: None,
            idempotency_key: Some("rcpt-pft".into()),
            allocations: None,
        },
        "corr-r",
    )
    .await
    .unwrap();
    assert_eq!(receipt.status, "posted");

    // Post a supplier payment against the (credit) purchase as well.
    application::purchases::create_payment(
        &state,
        &owner,
        SupplierPaymentInput {
            supplier_id: supplier,
            payment_method_id: method,
            cash_account_id: account,
            payment_date: "2026-09-13".into(),
            amount_minor: 5_000,
            notes: None,
            idempotency_key: Some("pay-pft".into()),
        },
        "corr-pay",
    )
    .await
    .unwrap();

    // Cash movements are bracketed by created_at, so use the full range.
    let profit = application::expenses::profit_summary(&state, &owner, None, None)
        .await
        .unwrap();
    assert_eq!(
        profit.revenue_minor, 4_300,
        "customer receipt must not raise revenue"
    );
    assert_eq!(profit.delivery_income_minor, 300);
    assert_eq!(
        profit.cogs_minor, 1_000,
        "supplier payment must not raise COGS"
    );
    assert_eq!(profit.gross_profit_minor, 3_300);
    assert_eq!(profit.expenses_minor, 0);
    assert_eq!(profit.damage_loss_minor, 0);
    assert_eq!(profit.operational_profit_minor, 3_300);
    assert_eq!(profit.cash_inflow_minor, 4_300);
    assert_eq!(profit.cash_outflow_minor, 5_000);
    assert_eq!(profit.net_cash_flow_minor, -700);

    // Cash account reconciles with the billed figures.
    let net = 1_000_000 + 4_300 - 5_000;
    assert_eq!(cash_balance_sql(&state, account).await, net);

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Damage write-offs are a documented loss that reduces operational profit.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn damage_writeoff_is_reported_as_loss() {
    let dir = temp_dir("damage");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let location = main_location(&state).await;
    let product = stock_product(&state, &owner, "P-DMG", 5, 300).await;

    let damage = application::fulfilment::record_damage(
        &state,
        &owner,
        furniture_shop_lib::dto::fulfilment::DamageRecordInput {
            product_id: product,
            location_id: location,
            quantity: 1,
            damage_date: "2026-09-08".into(),
            source: "in_hand".into(),
            reason: Some("fell over".into()),
            estimated_loss_minor: Some(300),
            photo_path: None,
        },
        "corr-d1",
    )
    .await
    .unwrap();
    application::fulfilment::decide_damage(
        &state,
        &owner,
        furniture_shop_lib::dto::fulfilment::DamageDecisionInput {
            damage_id: damage.id,
            decision: "write_off".into(),
            decision_note: Some("worthless".into()),
            linked_sale_id: None,
        },
        "corr-d2",
    )
    .await
    .unwrap();

    let profit = application::expenses::profit_summary(
        &state,
        &owner,
        Some("2026-09-01".into()),
        Some("2026-09-30".into()),
    )
    .await
    .unwrap();
    assert_eq!(profit.damage_loss_minor, 300);
    assert_eq!(profit.operational_profit_minor, -300);

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Owner capital and withdrawals move cash but never touch operational profit.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn owner_transfers_excluded_from_profit() {
    let dir = temp_dir("owner");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let account = funded_cash(&state, &owner, "OWN01").await;
    let before = cash_balance_sql(&state, account).await;

    let capital = application::expenses::owner_transaction_post(
        &state,
        &owner,
        OwnerTransactionInput {
            kind: "capital_in".into(),
            amount_minor: 200_000,
            transaction_date: "2026-09-02".into(),
            cash_account_id: account,
            notes: Some("initial capital".into()),
            idempotency_key: Some("own-in-1".into()),
        },
        "corr-o1",
    )
    .await
    .unwrap();
    assert_eq!(capital.transaction_number, "OWN-000001");

    application::expenses::owner_transaction_post(
        &state,
        &owner,
        OwnerTransactionInput {
            kind: "withdrawal".into(),
            amount_minor: 50_000,
            transaction_date: "2026-09-03".into(),
            cash_account_id: account,
            notes: None,
            idempotency_key: Some("own-out-1".into()),
        },
        "corr-o2",
    )
    .await
    .unwrap();

    assert_eq!(
        cash_balance_sql(&state, account).await,
        before + 200_000 - 50_000
    );
    assert_eq!(cash_entry_count(&state, "owner_capital").await, 1);
    assert_eq!(cash_entry_count(&state, "owner_withdrawal").await, 1);

    let profit = application::expenses::profit_summary(&state, &owner, None, None)
        .await
        .unwrap();
    assert_eq!(profit.owner_capital_in_minor, 200_000);
    assert_eq!(profit.owner_withdrawals_minor, 50_000);
    assert_eq!(profit.expenses_minor, 0);
    assert_eq!(profit.revenue_minor, 0);
    assert_eq!(profit.operational_profit_minor, 0);

    // Withdrawing more than the account has is rejected atomically.
    let err = application::expenses::owner_transaction_post(
        &state,
        &owner,
        OwnerTransactionInput {
            kind: "withdrawal".into(),
            amount_minor: 99_999_999,
            transaction_date: "2026-09-04".into(),
            cash_account_id: account,
            notes: None,
            idempotency_key: Some("own-over".into()),
        },
        "corr-o3",
    )
    .await
    .expect_err("overdraft must fail");
    assert!(matches!(err, AppError::Validation(_)));

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// RBAC: expenses are viewable/postable by manager and accountant; salesperson
// is excluded; owner.transfer requires the owner/manager roles.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn expense_and_profit_permissions_are_enforced() {
    let dir = temp_dir("rbac");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "owner").await;
    let accountant = make_role_principal(&state, "acc", "accountant").await;
    let salesperson = make_role_principal(&state, "sp", "salesperson").await;
    let account = funded_cash(&state, &owner, "RBAC01").await;

    // Salesperson cannot even list expenses.
    let err = application::expenses::expense_category_list(&state, &salesperson)
        .await
        .expect_err("salesperson must be denied");
    assert!(matches!(err, AppError::Unauthorized(_)));

    // Accountant can post an expense and read the profit report.
    let post = application::expenses::expense_post(
        &state,
        &accountant,
        ExpenseInput {
            category_id: 2,
            amount_minor: 15_000,
            expense_date: "2026-09-07".into(),
            cash_account_id: account,
            payment_method_id: 1,
            description: "Salaries advance".into(),
            payee: None,
            reference: None,
            attachment_path: None,
            idempotency_key: Some("exp-rbac".into()),
        },
        "corr-a1",
    )
    .await
    .unwrap();
    assert_eq!(post.status, "posted");
    application::expenses::profit_summary(&state, &accountant, None, None)
        .await
        .expect("accountant reads profit");

    // Salesperson cannot post an owner transfer (and accountant cannot either).
    let input = OwnerTransactionInput {
        kind: "capital_in".into(),
        amount_minor: 1000,
        transaction_date: "2026-09-07".into(),
        cash_account_id: account,
        notes: None,
        idempotency_key: None,
    };
    let err_sp = application::expenses::owner_transaction_post(
        &state,
        &salesperson,
        input.clone(),
        "corr-x1",
    )
    .await
    .expect_err("salesperson denied");
    assert!(matches!(err_sp, AppError::Unauthorized(_)));
    let err_acc =
        application::expenses::owner_transaction_post(&state, &accountant, input, "corr-x2")
            .await
            .expect_err("accountant denied");
    assert!(matches!(err_acc, AppError::Unauthorized(_)));

    // Owner is allowed.
    application::expenses::owner_transaction_post(
        &state,
        &owner,
        OwnerTransactionInput {
            kind: "capital_in".into(),
            amount_minor: 1000,
            transaction_date: "2026-09-07".into(),
            cash_account_id: account,
            notes: None,
            idempotency_key: Some("own-rbac".into()),
        },
        "corr-o",
    )
    .await
    .expect("owner allowed");

    // Accountant can create a new category and see it.
    let cat = application::expenses::expense_category_create(
        &state,
        &accountant,
        ExpenseCategoryInput {
            code: "TEA".into(),
            name: "Tea / refreshments".into(),
            is_active: Some(true),
        },
        "corr-cat",
    )
    .await
    .unwrap();
    assert_eq!(cat.code, "TEA");

    state.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn electricity_expense_end_to_end_filters_report_reversal_and_restart() {
    let dir = temp_dir("electricity-e2e");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "electricity-owner").await;
    let salesperson = make_role_principal(&state, "electricity-sales", "salesperson").await;
    let account = funded_cash(&state, &owner, "ELECTRICITY01").await;
    let opening_balance = cash_balance_sql(&state, account).await;

    let created_category = application::expenses::expense_category_create(
        &state,
        &owner,
        ExpenseCategoryInput {
            code: "TEST_OFFICE_SUPPLIES".into(),
            name: "Test Office Supplies".into(),
            is_active: Some(true),
        },
        "corr-category-create",
    )
    .await
    .unwrap();
    let duplicate = application::expenses::expense_category_create(
        &state,
        &owner,
        ExpenseCategoryInput {
            code: "TEST_OFFICE_SUPPLIES_2".into(),
            name: "test office supplies".into(),
            is_active: Some(true),
        },
        "corr-category-duplicate",
    )
    .await
    .expect_err("category names must be unique without regard to case");
    assert!(matches!(duplicate, AppError::Conflict(_)));

    let electricity_id: i64 =
        sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'electricity'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    let duplicate_rename = application::expenses::expense_category_update(
        &state,
        &owner,
        ExpenseCategoryUpdateInput {
            id: created_category.id,
            name: "ELECTRICITY".into(),
            is_active: true,
        },
        "corr-category-duplicate-rename",
    )
    .await
    .expect_err("renaming to an existing category name must fail");
    assert!(matches!(duplicate_rename, AppError::Conflict(_)));
    let input = ExpenseInput {
        category_id: electricity_id,
        amount_minor: 150_000,
        expense_date: "2026-09-11".into(),
        cash_account_id: account,
        payment_method_id: 1,
        description: "September meter reading".into(),
        payee: None,
        reference: Some("TEST-ELEC-1500".into()),
        attachment_path: Some("isolated-test-bill.pdf".into()),
        idempotency_key: Some("test-electricity-1500".into()),
    };
    let expense =
        application::expenses::expense_post(&state, &owner, input.clone(), "corr-electricity-post")
            .await
            .unwrap();
    let replay =
        application::expenses::expense_post(&state, &owner, input, "corr-electricity-replay")
            .await
            .unwrap();
    assert_eq!(replay.id, expense.id);
    assert_eq!(
        cash_balance_sql(&state, account).await,
        opening_balance - 150_000
    );
    let matching_outflows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cash_entries
          WHERE entry_type = 'expense' AND reference_type = 'expense'
            AND reference_id = ? AND amount_minor = -150000",
    )
    .bind(expense.id)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(matching_outflows, 1);

    let filtered = application::expenses::expense_page(
        &state,
        &owner,
        ExpensePageInput {
            status: Some("posted".into()),
            category_id: Some(electricity_id),
            cash_account_id: Some(account),
            from_date: Some("2026-09-11".into()),
            to_date: Some("2026-09-11".into()),
            search: Some("elec-1500".into()),
            sort_by: Some("amount".into()),
            sort_direction: Some("desc".into()),
            limit: Some(20),
            offset: Some(0),
        },
    )
    .await
    .unwrap();
    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.total_amount_minor, 150_000);
    assert_eq!(
        filtered.items[0].payment_method_name.as_deref(),
        Some("Cash")
    );
    let note_search = application::expenses::expense_page(
        &state,
        &owner,
        ExpensePageInput {
            status: Some("posted".into()),
            category_id: None,
            cash_account_id: None,
            from_date: None,
            to_date: None,
            search: Some("meter reading".into()),
            sort_by: Some("date".into()),
            sort_direction: Some("asc".into()),
            limit: Some(20),
            offset: Some(0),
        },
    )
    .await
    .unwrap();
    assert_eq!(note_search.total, 1);
    let second_page = application::expenses::expense_page(
        &state,
        &owner,
        ExpensePageInput {
            status: Some("posted".into()),
            category_id: Some(electricity_id),
            cash_account_id: None,
            from_date: None,
            to_date: None,
            search: None,
            sort_by: Some("date".into()),
            sort_direction: Some("desc".into()),
            limit: Some(1),
            offset: Some(1),
        },
    )
    .await
    .unwrap();
    assert_eq!(second_page.total, 1);
    assert!(second_page.items.is_empty());

    let report = application::reports::export_report(
        &state,
        "expense_report",
        &ReportFilterInput {
            from_date: Some("2026-09-11".into()),
            to_date: Some("2026-09-11".into()),
            cash_account_id: Some(account),
            category_id: Some(electricity_id),
            status: None,
        },
        ExportFormat::Csv,
    )
    .await
    .unwrap();
    assert_eq!(report.row_count, 1);
    let csv = fs::read_to_string(&report.report_path).unwrap();
    assert_eq!(csv.matches("TEST-ELEC-1500").count(), 1);
    let pdf_report = application::reports::export_report(
        &state,
        "expense_report",
        &ReportFilterInput {
            from_date: Some("2026-09-11".into()),
            to_date: Some("2026-09-11".into()),
            cash_account_id: Some(account),
            category_id: Some(electricity_id),
            status: None,
        },
        ExportFormat::Pdf,
    )
    .await
    .unwrap();
    assert_eq!(pdf_report.row_count, 1);
    assert!(fs::metadata(pdf_report.report_path).unwrap().len() > 100);

    let renamed = application::expenses::expense_category_update(
        &state,
        &owner,
        ExpenseCategoryUpdateInput {
            id: electricity_id,
            name: "Electricity & Power".into(),
            is_active: false,
        },
        "corr-electricity-archive",
    )
    .await
    .unwrap();
    assert!(!renamed.is_active);
    let history = application::expenses::expense_page(
        &state,
        &owner,
        ExpensePageInput {
            status: None,
            category_id: Some(electricity_id),
            cash_account_id: None,
            from_date: None,
            to_date: None,
            search: Some("Electricity & Power".into()),
            sort_by: None,
            sort_direction: None,
            limit: Some(20),
            offset: Some(0),
        },
    )
    .await
    .unwrap();
    assert_eq!(history.total, 1);
    assert_eq!(history.items[0].category_name, "Electricity & Power");

    let archived_post = application::expenses::expense_post(
        &state,
        &owner,
        ExpenseInput {
            category_id: electricity_id,
            amount_minor: 100,
            expense_date: "2026-09-11".into(),
            cash_account_id: account,
            payment_method_id: 1,
            description: String::new(),
            payee: None,
            reference: None,
            attachment_path: None,
            idempotency_key: Some("archived-category-rejected".into()),
        },
        "corr-archived-rejected",
    )
    .await
    .expect_err("archived categories cannot be used for new expenses");
    assert!(matches!(archived_post, AppError::Validation(_)));

    let denied = application::expenses::expense_page(
        &state,
        &salesperson,
        ExpensePageInput {
            status: None,
            category_id: None,
            cash_account_id: None,
            from_date: None,
            to_date: None,
            search: None,
            sort_by: None,
            sort_direction: None,
            limit: None,
            offset: None,
        },
    )
    .await
    .expect_err("salesperson cannot read expenses");
    assert!(matches!(denied, AppError::Unauthorized(_)));
    let denied_reverse = application::expenses::expense_reverse(
        &state,
        &salesperson,
        ExpenseReverseInput {
            expense_id: expense.id,
            reason: "Must be denied".into(),
        },
        "corr-denied-reverse",
    )
    .await
    .expect_err("salesperson cannot reverse expenses");
    assert!(matches!(denied_reverse, AppError::Unauthorized(_)));

    application::expenses::expense_reverse(
        &state,
        &owner,
        ExpenseReverseInput {
            expense_id: expense.id,
            reason: "Isolated reversal verification".into(),
        },
        "corr-electricity-reverse",
    )
    .await
    .unwrap();
    assert_eq!(cash_balance_sql(&state, account).await, opening_balance);
    let matching_reversals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cash_entries
          WHERE entry_type = 'expense_reversal' AND reference_id = ? AND amount_minor = 150000",
    )
    .bind(expense.id)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(matching_reversals, 1);

    let reversed_report = application::reports::export_report(
        &state,
        "expense_report",
        &ReportFilterInput {
            from_date: Some("2026-09-11".into()),
            to_date: Some("2026-09-11".into()),
            cash_account_id: Some(account),
            category_id: Some(electricity_id),
            status: None,
        },
        ExportFormat::Csv,
    )
    .await
    .unwrap();
    assert_eq!(reversed_report.row_count, 0);
    let profit = application::expenses::profit_summary(
        &state,
        &owner,
        Some("2026-09-11".into()),
        Some("2026-09-11".into()),
    )
    .await
    .unwrap();
    assert_eq!(profit.expenses_minor, 0);

    state.pool.close().await;
    let reopened = open_state(&dir).await;
    let persisted_status: String = sqlx::query_scalar("SELECT status FROM expenses WHERE id = ?")
        .bind(expense.id)
        .fetch_one(&reopened.pool)
        .await
        .unwrap();
    let persisted_category: String =
        sqlx::query_scalar("SELECT name FROM expense_categories WHERE id = ?")
            .bind(electricity_id)
            .fetch_one(&reopened.pool)
            .await
            .unwrap();
    let created_category_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM expense_categories WHERE id = ?")
            .bind(created_category.id)
            .fetch_one(&reopened.pool)
            .await
            .unwrap();
    assert_eq!(persisted_status, "reversed");
    assert_eq!(persisted_category, "Electricity & Power");
    assert_eq!(created_category_count, 1);

    reopened.pool.close().await;
    let _ = fs::remove_dir_all(&dir);
}
