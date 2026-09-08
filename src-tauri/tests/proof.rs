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

    assert_eq!(info.version, 1);
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
    assert!(perm_count >= 13);

    let loc_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM locations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(loc_count, 1);

    // Rerun migrations: must be a no-op.
    infra::db::MIGRATOR.run(&pool).await.unwrap();
    let version_again: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(version_again, 1);

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
    assert!(paths.fonts_dir.join("NotoNastaliqUrdu-Regular.ttf").exists());

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
    let thumb_name = imported
        .stored_name
        .replace(".webp", "-thumb.webp");
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