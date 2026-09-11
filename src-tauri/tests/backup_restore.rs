use furniture_shop_lib::application::auth::Principal;
use furniture_shop_lib::infrastructure::{
    backup_package, backup_preferences, db, restore, FilePaths,
};
use furniture_shop_lib::state::AppState;

fn temp_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("furniture-{label}-{}", uuid::Uuid::new_v4()))
}

#[tokio::test]
async fn complete_package_round_trip_restores_records_assets_and_invalidates_sessions() {
    let root = temp_root("complete-backup");
    let data = root.join("app-data");
    let selected = root.join("owner-selected-backups");
    std::fs::create_dir_all(&selected).unwrap();
    let paths = FilePaths::init(&data).unwrap();
    let (pool, _) = db::open(&paths).await.unwrap();

    sqlx::query("INSERT INTO users (id, username, password_hash, full_name) VALUES (9001, 'restore-owner', 'hash', 'Restore Owner')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO sessions (id, user_id) VALUES ('must-be-revoked', 9001)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO settings (key, value_json, updated_by) VALUES ('test.restore.setting', '\"snapshot-value\"', 9001)")
        .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO locations (id, name, type) VALUES (9001, 'Restore Showroom', 'showroom')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO categories (id, name) VALUES (9001, 'Restore Furniture')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO products (id, article_number, article_number_norm, name, category_id, cost_minor, sale_price_minor) VALUES (9001, 'REST-001', 'rest-001', 'Restore Sofa', 9001, 50000, 75000)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO product_images (product_id, relative_path, thumbnail_path, is_primary, sha256, mime_type) VALUES (9001, 'restore.webp', 'restore-thumb.webp', 1, 'image-hash', 'image/webp')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO suppliers (id, code, name, opening_balance_minor, created_by) VALUES (9001, 'REST-SUP', 'Restore Supplier', 12000, 9001)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO supplier_ledger_entries (supplier_id, entry_type, amount_minor, balance_after_minor, created_by) VALUES (9001, 'opening_balance', 12000, 12000, 9001)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO supplier_payments (payment_number, supplier_id, payment_method_id, cash_account_id, payment_date, amount_minor, created_by) VALUES ('REST-SP', 9001, 1, 1, '2026-09-11', 2000, 9001)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customers (id, code, name, opening_balance_minor, created_by) VALUES (9001, 'REST-CUS', 'Restore Customer', 18000, 9001)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customer_ledger_entries (customer_id, entry_type, amount_minor, balance_after_minor, created_by) VALUES (9001, 'opening_balance', 18000, 18000, 9001)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO sales (id, sale_number, customer_id, customer_name, location_id, sale_date, status, total_minor, due_minor, created_by) VALUES (9001, 'REST-SALE', 9001, 'Restore Customer', 9001, '2026-09-11', 'confirmed', 30000, 20000, 9001)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO sale_items (sale_id, product_id, article_number, product_name, quantity, unit_price_minor, line_total_minor) VALUES (9001, 9001, 'REST-001', 'Restore Sofa', 1, 30000, 30000)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO customer_payments (receipt_number, customer_id, sale_id, payment_method_id, cash_account_id, payment_date, amount_minor, created_by) VALUES ('REST-CP', 9001, 9001, 1, 1, '2026-09-11', 10000, 9001)")
        .execute(&pool).await.unwrap();

    let external_bill = root.join("electricity-bill.pdf");
    std::fs::write(&external_bill, b"external bill").unwrap();
    sqlx::query("INSERT INTO expense_categories (id, code, name, created_by) VALUES (9001, 'restore-electricity', 'Electricity', 9001)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO expenses (id, expense_number, category_id, amount_minor, expense_date, cash_account_id, payment_method_id, description, attachment_path, status, created_by, posted_by) VALUES (9001, 'REST-EXP', 9001, 150000, '2026-09-11', 1, 1, 'Electricity bill', ?, 'posted', 9001, 9001)")
        .bind(external_bill.to_string_lossy().into_owned()).execute(&pool).await.unwrap();

    std::fs::write(paths.images_dir.join("restore.webp"), b"full-image").unwrap();
    std::fs::write(paths.images_dir.join("restore-thumb.webp"), b"thumbnail").unwrap();
    std::fs::write(paths.branding_dir.join("shop-logo.webp"), b"logo").unwrap();
    std::fs::create_dir_all(data.join("attachments")).unwrap();
    std::fs::write(data.join("attachments/bill.pdf"), b"bill").unwrap();
    std::fs::create_dir_all(data.join("templates")).unwrap();
    std::fs::write(data.join("templates/invoice.json"), b"template").unwrap();
    std::fs::write(paths.backups_dir.join("old-backup.db"), b"must-not-recurse").unwrap();

    backup_preferences::save(
        &data,
        &backup_preferences::BackupPreferences {
            directory: Some(selected.to_string_lossy().into_owned()),
            auto_backup_on_close: true,
        },
    )
    .unwrap();

    let schema = db::MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap();
    let package =
        backup_package::create_package(&paths, &selected, "Isolated-Test", "manual", schema)
            .unwrap();
    assert!(package.name.ends_with(".furniture-backup"));
    let inspected = backup_package::inspect_backup(&package.path, schema, &data).unwrap();
    assert!(!inspected.legacy_database_only);
    assert_eq!(inspected.file_count, 7); // DB + assets; old backups are not recursively included.
    let listed = backup_package::list_backup_files(&selected, schema, &data).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, package.name);

    sqlx::query("UPDATE products SET name = 'Mutated Sofa' WHERE id = 9001")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE settings SET value_json = '\"mutated\"' WHERE key = 'test.restore.setting'",
    )
    .execute(&pool)
    .await
    .unwrap();
    std::fs::write(paths.images_dir.join("restore.webp"), b"mutated-image").unwrap();

    let state = AppState::new(pool, paths.clone());
    let restore_owner = Principal {
        session_id: "restore-test-session".into(),
        user_id: 9001,
        username: "restore-owner".into(),
        full_name: "Restore Owner".into(),
        roles: vec!["owner".into()],
        permissions: vec!["backup.create".into(), "backup.restore".into()],
    };
    let scheduled = furniture_shop_lib::application::backup_workflow::schedule_restore(
        &state,
        &restore_owner,
        &package.path.to_string_lossy(),
        "restore-round-trip",
    )
    .await
    .unwrap();
    assert!(scheduled.restart_required);
    assert!(selected
        .join(scheduled.safety_backup_name.unwrap())
        .exists());
    assert!(restore::read_restore_marker(&data).unwrap().is_some());
    state.pool.close().await;
    drop(state);

    restore::perform_pending_restore(&paths).unwrap();
    let (restored_pool, _) = db::open(&paths).await.unwrap();
    let product: String = sqlx::query_scalar("SELECT name FROM products WHERE id = 9001")
        .fetch_one(&restored_pool)
        .await
        .unwrap();
    let setting: String =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'test.restore.setting'")
            .fetch_one(&restored_pool)
            .await
            .unwrap();
    let supplier_balance: i64 = sqlx::query_scalar(
        "SELECT balance_after_minor FROM supplier_ledger_entries WHERE supplier_id = 9001",
    )
    .fetch_one(&restored_pool)
    .await
    .unwrap();
    let customer_balance: i64 = sqlx::query_scalar(
        "SELECT balance_after_minor FROM customer_ledger_entries WHERE customer_id = 9001",
    )
    .fetch_one(&restored_pool)
    .await
    .unwrap();
    let sale_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sales WHERE id = 9001")
        .fetch_one(&restored_pool)
        .await
        .unwrap();
    let supplier_payment_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM supplier_payments WHERE supplier_id = 9001")
            .fetch_one(&restored_pool)
            .await
            .unwrap();
    let customer_payment_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM customer_payments WHERE customer_id = 9001")
            .fetch_one(&restored_pool)
            .await
            .unwrap();
    let active_sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE active = 1")
        .fetch_one(&restored_pool)
        .await
        .unwrap();
    let restored_attachment: String =
        sqlx::query_scalar("SELECT attachment_path FROM expenses WHERE id = 9001")
            .fetch_one(&restored_pool)
            .await
            .unwrap();
    let restored_backup_location: String =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'backup.location'")
            .fetch_one(&restored_pool)
            .await
            .unwrap();
    assert_eq!(product, "Restore Sofa");
    assert_eq!(setting, "\"snapshot-value\"");
    assert_eq!(supplier_balance, 12000);
    assert_eq!(customer_balance, 18000);
    assert_eq!(sale_count, 1);
    assert_eq!(supplier_payment_count, 1);
    assert_eq!(customer_payment_count, 1);
    assert_eq!(active_sessions, 0);
    assert_eq!(
        serde_json::from_str::<Option<String>>(&restored_backup_location)
            .unwrap()
            .unwrap(),
        std::fs::canonicalize(&selected).unwrap().to_string_lossy()
    );
    assert_eq!(
        std::fs::read(&restored_attachment).unwrap(),
        b"external bill"
    );
    assert_eq!(
        std::fs::read(paths.images_dir.join("restore.webp")).unwrap(),
        b"full-image"
    );
    assert_eq!(
        std::fs::read(paths.branding_dir.join("shop-logo.webp")).unwrap(),
        b"logo"
    );
    assert_eq!(
        std::fs::read(data.join("attachments/bill.pdf")).unwrap(),
        b"bill"
    );
    assert_eq!(
        std::fs::read(data.join("templates/invoice.json")).unwrap(),
        b"template"
    );
    assert_eq!(
        backup_preferences::load(&data).unwrap().directory.unwrap(),
        std::fs::canonicalize(&selected).unwrap().to_string_lossy()
    );

    restored_pool.close().await;
    drop(restored_pool);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn corrupt_package_and_missing_destination_are_rejected_without_touching_live_data() {
    let root = temp_root("corrupt-backup");
    let data = root.join("app-data");
    let selected = root.join("selected");
    std::fs::create_dir_all(&selected).unwrap();
    let paths = FilePaths::init(&data).unwrap();
    let (pool, _) = db::open(&paths).await.unwrap();
    sqlx::query("INSERT INTO settings (key, value_json) VALUES ('corrupt.probe', '1')")
        .execute(&pool)
        .await
        .unwrap();
    let schema = db::MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap();
    let package =
        backup_package::create_package(&paths, &selected, "Corrupt-Test", "manual", schema)
            .unwrap();
    let mut bytes = std::fs::read(&package.path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    let corrupt = selected.join("corrupt.furniture-backup");
    std::fs::write(&corrupt, bytes).unwrap();
    assert!(backup_package::inspect_backup(&corrupt, schema, &data).is_err());
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = 'corrupt.probe'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);

    let unavailable = root.join("missing-folder");
    assert!(backup_preferences::validate_directory(&unavailable).is_err());
    pool.close().await;
    drop(pool);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn automatic_backup_obeys_on_off_setting_and_writes_to_selected_folder_once() {
    let root = temp_root("automatic-backup");
    let data = root.join("app-data");
    let selected = root.join("selected");
    std::fs::create_dir_all(&selected).unwrap();
    let paths = FilePaths::init(&data).unwrap();
    let (pool, _) = db::open(&paths).await.unwrap();
    let state = AppState::new(pool, paths);

    backup_preferences::save(
        &data,
        &backup_preferences::BackupPreferences {
            directory: Some(selected.to_string_lossy().into_owned()),
            auto_backup_on_close: false,
        },
    )
    .unwrap();
    assert!(
        furniture_shop_lib::application::backup_workflow::create_automatic(&state)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(&selected).unwrap().count(), 0);

    backup_preferences::save(
        &data,
        &backup_preferences::BackupPreferences {
            directory: Some(selected.to_string_lossy().into_owned()),
            auto_backup_on_close: true,
        },
    )
    .unwrap();
    let result = furniture_shop_lib::application::backup_workflow::create_automatic(&state)
        .await
        .unwrap();
    assert_eq!(result.kind, "automatic");
    assert!(std::path::Path::new(&result.backup_path)
        .starts_with(std::fs::canonicalize(&selected).unwrap()));
    assert_eq!(std::fs::read_dir(&selected).unwrap().count(), 1);

    state.pool.close().await;
    drop(state);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn package_create_and_restore_require_the_existing_permissions() {
    let root = temp_root("backup-permissions");
    let paths = FilePaths::init(&root).unwrap();
    let (pool, _) = db::open(&paths).await.unwrap();
    let state = AppState::new(pool, paths);
    let principal = Principal {
        session_id: "isolated-session".into(),
        user_id: 1,
        username: "limited".into(),
        full_name: "Limited User".into(),
        roles: vec!["salesperson".into()],
        permissions: Vec::new(),
    };
    let create_error = furniture_shop_lib::application::backup_workflow::create_manual(
        &state,
        &principal,
        "Denied",
        "permission-test",
    )
    .await
    .unwrap_err();
    let inspect_error = furniture_shop_lib::application::backup_workflow::inspect(
        &state,
        &principal,
        "missing.furniture-backup",
    )
    .await
    .unwrap_err();
    assert_eq!(create_error.code(), "UNAUTHORIZED");
    assert_eq!(inspect_error.code(), "UNAUTHORIZED");
    state.pool.close().await;
    drop(state);
    let _ = std::fs::remove_dir_all(root);
}
