use std::fs;
use std::path::PathBuf;

use furniture_shop_lib::infrastructure as infra;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "furniture-shop-proof-{label}-{}",
        uuid::Uuid::now_v7()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn db_opens_runs_migrations_and_seeds() {
    let dir = temp_dir("db");
    let paths = infra::FilePaths::init(&dir).unwrap();
    let (pool, info) = infra::db::open(&paths).await.unwrap();

    assert_eq!(info.version, 12);
    assert_eq!(info.pending_migrations, 0);

    let role_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(role_count, 5);

    let perm_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM permissions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(perm_count, 46);

    let owner_perm_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
           FROM role_permissions rp
           JOIN roles r ON r.id = rp.role_id
          WHERE r.code = 'owner'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner_perm_count, perm_count);

    let loc_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM locations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(loc_count, 2);

    // Phase 4 inventory tables and views exist.
    let inventory_objects: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type IN ('table', 'view', 'index') AND name IN (
             'stock_movements', 'stock_reservations', 'stock_balances',
             'inventory_count_sessions', 'inventory_count_lines',
             'inventory_cost_layers', 'current_stock', 'current_valuation'
         )",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(inventory_objects, 8);

    // Phase 3 catalogue seeds (units) are present and idempotent.
    let unit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM units")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unit_count, 5);

    // Catalogue tables exist with the expected live-unique article index.
    let product_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type IN ('table', 'index') AND name IN (
             'categories', 'product_types', 'units', 'products',
             'product_images', 'product_attributes', 'uq_products_article_live'
         )",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(product_tables, 7);

    // Rerun migrations: must be a no-op.
    infra::db::MIGRATOR.run(&pool).await.unwrap();
    let version_again: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version_again, 12);

    // Phase 10 dashboard/search composite indexes are present.
    let phase10_indexes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'index' AND name IN (
             'idx_sales_status_date', 'idx_sales_returns_status_date',
             'idx_customer_payments_status_date', 'idx_supplier_payments_status_date',
             'idx_customers_name', 'idx_customers_phone',
             'idx_suppliers_name', 'idx_suppliers_phone',
             'idx_audit_actor_created'
         )",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase10_indexes, 9);

    // Walk-in payment account column (migration 0009) is present.
    let phase9_note: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('sales') WHERE name = 'payment_cash_account_id'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase9_note, 1);

    // Phase 7 due-control additions are present.
    let phase7_objects: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'index' AND name = 'idx_sales_due_control'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase7_objects, 1);
    let phase7_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('customers') WHERE name = 'credit_days'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase7_columns, 1);

    // Phase 8 fulfilment tables are present.
    let phase8_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'table' AND name IN (
             'delivery_items', 'sales_return_items', 'credit_notes', 'damage_records'
         )",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase8_tables, 4);
    let phase8_ledger_types: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('customer_ledger_entries')
         WHERE name = 'entry_type'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase8_ledger_types, 1);
    let phase8_cash_types: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('cash_entries') WHERE name = 'entry_type'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase8_cash_types, 1);
    let phase8_sequences: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM document_sequences
         WHERE document_type IN ('delivery', 'sales_return', 'credit_note', 'damage_record')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase8_sequences, 4);

    // Phase 9 expenses/cash/profit tables are present.
    let phase9_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'table' AND name IN (
             'expense_categories', 'expenses', 'owner_transactions'
         )",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase9_tables, 3);
    let phase9_sequences: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM document_sequences
         WHERE document_type IN ('expense', 'owner_transaction')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase9_sequences, 2);
    let phase9_permissions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM permissions
         WHERE code IN ('expense.view', 'expense.create', 'expense.reverse',
                        'profit.view', 'owner.transfer')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase9_permissions, 5);
    let phase9_category_seed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM expense_categories")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(phase9_category_seed, 12);
    let phase9_cash_entry_types: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('cash_entries')
         WHERE name = 'entry_type'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase9_cash_entry_types, 1);
    let phase9_accountant_grants: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
           FROM roles r
           JOIN role_permissions rp ON rp.role_id = r.id
           JOIN permissions p ON p.id = rp.permission_id
          WHERE r.code = 'accountant'
            AND p.code IN ('expense.view', 'expense.create', 'expense.reverse', 'profit.view')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase9_accountant_grants, 4);
    let owner_transfer_owner_only: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
           FROM roles r
           JOIN role_permissions rp ON rp.role_id = r.id
           JOIN permissions p ON p.id = rp.permission_id
          WHERE p.code = 'owner.transfer' AND r.code <> 'owner'
            AND r.code <> 'manager'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner_transfer_owner_only, 0);

    // Owner role template grants every permission (Phase 2 seed invariant).
    let owner_has_all: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
           FROM permissions p
          WHERE NOT EXISTS (
             SELECT 1
               FROM role_permissions rp
              JOIN roles r ON r.id = rp.role_id
              WHERE r.code = 'owner' AND rp.permission_id = p.id
          )",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner_has_all, 0);

    pool.close().await;
    drop(paths);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn pdf_embeds_unicode_font_and_renders() {
    let dir = temp_dir("pdf");
    let paths = infra::FilePaths::init(&dir).unwrap();

    let pdf = infra::generate_proof_pdf(&paths.reports_dir, &paths.fonts_dir)
        .expect("generate_proof_pdf succeeded");

    assert_eq!(pdf.pages, 1);
    assert!(pdf.bytes > 1000, "pdf should be non-trivial in size");
    assert_eq!(
        fs::metadata(&pdf.path).unwrap().len(),
        pdf.bytes,
        "reported size matches on-disk size"
    );

    // The embedded font must have been materialized in the fonts dir.
    assert!(paths
        .fonts_dir
        .join("NotoNastaliqUrdu-Regular.ttf")
        .exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn image_pipeline_validates_and_creates_thumbnails() {
    let dir = temp_dir("image");
    let paths = infra::FilePaths::init(&dir).unwrap();

    // Build a valid 320x200 PNG in memory.
    let img = image::DynamicImage::new_rgba8(320, 200);
    let source = dir.join("sample.png");
    img.save(&source).unwrap();

    let imported = infra::import_image(&source, &paths.images_dir).expect("import ok");
    assert_eq!(imported.width, 320);
    assert_eq!(imported.height, 200);
    assert_eq!(
        imported.stored_name.rsplit_once('.').map(|(_, e)| e),
        Some("webp")
    );
    assert!(paths.images_dir.join(&imported.stored_name).exists());
    let thumb_name = imported.stored_name.replace(".webp", "-thumb.webp");
    assert!(paths.images_dir.join(&thumb_name).exists());

    // A non-image file must be rejected cleanly.
    let junk = dir.join("pretend.png");
    fs::write(&junk, b"this is not an image").unwrap();
    let err = infra::import_image(&junk, &paths.images_dir)
        .err()
        .expect("junk file produces an error");
    assert!(matches!(err, furniture_shop_lib::error::AppError::Image(_)));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn backup_snapshot_is_checksummed_and_verified() {
    let dir = temp_dir("backup");
    let paths = infra::FilePaths::init(&dir).unwrap();

    // Create a database first so the backup has real content.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (pool, _info) = rt.block_on(infra::db::open(&paths)).unwrap();
    rt.block_on(async {
        sqlx::query("INSERT OR IGNORE INTO settings (key, value_json) VALUES ('shop.name', '\"Proof Shop\"')")
            .execute(&pool)
            .await
            .unwrap();
    });
    rt.block_on(pool.close());

    let backup = infra::create_backup(&paths.db_path, &paths.backups_dir).expect("backup ok");
    assert!(backup.verified);
    assert_eq!(backup.sha256.len(), 64);
    assert!(fs::metadata(&backup.backup_path).unwrap().len() > 0);

    let list = infra::list_backups(&paths.backups_dir).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].sha256, backup.sha256);

    let _ = fs::remove_dir_all(&dir);
}
