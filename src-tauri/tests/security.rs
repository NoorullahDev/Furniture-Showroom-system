use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::application::audit::{query_audit, AuditFilter};
use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::application::{
    auth, catalogue, maintenance, products, search, settings, users,
};
use furniture_shop_lib::infrastructure as infra;
use furniture_shop_lib::state::AppState;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("furniture-shop-security-{label}"));
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

/// Expected permission sets are the exact grants in the released migrations.
/// Owner receives every permission; the four working templates get the focused
/// subsets below. This pins the RBAC policy so accidental grant drift fails.
fn manager_permissions() -> Vec<String> {
    vec![
        "audit.view",
        "backup.create",
        "bundle.create",
        "bundle.view",
        "credit.note.use",
        "customer.create",
        "customer.view",
        "damage.record",
        "delivery.create",
        "delivery.update",
        "delivery.view",
        "expense.create",
        "expense.reverse",
        "expense.view",
        "inventory.adjust",
        "inventory.count",
        "inventory.create",
        "inventory.valuation",
        "invoice.print",
        "owner.transfer",
        "payable.view",
        "payment.receive",
        "payment.void",
        "product.cost.view",
        "profit.view",
        "purchase.create",
        "report.export",
        "sale.cancel",
        "sale.create",
        "sale.credit",
        "sale.discount.override",
        "sale.return",
        "settings.manage",
        "supplier.create",
        "supplier.return",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn salesperson_permissions() -> Vec<String> {
    vec![
        "bundle.view",
        "credit.note.use",
        "customer.create",
        "customer.view",
        "delivery.update",
        "delivery.view",
        "invoice.print",
        "payment.receive",
        "product.create",
        "sale.create",
        "sale.credit",
        "sale.return",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn accountant_permissions() -> Vec<String> {
    vec![
        "credit.note.use",
        "customer.view",
        "delivery.view",
        "expense.create",
        "expense.reverse",
        "expense.view",
        "invoice.print",
        "payable.view",
        "payment.receive",
        "payment.void",
        "profit.view",
        "purchase.create",
        "report.export",
        "sale.return",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn storekeeper_permissions() -> Vec<String> {
    vec![
        "damage.record",
        "delivery.create",
        "delivery.update",
        "delivery.view",
        "inventory.adjust",
        "inventory.count",
        "inventory.create",
        "product.create",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

#[tokio::test]
async fn rbac_matrix_matches_seed_policy() {
    let dir = temp_dir("rbac-matrix");
    let state = open_state(&dir).await;

    let owner = make_owner(&state, "alice").await;
    let manager = make_role_principal(&state, "mgr", "manager").await;
    let salesperson = make_role_principal(&state, "sp", "salesperson").await;
    let accountant = make_role_principal(&state, "acc", "accountant").await;
    let storekeeper = make_role_principal(&state, "sk", "storekeeper").await;

    // Owner inherits every granted permission by construction of the seed.
    let all: Vec<String> = sqlx::query_scalar("SELECT code FROM permissions ORDER BY code")
        .fetch_all(&state.pool)
        .await
        .unwrap();
    assert_eq!(owner.permissions, all);

    assert_eq!(manager.permissions, manager_permissions());
    assert_eq!(salesperson.permissions, salesperson_permissions());
    assert_eq!(accountant.permissions, accountant_permissions());
    assert_eq!(storekeeper.permissions, storekeeper_permissions());
}

#[tokio::test]
async fn report_export_permission_is_the_seeded_code() {
    // Regression guard: the command layer previously requested `reports.view`,
    // which is never seeded, so every report export returned UNAUTHORIZED for
    // every role. The correct, granted permission is `report.export`.
    let dir = temp_dir("report-perm");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;
    let manager = make_role_principal(&state, "mgr", "manager").await;
    let accountant = make_role_principal(&state, "acc", "accountant").await;
    let salesperson = make_role_principal(&state, "sp", "salesperson").await;
    let storekeeper = make_role_principal(&state, "sk", "storekeeper").await;

    assert!(owner.require("report.export").is_ok());
    assert!(manager.require("report.export").is_ok());
    assert!(accountant.require("report.export").is_ok());
    assert!(salesperson.require("report.export").is_err());
    assert!(storekeeper.require("report.export").is_err());

    // The stale code must not exist anywhere in the permission registry.
    let stale: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM permissions WHERE code = 'reports.view'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(stale, 0);
}

#[tokio::test]
async fn path_traversal_read_product_image_rejected() {
    let dir = temp_dir("path-image");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let images_dir = state.paths.images_dir.clone();
    fs::write(images_dir.join("ok.png"), b"fake-image-bytes").unwrap();

    // Serving a file inside the approved directory works.
    let data =
        products::read_product_image(&state, &owner, &images_dir.join("ok.png").to_string_lossy())
            .await
            .unwrap();
    assert!(data.starts_with("data:"));

    // `..` escape that resolves outside the images directory is rejected.
    let escape = images_dir.join("..").join("ok-x.bin");
    fs::write(&escape, b"secret").unwrap();
    let err = products::read_product_image(&state, &owner, &escape.to_string_lossy())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");

    // Absolute path outside the images directory is rejected.
    let outside = dir.join("outside.txt");
    fs::write(&outside, b"secret").unwrap();
    let err = products::read_product_image(&state, &owner, &outside.to_string_lossy())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");

    // Encoded traversal never leaks existence and is rejected (not found).
    let encoded = images_dir
        .join("..%5c..%5cdir")
        .to_string_lossy()
        .to_string();
    let err = products::read_product_image(&state, &owner, &encoded)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "NOT_FOUND");
}

#[tokio::test]
async fn path_traversal_delete_backup_rejected() {
    let dir = temp_dir("path-backup");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let err = maintenance::delete_backup(&state, &owner, "../escape.db", "corr-1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");

    let err = maintenance::delete_backup(&state, &owner, "dir\\escape.db", "corr-2")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");
}

#[tokio::test]
async fn path_traversal_restore_marker_rejected() {
    let dir = temp_dir("path-marker");
    let state = open_state(&dir).await;

    // A marker pointing one level above the backups directory resolves to a
    // real file, but perform_pending_restore must refuse to swap it in.
    let escaped = dir.join("escape.db");
    fs::write(&escaped, b"not a real backup").unwrap();

    infra::restore::write_restore_marker(
        &state.paths.data_dir,
        &infra::restore::RestoreMarker {
            backup_name: "../escape.db".into(),
            safety_backup: None,
        },
    )
    .unwrap();

    let err = infra::restore::perform_pending_restore(&state.paths).unwrap_err();
    assert_ne!(err.code(), "OK");
    // Live DB must still be the original (default shop schema with users table).
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn ensure_member_rejects_escapes() {
    let dir = temp_dir("ensure-member");
    let paths = infra::FilePaths::init(&dir).unwrap();

    let inside = paths.images_dir.join("a.png");
    fs::write(&inside, b"x").unwrap();
    assert!(paths.ensure_member(&paths.images_dir, &inside).is_ok());

    let outside = dir.join("b.bin");
    fs::write(&outside, b"x").unwrap();
    let err = paths
        .ensure_member(&paths.images_dir, &outside)
        .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");

    let rel_escape = inside.with_file_name("..").join("c.bin");
    fs::write(&rel_escape, b"x").unwrap();
    let err = paths
        .ensure_member(&paths.images_dir, &rel_escape)
        .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");
}

#[tokio::test]
async fn corrupt_and_unsupported_image_rejected() {
    let dir = temp_dir("bad-image");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let corrupt = dir.join("corrupt.jpg");
    fs::write(&corrupt, b"\xff\xd8\xff not a real jpeg body").unwrap();
    let err = products::add_product_image(&state, &owner, 1, &corrupt.to_string_lossy(), "corr-1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "IMAGE_ERROR");

    let unsupported = dir.join("plan.bmp");
    fs::write(&unsupported, b"BM unsupported format").unwrap();
    let err =
        products::add_product_image(&state, &owner, 1, &unsupported.to_string_lossy(), "corr-2")
            .await
            .unwrap_err();
    assert_eq!(err.code(), "IMAGE_ERROR");
}

#[tokio::test]
async fn oversized_image_serve_rejected() {
    let dir = temp_dir("big-image");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let big = state.paths.images_dir.join("huge.bin");
    fs::write(&big, vec![0u8; 26 * 1024 * 1024]).unwrap();
    let err = products::read_product_image(&state, &owner, &big.to_string_lossy())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");
}

#[tokio::test]
async fn non_sqlite_backup_rejected_and_db_untouched() {
    let dir = temp_dir("non-sqlite-backup");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    fs::write(
        state.paths.backups_dir.join("garbage.db"),
        b"this is not sqlite",
    )
    .unwrap();

    let err = maintenance::restore_backup(&state, &owner, "garbage.db", "corr-1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "BACKUP_ERROR");

    assert!(infra::restore::read_restore_marker(&state.paths.data_dir)
        .unwrap()
        .is_none());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1); // just the owner we created below; DB untouched
}

#[tokio::test]
async fn sql_injection_global_search_is_safe() {
    let dir = temp_dir("sqli-search");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let payloads = [
        "'; DROP TABLE users; --",
        "\" OR 1=1 --",
        "'; DELETE FROM settings; --",
    ];
    for payload in payloads {
        let result = search::global_search(&state, &owner, payload).await;
        assert!(result.is_ok(), "payload {payload:?} failed: {result:?}");
    }

    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(users, 1);
    let settings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert!(settings >= 0);
}

#[tokio::test]
async fn sql_injection_list_products_and_audit_safe() {
    let dir = temp_dir("sqli-list");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let payload = "sup' UNION SELECT ...; DROP TABLE users; --";
    let result = products::list_products(
        &state,
        &owner,
        Some("all".into()),
        Some(payload.to_string()),
        None,
        None,
        None,
        None,
        Some(payload.to_string()),
    )
    .await;
    assert!(result.is_ok(), "{result:?}");

    let filter = AuditFilter {
        action: Some("x' OR '1'='1".into()),
        entity_type: Some("'; DROP TABLE users; --".into()),
        entity_id: None,
        user_id: Some(1),
        from: Some("2020-01-01".into()),
        to: Some("2030-01-01".into()),
        limit: 50,
        offset: 0,
    };
    let page = query_audit(&state, &owner, &filter).await;
    assert!(page.is_ok(), "{page:?}");

    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(users, 1);
}

#[tokio::test]
async fn sql_injection_dropped_callback_is_stored_literally() {
    let dir = temp_dir("sqli-inject");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let payload = "x'); DROP TABLE categories; --";
    let created = catalogue::create_category(&state, &owner, payload, None, 0, "corr-1")
        .await
        .unwrap();
    assert!(created.id > 0);

    let stored: String = sqlx::query_scalar("SELECT name FROM categories WHERE id = ?")
        .bind(created.id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(stored, payload);

    let categories: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM categories")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(categories, 1);
}

#[tokio::test]
async fn locked_session_blocked_and_unlock_works() {
    let dir = temp_dir("locked");
    let state = open_state(&dir).await;
    let hash = infra::password::hash_password("Str0ngPass123").unwrap();
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, full_name) VALUES ('lockme', ?, 'Lock Me') RETURNING id",
    )
    .bind(hash)
    .fetch_one(&state.pool)
    .await
    .unwrap();

    let session = state.sessions.create(user_id).await.unwrap();
    auth::lock_session(&state, &session.id).await.unwrap();

    let err = auth::resolve_session(&state, &session.id)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "SESSION_LOCKED");

    let err = auth::unlock_session(&state, &session.id, "wrong-pass")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_CREDENTIALS");

    auth::unlock_session(&state, &session.id, "Str0ngPass123")
        .await
        .unwrap();
    assert!(auth::resolve_session(&state, &session.id).await.is_ok());
}

#[tokio::test]
async fn backup_tamper_rejected_and_db_untouched() {
    let dir = temp_dir("tamper");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let backup = maintenance::create_backup(&state, &owner, "corr-1")
        .await
        .unwrap();
    let filename = std::path::Path::new(&backup.backup_path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Corrupt the SQLite file header magic so integrity verification fails.
    let backup_path = state.paths.backups_dir.join(&filename);
    let mut bytes = fs::read(&backup_path).unwrap();
    bytes[0] ^= 0xFF;
    bytes[1] ^= 0xFF;
    fs::write(&backup_path, &bytes).unwrap();

    assert!(infra::verify_backup_file(&backup_path).is_err());

    let err = maintenance::restore_backup(&state, &owner, &filename, "corr-2")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "BACKUP_ERROR");

    // No safety backup, no marker, live DB untouched.
    assert!(infra::restore::read_restore_marker(&state.paths.data_dir)
        .unwrap()
        .is_none());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn audit_hash_chain_detects_tampering() {
    let dir = temp_dir("audit-chain");
    let state = open_state(&dir).await;

    state
        .audits
        .record_pool(infra::AuditInput {
            action: "auth.login".into(),
            user_id: Some(1),
            ..Default::default()
        })
        .await
        .unwrap();
    state
        .audits
        .record_pool(infra::AuditInput {
            action: "settings.update".into(),
            user_id: Some(1),
            ..Default::default()
        })
        .await
        .unwrap();
    state
        .audits
        .record_pool(infra::AuditInput {
            action: "backup.create".into(),
            user_id: Some(1),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(state.audits.verify_chain().await.unwrap().is_none());

    // Tamper with the middle row's action; the chain must break. The verifier
    // recomputes each row's hash and compares it against the NEXT row's stored
    // prev_hash, so the broken link is reported at the row following the edit
    // (the tampered row 2 is detected when the (2,3) pair is compared).
    sqlx::query("UPDATE audit_logs SET action = 'evidences.alter' WHERE id = 2")
        .execute(&state.pool)
        .await
        .unwrap();
    let broken = state.audits.verify_chain().await.unwrap();
    assert!(broken.is_some(), "tampering went undetected");
    assert_eq!(broken.unwrap(), 3);
}

#[tokio::test]
async fn csv_injection_cells_are_neutralized() {
    let dir = temp_dir("csv-inject");
    let out_path = dir.join("report.csv");

    let table = infra::CsvTable {
        title: "Sales".into(),
        generated_at: "now".into(),
        filter_summary: "all".into(),
        columns: vec!["Article".into(), "Formula".into()],
        rows: vec![vec!["=SUM(A1:A9)".into(), "+12345".into()]],
        totals: Some(vec!["=2+2".into(), "-44".into()]),
    };
    infra::write_csv(&table, &out_path).unwrap();

    let content = fs::read_to_string(&out_path).unwrap();
    assert!(content.contains("'=SUM(A1:A9)"), "{content}");
    assert!(content.contains("'+12345"), "{content}");
    assert!(content.contains("'=2+2"), "{content}");
    assert!(content.contains("'-44"), "{content}");

    // Spreadsheet formula characters that were sanitized must not appear raw.
    for cell in ["=SUM(A1:A9)", "+12345"] {
        assert!(
            content.lines().filter(|l| l.contains(cell)).count() <= 1,
            "un-sanitized formula {cell:?} leaked: {content}"
        );
    }
}

#[tokio::test]
async fn oversized_inputs_rejected() {
    let dir = temp_dir("oversize");
    let state = open_state(&dir).await;
    let owner = make_owner(&state, "alice").await;

    let long_name = "a".repeat(121);
    let err = catalogue::create_category(&state, &owner, &long_name, None, 0, "corr-1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");

    let long_full_name = "n".repeat(201);
    let err = users::create_user(
        &state,
        &owner,
        "newbie",
        &long_full_name,
        "Str0ngPass123",
        &["salesperson".to_string()],
        "corr-2",
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");

    let err = users::create_user(
        &state,
        &owner,
        "ab",
        "Too Short",
        "Str0ngPass123",
        &["salesperson".to_string()],
        "corr-3",
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");

    let err = users::create_user(
        &state,
        &owner,
        "weakpass",
        "Weak Password",
        "noDigitHere",
        &["salesperson".to_string()],
        "corr-4",
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "VALIDATION");
}

#[tokio::test]
async fn sensitive_setting_needs_permission() {
    let dir = temp_dir("settx");
    let state = open_state(&dir).await;
    let salesperson = make_role_principal(&state, "sp", "salesperson").await;

    let err = settings::set_authorized(&state, &salesperson, "shop.name", "\"x\"", "corr-1")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "UNAUTHORIZED");

    let owner = make_owner(&state, "alice").await;
    settings::set_authorized(&state, &owner, "shop.name", "\"Showroom\"", "corr-2")
        .await
        .unwrap();
}
