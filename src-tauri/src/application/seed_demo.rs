use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SeedResult {
    pub categories: i64,
    pub product_types: i64,
    pub products: i64,
    pub suppliers: i64,
    pub customers: i64,
    pub purchases: i64,
    pub sales: i64,
    pub expenses: i64,
    pub bundles: i64,
    pub customer_payments: i64,
    pub supplier_payments: i64,
    pub sales_returns: i64,
    pub credit_notes: i64,
    pub deliveries: i64,
    pub damage_records: i64,
}

pub async fn seed_demo_data(state: &AppState) -> Result<SeedResult, AppError> {
    let now = state.clock.now_iso();

    let admin_id: i64 = sqlx::query_scalar("SELECT id FROM users ORDER BY id LIMIT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(|_| {
            AppError::Internal("no users found; complete first-run before seeding".into())
        })?;

    let showroom_id: i64 =
        sqlx::query_scalar("SELECT id FROM locations WHERE type = 'showroom' ORDER BY id LIMIT 1")
            .fetch_one(&state.pool)
            .await?;
    let store_id: i64 =
        sqlx::query_scalar("SELECT id FROM locations WHERE type = 'store' ORDER BY id LIMIT 1")
            .fetch_optional(&state.pool)
            .await?
            .unwrap_or(showroom_id);

    let cash_acct: i64 = sqlx::query_scalar(
        "SELECT id FROM cash_accounts WHERE code = 'main_cash'",
    )
    .fetch_one(&state.pool)
    .await?;

    let unit_pcs: i64 = sqlx::query_scalar("SELECT id FROM units WHERE code = 'pcs'")
        .fetch_one(&state.pool)
        .await?;

    let product_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
        .fetch_one(&state.pool)
        .await?;
    let skip_core = product_count > 0;

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                // ══════════════════════════════════════════════════════════
                // CATEGORIES  (skip if already seeded)
                // ══════════════════════════════════════════════════════════
                let (cat_living, cat_bedroom, cat_dining, cat_office, cat_sofas, cat_tables, cat_tv, cat_beds, cat_wardrobes, cat_nightstands, cat_dtables, cat_dchairs, cat_odesks, cat_ochairs, cat_bshelf, cat_beds_set, cat_dress, cat_dchair_single);

                if !skip_core {
                    cat_living = insert_category(tx, "Living Room", 1, &now).await?;
                    cat_sofas = insert_category_child(tx, "Sofas & Couches", cat_living, 1, &now).await?;
                    cat_tables = insert_category_child(tx, "Coffee Tables", cat_living, 2, &now).await?;
                    cat_tv = insert_category_child(tx, "TV Units", cat_living, 3, &now).await?;
                    cat_bshelf = insert_category_child(tx, "Bookshelves", cat_living, 4, &now).await?;

                    cat_bedroom = insert_category(tx, "Bedroom", 2, &now).await?;
                    cat_beds = insert_category_child(tx, "Beds & Frames", cat_bedroom, 1, &now).await?;
                    cat_wardrobes = insert_category_child(tx, "Wardrobes", cat_bedroom, 2, &now).await?;
                    cat_nightstands = insert_category_child(tx, "Nightstands", cat_bedroom, 3, &now).await?;
                    cat_dress = insert_category_child(tx, "Dressing Tables", cat_bedroom, 4, &now).await?;
                    cat_beds_set = insert_category_child(tx, "Bed Sets", cat_bedroom, 5, &now).await?;

                    cat_dining = insert_category(tx, "Dining", 3, &now).await?;
                    cat_dtables = insert_category_child(tx, "Dining Tables", cat_dining, 1, &now).await?;
                    cat_dchairs = insert_category_child(tx, "Dining Chairs", cat_dining, 2, &now).await?;
                    cat_dchair_single = insert_category_child(tx, "Individual Chairs", cat_dining, 3, &now).await?;

                    cat_office = insert_category(tx, "Office", 4, &now).await?;
                    cat_odesks = insert_category_child(tx, "Office Desks", cat_office, 1, &now).await?;
                    cat_ochairs = insert_category_child(tx, "Office Chairs", cat_office, 2, &now).await?;
                } else {
                    #[allow(unused_assignments)]
                    { cat_living = 0; cat_bedroom = 0; cat_dining = 0; cat_office = 0;
                    cat_sofas = 0; cat_tables = 0; cat_tv = 0; cat_beds = 0;
                    cat_wardrobes = 0; cat_nightstands = 0; cat_dtables = 0; cat_dchairs = 0;
                    cat_odesks = 0; cat_ochairs = 0; cat_bshelf = 0; cat_beds_set = 0;
                    cat_dress = 0; cat_dchair_single = 0; }
                }

                let cat_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM categories")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // PRODUCT TYPES  (skip if already seeded)
                // ══════════════════════════════════════════════════════════
                if !skip_core {
                    let _ = insert_pt(tx, cat_sofas, "3-Seater Sofa", &now).await;
                    let _ = insert_pt(tx, cat_sofas, "L-Shape Sofa", &now).await;
                    let _ = insert_pt(tx, cat_sofas, "Recliner Sofa", &now).await;
                    let _ = insert_pt(tx, cat_sofas, "Fabric Sofa", &now).await;
                    let _ = insert_pt(tx, cat_tables, "Center Table", &now).await;
                    let _ = insert_pt(tx, cat_tables, "Nesting Tables", &now).await;
                    let _ = insert_pt(tx, cat_tables, "Round Coffee Table", &now).await;
                    let _ = insert_pt(tx, cat_tv, "TV Cabinet", &now).await;
                    let _ = insert_pt(tx, cat_tv, "Wall-Mounted Unit", &now).await;
                    let _ = insert_pt(tx, cat_tv, "TV Stand", &now).await;
                    let _ = insert_pt(tx, cat_bshelf, "5-Tier Bookshelf", &now).await;
                    let _ = insert_pt(tx, cat_beds, "King Size Bed", &now).await;
                    let _ = insert_pt(tx, cat_beds, "Double Bed", &now).await;
                    let _ = insert_pt(tx, cat_beds, "Single Bed", &now).await;
                    let _ = insert_pt(tx, cat_wardrobes, "4-Door Wardrobe", &now).await;
                    let _ = insert_pt(tx, cat_wardrobes, "Sliding Door Wardrobe", &now).await;
                    let _ = insert_pt(tx, cat_nightstands, "2-Drawer Nightstand", &now).await;
                    let _ = insert_pt(tx, cat_dress, "Dressing Table", &now).await;
                    let _ = insert_pt(tx, cat_beds_set, "Complete Bed Set", &now).await;
                    let _ = insert_pt(tx, cat_dtables, "6-Seater Dining Set", &now).await;
                    let _ = insert_pt(tx, cat_dtables, "4-Seater Dining Set", &now).await;
                    let _ = insert_pt(tx, cat_dtables, "Solid Wood Dining Table", &now).await;
                    let _ = insert_pt(tx, cat_dchairs, "Dining Chair Set of 4", &now).await;
                    let _ = insert_pt(tx, cat_dchair_single, "Upholstered Dining Chair", &now).await;
                    let _ = insert_pt(tx, cat_odesks, "Executive Desk", &now).await;
                    let _ = insert_pt(tx, cat_odesks, "L-Shaped Workstation", &now).await;
                    let _ = insert_pt(tx, cat_ochairs, "Executive Chair", &now).await;
                    let _ = insert_pt(tx, cat_ochairs, "Task Chair", &now).await;
                }

                let pt_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_types")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // PRODUCTS  (INSERT OR IGNORE — safe to re-run)
                // ══════════════════════════════════════════════════════════
                #[allow(clippy::type_complexity)]
                let products: &[(i64, i64, &str, &str, &str, i64, i64, i64)] = &[
                    // ── Sofas ──
                    (cat_sofas,   0, "FS-001", "Royal 3-Seater Sofa",         "Sheesham Wood",     4_500_000,  6_500_000,  3),
                    (cat_sofas,   0, "FS-002", "Modern L-Shape Sofa",         "Sheesham Wood",     6_500_000,  9_500_000,  2),
                    (cat_sofas,   0, "FS-003", "Comfort Recliner Sofa",       "Pine Wood",         5_500_000,  8_000_000,  2),
                    (cat_sofas,   0, "FS-022", "Fabric 3-Seater Sofa",        "Fabric & Oak",      3_800_000,  5_500_000,  4),
                    (cat_sofas,   0, "FS-031", "Single Seater Armchair",      "Sheesham Wood",     1_800_000,  2_700_000,  6),
                    (cat_sofas,   0, "FS-032", "Fabric loveseat",             "Fabric & Pine",     2_500_000,  3_800_000,  4),
                    // ── Tables ──
                    (cat_tables,  0, "FS-004", "Marble Center Table",         "Marble & Steel",    1_200_000,  1_800_000,  5),
                    (cat_tables,  0, "FS-005", "Wood Nesting Tables",         "Oak Wood",            800_000,  1_200_000,  4),
                    (cat_tables,  0, "FS-027", "Round Coffee Table",          "Sheesham Wood",       600_000,    950_000,  6),
                    (cat_tables,  0, "FS-033", "Rectangular Coffee Table",    "MDF & Oak",           750_000,  1_100_000,  5),
                    // ── TV Units ──
                    (cat_tv,      0, "FS-006", "Elegant TV Cabinet",          "MDF & Glass",      1_800_000,  2_800_000,  3),
                    (cat_tv,      0, "FS-007", "Wall-Mounted TV Unit",        "MDF",              1_500_000,  2_200_000,  4),
                    (cat_tv,      0, "FS-028", "TV Stand with Drawers",       "Sheesham Wood",    1_200_000,  1_800_000,  4),
                    // ── Bookshelves ──
                    (cat_bshelf,  0, "FS-029", "5-Tier Open Bookshelf",       "Sheesham & Steel",   900_000,  1_400_000,  3),
                    (cat_bshelf,  0, "FS-034", "Glass-Door Bookshelf",        "Sheesham & Glass", 1_500_000,  2_300_000,  2),
                    // ── Beds ──
                    (cat_beds,    0, "FS-008", "King Size Bed Frame",         "Sheesham Wood",    3_500_000,  5_200_000,  2),
                    (cat_beds,    0, "FS-009", "Classic Double Bed",           "Sheesham Wood",    2_500_000,  3_800_000,  3),
                    (cat_beds,    0, "FS-010", "Single Bed Frame",             "Pine Wood",        1_500_000,  2_200_000,  4),
                    (cat_beds,    0, "FS-023", "King Size Bed with Storage",   "Sheesham Wood",    4_200_000,  6_200_000,  2),
                    (cat_beds,    0, "FS-035", "Upholstered King Bed",         "Fabric & Oak",     4_800_000,  7_000_000,  1),
                    // ── Wardrobes ──
                    (cat_wardrobes, 0, "FS-011", "4-Door Wardrobe",            "Sheesham Wood",    4_500_000,  6_800_000,  2),
                    (cat_wardrobes, 0, "FS-012", "Sliding Door Wardrobe",      "MDF & Mirror",     5_500_000,  8_200_000,  1),
                    (cat_wardrobes, 0, "FS-026", "3-Door Wardrobe",            "Sheesham Wood",    3_800_000,  5_600_000,  2),
                    // ── Nightstands ──
                    (cat_nightstands, 0, "FS-013", "2-Drawer Nightstand",      "Sheesham Wood",      800_000,  1_200_000,  6),
                    (cat_nightstands, 0, "FS-024", "Nightstand Set of 2",      "Sheesham Wood",    1_400_000,  2_100_000,  4),
                    // ── Dressing Tables ──
                    (cat_dress,   0, "FS-025", "Dressing Table with Mirror",  "Sheesham Wood",    2_200_000,  3_300_000,  3),
                    (cat_dress,   0, "FS-036", "Compact Vanity Table",        "MDF & Mirror",     1_200_000,  1_800_000,  4),
                    // ── Dining Tables ──
                    (cat_dtables, 0, "FS-014", "6-Seater Dining Table",       "Sheesham Wood",    5_500_000,  8_200_000,  2),
                    (cat_dtables, 0, "FS-015", "4-Seater Dining Table",       "Oak Wood",         3_500_000,  5_200_000,  3),
                    (cat_dtables, 0, "FS-021", "Solid Wood Extending Table",  "Sheesham Wood",    6_000_000,  9_000_000,  2),
                    // ── Dining Chairs ──
                    (cat_dchairs, 0, "FS-016", "Dining Chair Set of 4",       "Sheesham Wood",    1_200_000,  1_800_000,  5),
                    (cat_dchairs, 0, "FS-030", "Upholstered Dining Chair x4", "Fabric & Oak",     1_600_000,  2_400_000,  4),
                    (cat_dchair_single, 0, "FS-037", "Single Dining Chair",   "Sheesham Wood",      350_000,    520_000, 10),
                    // ── Office Desks ──
                    (cat_odesks,  0, "FS-017", "Executive Office Desk",       "Sheesham Wood",    2_500_000,  3_800_000,  2),
                    (cat_odesks,  0, "FS-018", "L-Shaped Workstation",        "MDF & Steel",      1_800_000,  2_800_000,  3),
                    (cat_odesks,  0, "FS-038", "Standing Desk Converter",     "MDF & Steel",      1_200_000,  1_800_000,  3),
                    // ── Office Chairs ──
                    (cat_ochairs, 0, "FS-019", "Executive Office Chair",      "Leather & Chrome",  1_200_000,  1_900_000,  4),
                    (cat_ochairs, 0, "FS-020", "Ergonomic Task Chair",        "Mesh & Nylon",       800_000,  1_200_000,  6),
                ];

                for &(cat_id, _pt_id, article, name, material, cost, sale, min_stock) in products {
                    let pt_id_val: Option<i64> = if _pt_id > 0 { Some(_pt_id) } else { None };
                    sqlx::query(
                        "INSERT OR IGNORE INTO products
                             (article_number, article_number_norm, name, category_id, product_type_id,
                              unit_id, material, cost_minor, sale_price_minor, minimum_stock,
                              track_stock, is_active, created_at, updated_at)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 1, ?, ?)",
                    )
                    .bind(article)
                    .bind(article.to_lowercase())
                    .bind(name)
                    .bind(cat_id)
                    .bind(pt_id_val)
                    .bind(unit_pcs)
                    .bind(material)
                    .bind(cost)
                    .bind(sale)
                    .bind(min_stock)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }

                let prod_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // SUPPLIERS  (INSERT OR IGNORE)
                // ══════════════════════════════════════════════════════════
                let suppliers: &[(&str, &str, &str, &str, &str, i64)] = &[
                    ("SUP-001", "Al-Hamd Furniture Industries", "042-35123456", "info@alhamd.pk",       "Sialkot Road, Lahore",      5_000_000),
                    ("SUP-002", "Karachi Wood Works",           "021-34567890", "sales@karachiwood.pk",  "SITE Area, Karachi",        3_000_000),
                    ("SUP-003", "Pak Ceramic & Marble",         "041-87654321", "orders@pakmarble.pk",   "D-Ground, Faisalabad",      2_000_000),
                    ("SUP-004", "Sheesham Export Hub",           "052-4567890",  "export@sheesham.pk",    "Bhai Wala, Gujrat",         4_000_000),
                ];

                for &(code, name, phone, email, address, opening) in suppliers {
                    sqlx::query(
                        "INSERT OR IGNORE INTO suppliers
                             (code, name, phone, email, address, opening_balance_minor,
                              is_active, created_by, created_at, updated_at)
                         VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?)",
                    )
                    .bind(code).bind(name).bind(phone).bind(email).bind(address)
                    .bind(opening).bind(admin_id).bind(&now).bind(&now)
                    .execute(&mut *tx).await?;
                }

                let sup_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM suppliers")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // CUSTOMERS  (INSERT OR IGNORE)
                // ══════════════════════════════════════════════════════════
                let customers: &[(&str, &str, &str, &str, i64)] = &[
                    ("CUS-001", "Ahmed Khan & Sons",              "0300-1234567", "ahmed.khan@gmail.com",     5_000_000),
                    ("CUS-002", "Fatima Enterprises",              "0321-9876543", "fatima.ent@gmail.com",     3_000_000),
                    ("CUS-003", "Muhammad Ali Builders",           "0333-5551234", "ali.builders@gmail.com",  10_000_000),
                    ("CUS-004", "Sara Interior Design Studio",     "0345-2223344", "sara.interior@gmail.com",  2_000_000),
                    ("CUS-005", "Habib Residence",                 "0312-8889900", "habib.res@gmail.com",      1_500_000),
                    ("CUS-006", "Royal Apartments (DHA Phase 5)",  "0300-7654321", "royal.apt@gmail.com",      8_000_000),
                    ("CUS-007", "Green Valley Homes",              "0321-1122334", "green.valley@gmail.com",   6_000_000),
                ];

                for &(code, name, phone, email, credit) in customers {
                    sqlx::query(
                        "INSERT OR IGNORE INTO customers
                             (code, name, phone, email, credit_limit_minor,
                              is_active, created_by, created_at, updated_at)
                         VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?)",
                    )
                    .bind(code).bind(name).bind(phone).bind(email).bind(credit)
                    .bind(admin_id).bind(&now).bind(&now)
                    .execute(&mut *tx).await?;
                }

                let cust_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM customers")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // BUNDLES  (bedroom, dining, sofa sets)
                // ══════════════════════════════════════════════════════════
                let existing_bundles: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bundles")
                    .fetch_one(&mut *tx).await?;

                if existing_bundles == 0 {
                    // ── Bedroom Set ──
                    let br_id = sqlx::query(
                        "INSERT INTO bundles (code, name, description, default_price_minor,
                             is_active, created_by, created_at, updated_at)
                         VALUES ('BR-001', 'Complete Bedroom Set',
                             'King bed + 2 nightstands + dressing table + 3-door wardrobe. Save 10%.',
                             145_000_000, 1, ?, ?, ?)",
                    )
                    .bind(admin_id).bind(&now).bind(&now)
                    .execute(&mut *tx).await?.last_insert_rowid();

                    for (article, qty, sort) in [
                        ("FS-023", 1, 1),  // King bed with storage
                        ("FS-024", 1, 2),  // Nightstand set of 2
                        ("FS-025", 1, 3),  // Dressing table
                        ("FS-026", 1, 4),  // 3-door wardrobe
                    ] {
                        let pid: i64 = sqlx::query_scalar("SELECT id FROM products WHERE article_number = ?")
                            .bind(article).fetch_one(&mut *tx).await?;
                        sqlx::query(
                            "INSERT INTO bundle_items (bundle_id, product_id, quantity, sort_order)
                             VALUES (?, ?, ?, ?)",
                        )
                        .bind(br_id).bind(pid).bind(qty).bind(sort)
                        .execute(&mut *tx).await?;
                    }

                    // ── Dining Set ──
                    let dn_id = sqlx::query(
                        "INSERT INTO bundles (code, name, description, default_price_minor,
                             is_active, created_by, created_at, updated_at)
                         VALUES ('DN-001', '6-Seater Dining Set',
                             'Extending dining table + 6 upholstered chairs. Save 12%.',
                             120_000_000, 1, ?, ?, ?)",
                    )
                    .bind(admin_id).bind(&now).bind(&now)
                    .execute(&mut *tx).await?.last_insert_rowid();

                    for (article, qty, sort) in [
                        ("FS-021", 1, 1),  // Solid wood extending table
                        ("FS-030", 1, 2),  // Upholstered dining chair x4
                        ("FS-037", 2, 3),  // 2 single dining chairs
                    ] {
                        let pid: i64 = sqlx::query_scalar("SELECT id FROM products WHERE article_number = ?")
                            .bind(article).fetch_one(&mut *tx).await?;
                        sqlx::query(
                            "INSERT INTO bundle_items (bundle_id, product_id, quantity, sort_order)
                             VALUES (?, ?, ?, ?)",
                        )
                        .bind(dn_id).bind(pid).bind(qty).bind(sort)
                        .execute(&mut *tx).await?;
                    }

                    // ── Sofa / Living Room Set ──
                    let sf_id = sqlx::query(
                        "INSERT INTO bundles (code, name, description, default_price_minor,
                             is_active, created_by, created_at, updated_at)
                         VALUES ('SF-001', 'Living Room Package',
                             '3-seater sofa + 2 single armchairs + round coffee table. Save 8%.',
                             110_000_000, 1, ?, ?, ?)",
                    )
                    .bind(admin_id).bind(&now).bind(&now)
                    .execute(&mut *tx).await?.last_insert_rowid();

                    for (article, qty, sort) in [
                        ("FS-022", 1, 1),  // Fabric 3-seater
                        ("FS-031", 2, 2),  // 2 single armchairs
                        ("FS-027", 1, 3),  // Round coffee table
                    ] {
                        let pid: i64 = sqlx::query_scalar("SELECT id FROM products WHERE article_number = ?")
                            .bind(article).fetch_one(&mut *tx).await?;
                        sqlx::query(
                            "INSERT INTO bundle_items (bundle_id, product_id, quantity, sort_order)
                             VALUES (?, ?, ?, ?)",
                        )
                        .bind(sf_id).bind(pid).bind(qty).bind(sort)
                        .execute(&mut *tx).await?;
                    }
                }

                let bundle_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bundles")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // PURCHASES  (6 total, spread across 45 days)
                // ══════════════════════════════════════════════════════════
                let existing_purchases: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM purchases")
                    .fetch_one(&mut *tx).await?;

                if existing_purchases == 0 {
                    // Purchase 1 — Al-Hamd, oldest
                    let p1 = insert_purchase_dated(tx, "SUP-001", showroom_id, admin_id, "INV-ALH-001", "2026-07-28", &now).await?;
                    insert_purchase_item(tx, p1, "FS-001", 5, 4_500_000).await?;
                    insert_purchase_item(tx, p1, "FS-002", 3, 6_500_000).await?;
                    insert_purchase_item(tx, p1, "FS-004", 10, 1_200_000).await?;
                    let t1: i64 = 5*4_500_000 + 3*6_500_000 + 10*1_200_000;
                    sqlx::query("UPDATE purchases SET total_minor=?, paid_minor=?, due_minor=?, status='posted', posted_by=?, posted_at=? WHERE id=?")
                        .bind(t1).bind(t1).bind(0i64).bind(admin_id).bind(&now).bind(p1).execute(&mut *tx).await?;

                    // Purchase 2 — Karachi Wood, 30 days ago
                    let p2 = insert_purchase_dated(tx, "SUP-002", store_id, admin_id, "INV-KWW-102", "2026-08-12", &now).await?;
                    insert_purchase_item(tx, p2, "FS-008", 4, 3_500_000).await?;
                    insert_purchase_item(tx, p2, "FS-009", 6, 2_500_000).await?;
                    insert_purchase_item(tx, p2, "FS-011", 2, 4_500_000).await?;
                    let t2: i64 = 4*3_500_000 + 6*2_500_000 + 2*4_500_000;
                    let paid2: i64 = t2 * 60 / 100;
                    sqlx::query("UPDATE purchases SET total_minor=?, paid_minor=?, due_minor=?, status='posted', posted_by=?, posted_at=? WHERE id=?")
                        .bind(t2).bind(paid2).bind(t2 - paid2).bind(admin_id).bind(&now).bind(p2).execute(&mut *tx).await?;

                    // Purchase 3 — Sheesham Export, 20 days ago
                    let p3 = insert_purchase_dated(tx, "SUP-004", showroom_id, admin_id, "INV-SEH-205", "2026-08-22", &now).await?;
                    insert_purchase_item(tx, p3, "FS-014", 3, 5_500_000).await?;
                    insert_purchase_item(tx, p3, "FS-016", 8, 1_200_000).await?;
                    insert_purchase_item(tx, p3, "FS-019", 5, 1_200_000).await?;
                    let t3: i64 = 3*5_500_000 + 8*1_200_000 + 5*1_200_000;
                    let paid3: i64 = t3 * 40 / 100;
                    sqlx::query("UPDATE purchases SET total_minor=?, paid_minor=?, due_minor=?, status='posted', posted_by=?, posted_at=? WHERE id=?")
                        .bind(t3).bind(paid3).bind(t3 - paid3).bind(admin_id).bind(&now).bind(p3).execute(&mut *tx).await?;

                    // Purchase 4 — Al-Hamd, 14 days ago
                    let p4 = insert_purchase_dated(tx, "SUP-001", showroom_id, admin_id, "INV-ALH-014", "2026-08-28", &now).await?;
                    insert_purchase_item(tx, p4, "FS-022", 4, 3_800_000).await?;
                    insert_purchase_item(tx, p4, "FS-031", 6, 1_800_000).await?;
                    insert_purchase_item(tx, p4, "FS-027", 8, 600_000).await?;
                    let t4: i64 = 4*3_800_000 + 6*1_800_000 + 8*600_000;
                    sqlx::query("UPDATE purchases SET total_minor=?, paid_minor=0, due_minor=?, status='posted', posted_by=?, posted_at=? WHERE id=?")
                        .bind(t4).bind(t4).bind(admin_id).bind(&now).bind(p4).execute(&mut *tx).await?;

                    // Purchase 5 — Pak Marble, 7 days ago
                    let p5 = insert_purchase_dated(tx, "SUP-003", showroom_id, admin_id, "INV-PCM-089", "2026-09-04", &now).await?;
                    insert_purchase_item(tx, p5, "FS-004", 6, 1_200_000).await?;
                    insert_purchase_item(tx, p5, "FS-033", 4, 750_000).await?;
                    let t5: i64 = 6*1_200_000 + 4*750_000;
                    sqlx::query("UPDATE purchases SET total_minor=?, paid_minor=?, due_minor=?, status='posted', posted_by=?, posted_at=? WHERE id=?")
                        .bind(t5).bind(t5 / 2).bind(t5 - t5/2).bind(admin_id).bind(&now).bind(p5).execute(&mut *tx).await?;

                    // Purchase 6 — Karachi Wood, yesterday
                    let p6 = insert_purchase_dated(tx, "SUP-002", store_id, admin_id, "INV-KWW-118", "2026-09-10", &now).await?;
                    insert_purchase_item(tx, p6, "FS-023", 2, 4_200_000).await?;
                    insert_purchase_item(tx, p6, "FS-035", 1, 4_800_000).await?;
                    insert_purchase_item(tx, p6, "FS-026", 3, 3_800_000).await?;
                    let t6: i64 = 2*4_200_000 + 4_800_000 + 3*3_800_000;
                    sqlx::query("UPDATE purchases SET total_minor=?, paid_minor=0, due_minor=?, status='posted', posted_by=?, posted_at=? WHERE id=?")
                        .bind(t6).bind(t6).bind(admin_id).bind(&now).bind(p6).execute(&mut *tx).await?;
                }

                let pur_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM purchases")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // STOCK OPENING BALANCES  (idempotent via INSERT OR IGNORE)
                // ══════════════════════════════════════════════════════════
                let stock_products: &[(i64, i64)] = &[
                    (1, 12), (2, 8), (3, 5), (4, 20), (5, 15), (6, 8), (7, 10), (8, 6),
                    (9, 10), (10, 12), (11, 4), (12, 3), (13, 8), (14, 5), (15, 6),
                    (16, 12), (17, 8), (18, 10), (19, 6), (20, 10),
                    (21, 5), (22, 6), (23, 4), (24, 8), (25, 3), (26, 4),
                    (27, 6), (28, 5), (29, 4), (30, 6), (31, 8), (32, 6),
                    (33, 5), (34, 3), (35, 2), (36, 4), (37, 10), (38, 3),
                ];
                for &(prod_id, qty) in stock_products {
                    let existing: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM stock_balances WHERE product_id = ? AND location_id = ?",
                    )
                    .bind(prod_id).bind(showroom_id).fetch_one(&mut *tx).await?;
                    if existing == 0 {
                        sqlx::query(
                            "INSERT INTO stock_balances (product_id, location_id, on_hand, reserved, damaged)
                             VALUES (?, ?, ?, 0, 0)",
                        )
                        .bind(prod_id).bind(showroom_id).bind(qty)
                        .execute(&mut *tx).await?;
                        sqlx::query(
                            "INSERT INTO stock_movements
                                 (product_id, location_id, movement_type, quantity_delta, unit_cost_minor,
                                  reason, created_by, created_at)
                             VALUES (?, ?, 'opening', ?, 0, 'Demo opening stock', ?, ?)",
                        )
                        .bind(prod_id).bind(showroom_id).bind(qty).bind(admin_id).bind(&now)
                        .execute(&mut *tx).await?;
                    }
                }

                // ══════════════════════════════════════════════════════════
                // SALES  (10 total, spread across 40 days, various payment states)
                // ══════════════════════════════════════════════════════════
                let existing_sales: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sales")
                    .fetch_one(&mut *tx).await?;

                if existing_sales == 0 {
                    // ── Sale 1: fully paid ──
                    let s1 = insert_sale_dated(tx, "CUS-001", showroom_id, admin_id, "2026-07-28", &now).await?;
                    insert_sale_item(tx, s1, 1, "FS-001", 1, 6_500_000, 4_500_000).await?;
                    insert_sale_item(tx, s1, 2, "FS-004", 2, 1_800_000, 1_200_000).await?;
                    let st1: i64 = 6_500_000 + 2*1_800_000;
                    let sc1: i64 = 4_500_000 + 2*1_200_000;
                    sqlx::query("UPDATE sales SET subtotal_minor=?, total_minor=?, cost_minor=?, paid_minor=?, due_minor=0, status='confirmed', confirmed_by=?, confirmed_at=? WHERE id=?")
                        .bind(st1).bind(st1).bind(sc1).bind(st1).bind(admin_id).bind(&now).bind(s1).execute(&mut *tx).await?;

                    // ── Sale 2: fully paid ──
                    let s2 = insert_sale_dated(tx, "CUS-002", showroom_id, admin_id, "2026-08-05", &now).await?;
                    insert_sale_item(tx, s2, 1, "FS-022", 2, 5_500_000, 3_800_000).await?;
                    insert_sale_item(tx, s2, 2, "FS-027", 3, 950_000, 600_000).await?;
                    let st2: i64 = 2*5_500_000 + 3*950_000;
                    let sc2: i64 = 2*3_800_000 + 3*600_000;
                    sqlx::query("UPDATE sales SET subtotal_minor=?, total_minor=?, cost_minor=?, paid_minor=?, due_minor=0, status='confirmed', confirmed_by=?, confirmed_at=? WHERE id=?")
                        .bind(st2).bind(st2).bind(sc2).bind(st2).bind(admin_id).bind(&now).bind(s2).execute(&mut *tx).await?;

                    // ── Sale 3: fully paid ──
                    let s3 = insert_sale_dated(tx, "CUS-004", showroom_id, admin_id, "2026-08-10", &now).await?;
                    insert_sale_item(tx, s3, 1, "FS-012", 1, 8_200_000, 5_500_000).await?;
                    insert_sale_item(tx, s3, 2, "FS-028", 2, 1_800_000, 1_200_000).await?;
                    let st3: i64 = 8_200_000 + 2*1_800_000;
                    let sc3: i64 = 5_500_000 + 2*1_200_000;
                    sqlx::query("UPDATE sales SET subtotal_minor=?, total_minor=?, cost_minor=?, paid_minor=?, due_minor=0, status='confirmed', confirmed_by=?, confirmed_at=? WHERE id=?")
                        .bind(st3).bind(st3).bind(sc3).bind(st3).bind(admin_id).bind(&now).bind(s3).execute(&mut *tx).await?;

                    // ── Sale 4: partially paid (40%) ──
                    let s4 = insert_sale_dated(tx, "CUS-003", showroom_id, admin_id, "2026-08-15", &now).await?;
                    insert_sale_item(tx, s4, 1, "FS-014", 1, 8_200_000, 5_500_000).await?;
                    insert_sale_item(tx, s4, 2, "FS-016", 2, 1_800_000, 1_200_000).await?;
                    insert_sale_item(tx, s4, 3, "FS-017", 1, 3_800_000, 2_500_000).await?;
                    let st4: i64 = 8_200_000 + 2*1_800_000 + 3_800_000;
                    let sc4: i64 = 5_500_000 + 2*1_200_000 + 2_500_000;
                    let paid4: i64 = st4 * 40 / 100;
                    sqlx::query("UPDATE sales SET subtotal_minor=?, total_minor=?, cost_minor=?, paid_minor=?, due_minor=?, status='confirmed', confirmed_by=?, confirmed_at=? WHERE id=?")
                        .bind(st4).bind(st4).bind(sc4).bind(paid4).bind(st4 - paid4).bind(admin_id).bind(&now).bind(s4).execute(&mut *tx).await?;

                    // ── Sale 5: no payment (full due) ──
                    let s5 = insert_sale_dated(tx, "CUS-006", showroom_id, admin_id, "2026-08-20", &now).await?;
                    insert_sale_item(tx, s5, 1, "FS-023", 1, 6_200_000, 4_200_000).await?;
                    insert_sale_item(tx, s5, 2, "FS-024", 1, 2_100_000, 1_400_000).await?;
                    insert_sale_item(tx, s5, 3, "FS-025", 1, 3_300_000, 2_200_000).await?;
                    let st5: i64 = 6_200_000 + 2_100_000 + 3_300_000;
                    let sc5: i64 = 4_200_000 + 1_400_000 + 2_200_000;
                    sqlx::query("UPDATE sales SET subtotal_minor=?, total_minor=?, cost_minor=?, paid_minor=0, due_minor=?, status='confirmed', confirmed_by=?, confirmed_at=? WHERE id=?")
                        .bind(st5).bind(st5).bind(sc5).bind(st5).bind(admin_id).bind(&now).bind(s5).execute(&mut *tx).await?;

                    // ── Sale 6: partially paid (60%) ──
                    let s6 = insert_sale_dated(tx, "CUS-007", showroom_id, admin_id, "2026-08-28", &now).await?;
                    insert_sale_item(tx, s6, 1, "FS-021", 1, 9_000_000, 6_000_000).await?;
                    insert_sale_item(tx, s6, 2, "FS-030", 2, 2_400_000, 1_600_000).await?;
                    let st6: i64 = 9_000_000 + 2*2_400_000;
                    let sc6: i64 = 6_000_000 + 2*1_600_000;
                    let paid6: i64 = st6 * 60 / 100;
                    sqlx::query("UPDATE sales SET subtotal_minor=?, total_minor=?, cost_minor=?, paid_minor=?, due_minor=?, status='confirmed', confirmed_by=?, confirmed_at=? WHERE id=?")
                        .bind(st6).bind(st6).bind(sc6).bind(paid6).bind(st6 - paid6).bind(admin_id).bind(&now).bind(s6).execute(&mut *tx).await?;

                    // ── Sale 7: fully paid ──
                    let s7 = insert_sale_dated(tx, "CUS-001", showroom_id, admin_id, "2026-09-02", &now).await?;
                    insert_sale_item(tx, s7, 1, "FS-031", 4, 2_700_000, 1_800_000).await?;
                    insert_sale_item(tx, s7, 2, "FS-029", 2, 1_400_000, 900_000).await?;
                    let st7: i64 = 4*2_700_000 + 2*1_400_000;
                    let sc7: i64 = 4*1_800_000 + 2*900_000;
                    sqlx::query("UPDATE sales SET subtotal_minor=?, total_minor=?, cost_minor=?, paid_minor=?, due_minor=0, status='confirmed', confirmed_by=?, confirmed_at=? WHERE id=?")
                        .bind(st7).bind(st7).bind(sc7).bind(st7).bind(admin_id).bind(&now).bind(s7).execute(&mut *tx).await?;

                    // ── Sale 8: partially paid (50%) ──
                    let s8 = insert_sale_dated(tx, "CUS-003", showroom_id, admin_id, "2026-09-05", &now).await?;
                    insert_sale_item(tx, s8, 1, "FS-011", 2, 6_800_000, 4_500_000).await?;
                    insert_sale_item(tx, s8, 2, "FS-013", 4, 1_200_000, 800_000).await?;
                    let st8: i64 = 2*6_800_000 + 4*1_200_000;
                    let sc8: i64 = 2*4_500_000 + 4*800_000;
                    let paid8: i64 = st8 / 2;
                    sqlx::query("UPDATE sales SET subtotal_minor=?, total_minor=?, cost_minor=?, paid_minor=?, due_minor=?, status='confirmed', confirmed_by=?, confirmed_at=? WHERE id=?")
                        .bind(st8).bind(st8).bind(sc8).bind(paid8).bind(st8 - paid8).bind(admin_id).bind(&now).bind(s8).execute(&mut *tx).await?;

                    // ── Sale 9: fully paid ──
                    let s9 = insert_sale_dated(tx, "CUS-005", showroom_id, admin_id, "2026-09-08", &now).await?;
                    insert_sale_item(tx, s9, 1, "FS-010", 2, 2_200_000, 1_500_000).await?;
                    insert_sale_item(tx, s9, 2, "FS-005", 1, 1_200_000, 800_000).await?;
                    let st9: i64 = 2*2_200_000 + 1_200_000;
                    let sc9: i64 = 2*1_500_000 + 800_000;
                    sqlx::query("UPDATE sales SET subtotal_minor=?, total_minor=?, cost_minor=?, paid_minor=?, due_minor=0, status='confirmed', confirmed_by=?, confirmed_at=? WHERE id=?")
                        .bind(st9).bind(st9).bind(sc9).bind(st9).bind(admin_id).bind(&now).bind(s9).execute(&mut *tx).await?;

                    // ── Sale 10: no payment yet (full due) ──
                    let s10 = insert_sale_dated(tx, "CUS-006", showroom_id, admin_id, "2026-09-10", &now).await?;
                    insert_sale_item(tx, s10, 1, "FS-035", 1, 7_000_000, 4_800_000).await?;
                    insert_sale_item(tx, s10, 2, "FS-034", 1, 2_300_000, 1_500_000).await?;
                    let st10: i64 = 7_000_000 + 2_300_000;
                    let sc10: i64 = 4_800_000 + 1_500_000;
                    sqlx::query("UPDATE sales SET subtotal_minor=?, total_minor=?, cost_minor=?, paid_minor=0, due_minor=?, status='confirmed', confirmed_by=?, confirmed_at=? WHERE id=?")
                        .bind(st10).bind(st10).bind(sc10).bind(st10).bind(admin_id).bind(&now).bind(s10).execute(&mut *tx).await?;
                }

                let sale_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sales")
                    .fetch_one(&mut *tx).await?;

                // ── Deduct stock for all confirmed sales ──
                let sale_items_all: Vec<(i64, i64)> = sqlx::query_as(
                    "SELECT si.product_id, si.quantity FROM sale_items si
                     JOIN sales s ON s.id = si.sale_id WHERE s.status = 'confirmed'",
                )
                .fetch_all(&mut *tx).await?;
                for (prod_id, qty) in &sale_items_all {
                    let existing_movement: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM stock_movements
                         WHERE product_id = ? AND location_id = ? AND movement_type = 'sale_issue'
                         AND reference_type = 'sale'",
                    )
                    .bind(prod_id).bind(showroom_id).fetch_one(&mut *tx).await?;
                    if existing_movement == 0 {
                        sqlx::query(
                            "UPDATE stock_balances SET on_hand = MAX(0, on_hand - ?)
                             WHERE product_id = ? AND location_id = ?",
                        )
                        .bind(qty).bind(prod_id).bind(showroom_id).execute(&mut *tx).await?;
                        sqlx::query(
                            "INSERT INTO stock_movements
                                 (product_id, location_id, movement_type, quantity_delta,
                                  reason, created_by, created_at)
                             VALUES (?, ?, 'sale_issue', ?, 'Demo sale issue', ?, ?)",
                        )
                        .bind(prod_id).bind(showroom_id).bind(-qty).bind(admin_id).bind(&now)
                        .execute(&mut *tx).await?;
                    }
                }

                // ══════════════════════════════════════════════════════════
                // CUSTOMER PAYMENTS  (against specific sales)
                // ══════════════════════════════════════════════════════════
                let existing_cp: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM customer_payments")
                    .fetch_one(&mut *tx).await?;

                if existing_cp == 0 {
                    let pay_method: i64 = sqlx::query_scalar(
                        "SELECT id FROM payment_methods WHERE code = 'cash'",
                    ).fetch_one(&mut *tx).await?;

                    // Payment for Sale 4 (CUS-003, 40% already paid)
                    let sale4_id: i64 = sqlx::query_scalar(
                        "SELECT id FROM sales WHERE status='confirmed' ORDER BY id LIMIT 1 OFFSET 3",
                    ).fetch_one(&mut *tx).await?;
                    let sale4_total: i64 = sqlx::query_scalar("SELECT total_minor FROM sales WHERE id=?")
                        .bind(sale4_id).fetch_one(&mut *tx).await?;
                    let pay4_amt = sale4_total * 30 / 100;
                    let cp4 = sqlx::query(
                        "INSERT INTO customer_payments
                             (customer_id, sale_id, payment_method_id, cash_account_id,
                              payment_date, amount_minor, status, created_by, created_at)
                         VALUES (?, ?, ?, ?, '2026-08-25', ?, 'posted', ?, ?)",
                    )
                    .bind(3i64).bind(sale4_id).bind(pay_method).bind(cash_acct)
                    .bind(pay4_amt).bind(admin_id).bind(&now)
                    .execute(&mut *tx).await?.last_insert_rowid();
                    sqlx::query("UPDATE customer_payments SET receipt_number = 'RCP-' || printf('%04d', id) WHERE id = ?")
                        .bind(cp4).execute(&mut *tx).await?;
                    sqlx::query("INSERT INTO customer_payment_allocations (payment_id, sale_id, amount_minor) VALUES (?, ?, ?)")
                        .bind(cp4).bind(sale4_id).bind(pay4_amt).execute(&mut *tx).await?;
                    sqlx::query("UPDATE sales SET paid_minor = paid_minor + ?, due_minor = MAX(0, due_minor - ?) WHERE id = ?")
                        .bind(pay4_amt).bind(pay4_amt).bind(sale4_id).execute(&mut *tx).await?;
                    sqlx::query("INSERT INTO cash_entries (cash_account_id, entry_type, amount_minor, reference_type, reference_id, created_by, created_at) VALUES (?, 'sale_payment', ?, 'sale', ?, ?, ?)")
                        .bind(cash_acct).bind(pay4_amt).bind(sale4_id).bind(admin_id).bind(&now).execute(&mut *tx).await?;

                    // Payment for Sale 6 (CUS-007, 60% already paid)
                    let sale6_id: i64 = sqlx::query_scalar(
                        "SELECT id FROM sales WHERE status='confirmed' ORDER BY id LIMIT 1 OFFSET 5",
                    ).fetch_one(&mut *tx).await?;
                    let sale6_due: i64 = sqlx::query_scalar("SELECT due_minor FROM sales WHERE id=?")
                        .bind(sale6_id).fetch_one(&mut *tx).await?;
                    let pay6_amt = sale6_due;
                    let cp6 = sqlx::query(
                        "INSERT INTO customer_payments
                             (customer_id, sale_id, payment_method_id, cash_account_id,
                              payment_date, amount_minor, status, created_by, created_at)
                         VALUES (?, ?, ?, ?, '2026-09-03', ?, 'posted', ?, ?)",
                    )
                    .bind(7i64).bind(sale6_id).bind(pay_method).bind(cash_acct)
                    .bind(pay6_amt).bind(admin_id).bind(&now)
                    .execute(&mut *tx).await?.last_insert_rowid();
                    sqlx::query("UPDATE customer_payments SET receipt_number = 'RCP-' || printf('%04d', id) WHERE id = ?")
                        .bind(cp6).execute(&mut *tx).await?;
                    sqlx::query("INSERT INTO customer_payment_allocations (payment_id, sale_id, amount_minor) VALUES (?, ?, ?)")
                        .bind(cp6).bind(sale6_id).bind(pay6_amt).execute(&mut *tx).await?;
                    sqlx::query("UPDATE sales SET paid_minor = paid_minor + ?, due_minor = 0 WHERE id = ?")
                        .bind(pay6_amt).bind(sale6_id).execute(&mut *tx).await?;
                    sqlx::query("INSERT INTO cash_entries (cash_account_id, entry_type, amount_minor, reference_type, reference_id, created_by, created_at) VALUES (?, 'sale_payment', ?, 'sale', ?, ?, ?)")
                        .bind(cash_acct).bind(pay6_amt).bind(sale6_id).bind(admin_id).bind(&now).execute(&mut *tx).await?;

                    // Payment for Sale 8 (CUS-003, 50% already paid)
                    let sale8_id: i64 = sqlx::query_scalar(
                        "SELECT id FROM sales WHERE status='confirmed' ORDER BY id LIMIT 1 OFFSET 7",
                    ).fetch_one(&mut *tx).await?;
                    let sale8_due: i64 = sqlx::query_scalar("SELECT due_minor FROM sales WHERE id=?")
                        .bind(sale8_id).fetch_one(&mut *tx).await?;
                    let pay8_amt = sale8_due / 2;
                    let cp8 = sqlx::query(
                        "INSERT INTO customer_payments
                             (customer_id, sale_id, payment_method_id, cash_account_id,
                              payment_date, amount_minor, status, created_by, created_at)
                         VALUES (?, ?, ?, ?, '2026-09-08', ?, 'posted', ?, ?)",
                    )
                    .bind(3i64).bind(sale8_id).bind(pay_method).bind(cash_acct)
                    .bind(pay8_amt).bind(admin_id).bind(&now)
                    .execute(&mut *tx).await?.last_insert_rowid();
                    sqlx::query("UPDATE customer_payments SET receipt_number = 'RCP-' || printf('%04d', id) WHERE id = ?")
                        .bind(cp8).execute(&mut *tx).await?;
                    sqlx::query("INSERT INTO customer_payment_allocations (payment_id, sale_id, amount_minor) VALUES (?, ?, ?)")
                        .bind(cp8).bind(sale8_id).bind(pay8_amt).execute(&mut *tx).await?;
                    sqlx::query("UPDATE sales SET paid_minor = paid_minor + ?, due_minor = MAX(0, due_minor - ?) WHERE id = ?")
                        .bind(pay8_amt).bind(pay8_amt).bind(sale8_id).execute(&mut *tx).await?;
                    sqlx::query("INSERT INTO cash_entries (cash_account_id, entry_type, amount_minor, reference_type, reference_id, created_by, created_at) VALUES (?, 'sale_payment', ?, 'sale', ?, ?, ?)")
                        .bind(cash_acct).bind(pay8_amt).bind(sale8_id).bind(admin_id).bind(&now).execute(&mut *tx).await?;
                }

                let cp_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM customer_payments")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // SUPPLIER PAYMENTS  (against purchases)
                // ══════════════════════════════════════════════════════════
                let existing_sp: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM supplier_payments")
                    .fetch_one(&mut *tx).await?;

                if existing_sp == 0 {
                    let pay_method: i64 = sqlx::query_scalar(
                        "SELECT id FROM payment_methods WHERE code = 'cash'",
                    ).fetch_one(&mut *tx).await?;

                    // Payment against Purchase 2 (INV-KWW-102, already has 60% paid)
                    let pur2_id: i64 = sqlx::query_scalar(
                        "SELECT id FROM purchases WHERE invoice_number = 'INV-KWW-102'",
                    ).fetch_one(&mut *tx).await?;
                    let pur2_due: i64 = sqlx::query_scalar("SELECT due_minor FROM purchases WHERE id=?")
                        .bind(pur2_id).fetch_one(&mut *tx).await?;
                    if pur2_due > 0 {
                        let sp2 = sqlx::query(
                            "INSERT INTO supplier_payments
                                 (supplier_id, payment_method_id, cash_account_id,
                                  payment_date, amount_minor, status, created_by, created_at)
                             VALUES (2, ?, ?, '2026-08-20', ?, 'posted', ?, ?)",
                        )
                        .bind(pay_method).bind(cash_acct).bind(pur2_due).bind(admin_id).bind(&now)
                        .execute(&mut *tx).await?.last_insert_rowid();
                        sqlx::query("UPDATE supplier_payments SET payment_number = 'SPAY-' || printf('%04d', id) WHERE id = ?")
                            .bind(sp2).execute(&mut *tx).await?;
                        sqlx::query("INSERT INTO supplier_payment_allocations (payment_id, purchase_id, amount_minor) VALUES (?, ?, ?)")
                            .bind(sp2).bind(pur2_id).bind(pur2_due).execute(&mut *tx).await?;
                        sqlx::query("UPDATE purchases SET paid_minor = paid_minor + ?, due_minor = 0 WHERE id = ?")
                            .bind(pur2_due).bind(pur2_id).execute(&mut *tx).await?;
                        sqlx::query("INSERT INTO cash_entries (cash_account_id, entry_type, amount_minor, reference_type, reference_id, created_by, created_at) VALUES (?, 'purchase_payment', ?, 'purchase', ?, ?, ?)")
                            .bind(cash_acct).bind(pur2_due).bind(pur2_id).bind(admin_id).bind(&now).execute(&mut *tx).await?;
                    }

                    // Payment against Purchase 3 (INV-SEH-205, already has 40% paid)
                    let pur3_id: i64 = sqlx::query_scalar(
                        "SELECT id FROM purchases WHERE invoice_number = 'INV-SEH-205'",
                    ).fetch_one(&mut *tx).await?;
                    let pur3_due: i64 = sqlx::query_scalar("SELECT due_minor FROM purchases WHERE id=?")
                        .bind(pur3_id).fetch_one(&mut *tx).await?;
                    if pur3_due > 0 {
                        let pay3_amt = pur3_due * 50 / 100;
                        let sp3 = sqlx::query(
                            "INSERT INTO supplier_payments
                                 (supplier_id, payment_method_id, cash_account_id,
                                  payment_date, amount_minor, status, created_by, created_at)
                             VALUES (4, ?, ?, '2026-09-05', ?, 'posted', ?, ?)",
                        )
                        .bind(pay_method).bind(cash_acct).bind(pay3_amt).bind(admin_id).bind(&now)
                        .execute(&mut *tx).await?.last_insert_rowid();
                        sqlx::query("UPDATE supplier_payments SET payment_number = 'SPAY-' || printf('%04d', id) WHERE id = ?")
                            .bind(sp3).execute(&mut *tx).await?;
                        sqlx::query("INSERT INTO supplier_payment_allocations (payment_id, purchase_id, amount_minor) VALUES (?, ?, ?)")
                            .bind(sp3).bind(pur3_id).bind(pay3_amt).execute(&mut *tx).await?;
                        sqlx::query("UPDATE purchases SET paid_minor = paid_minor + ?, due_minor = MAX(0, due_minor - ?) WHERE id = ?")
                            .bind(pay3_amt).bind(pay3_amt).bind(pur3_id).execute(&mut *tx).await?;
                        sqlx::query("INSERT INTO cash_entries (cash_account_id, entry_type, amount_minor, reference_type, reference_id, created_by, created_at) VALUES (?, 'purchase_payment', ?, 'purchase', ?, ?, ?)")
                            .bind(cash_acct).bind(pay3_amt).bind(pur3_id).bind(admin_id).bind(&now).execute(&mut *tx).await?;
                    }
                }

                let sp_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM supplier_payments")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // SALES RETURNS & CREDIT NOTES
                // ══════════════════════════════════════════════════════════
                let existing_ret: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sales_returns")
                    .fetch_one(&mut *tx).await?;

                if existing_ret == 0 {
                    // Return 1: credit note on Sale 3 (CUS-004, 1 TV stand returned, damaged)
                    let sale3_id: i64 = sqlx::query_scalar(
                        "SELECT id FROM sales WHERE status='confirmed' ORDER BY id LIMIT 1 OFFSET 2",
                    ).fetch_one(&mut *tx).await?;
                    let si_tv: i64 = sqlx::query_scalar(
                        "SELECT id FROM sale_items WHERE sale_id = ? AND article_number = 'FS-028' LIMIT 1",
                    ).bind(sale3_id).fetch_one(&mut *tx).await?;
                    let ret1 = sqlx::query(
                        "INSERT INTO sales_returns
                             (sale_id, customer_id, location_id, return_date, status, refund_type,
                              total_minor, total_refund_minor, credit_note_minor, notes,
                              posted_by, posted_at, created_by, created_at)
                         VALUES (?, 4, ?, '2026-08-15', 'posted', 'credit', 1_800_000, 1_800_000, 1_800_000,
                                 'Customer returned damaged TV stand — credit note issued', ?, ?, ?, ?)",
                    )
                    .bind(sale3_id).bind(showroom_id).bind(admin_id).bind(&now).bind(admin_id).bind(&now)
                    .execute(&mut *tx).await?.last_insert_rowid();
                    sqlx::query("UPDATE sales_returns SET return_number = 'RET-' || printf('%04d', id) WHERE id = ?")
                        .bind(ret1).execute(&mut *tx).await?;
                    sqlx::query(
                        "INSERT INTO sales_return_items
                             (return_id, sale_item_id, product_id, article_number, product_name,
                              quantity, unit_price_minor, unit_refund_minor, line_refund_minor, classification)
                         VALUES (?, ?, (SELECT id FROM products WHERE article_number='FS-028'),
                                 'FS-028', 'TV Stand with Drawers', 1, 1_800_000, 1_800_000, 1_800_000, 'damaged')",
                    )
                    .bind(ret1).bind(si_tv).execute(&mut *tx).await?;
                    sqlx::query(
                        "INSERT INTO credit_notes (customer_id, return_id, sale_id, amount_minor, status, notes, created_by, created_at)
                         VALUES (4, ?, ?, 1_800_000, 'open', 'Credit for damaged TV stand', ?, ?)",
                    )
                    .bind(ret1).bind(sale3_id).bind(admin_id).bind(&now).execute(&mut *tx).await?;
                    sqlx::query("UPDATE credit_notes SET credit_number = 'CN-' || printf('%04d', id) WHERE credit_number IS NULL")
                        .execute(&mut *tx).await?;

                    // Return 2: exchange on Sale 7 (CUS-001, swap armchair for different one)
                    let sale7_id: i64 = sqlx::query_scalar(
                        "SELECT id FROM sales WHERE status='confirmed' ORDER BY id LIMIT 1 OFFSET 6",
                    ).fetch_one(&mut *tx).await?;
                    let si_arm: i64 = sqlx::query_scalar(
                        "SELECT id FROM sale_items WHERE sale_id = ? AND article_number = 'FS-031' LIMIT 1",
                    ).bind(sale7_id).fetch_one(&mut *tx).await?;
                    let ret2 = sqlx::query(
                        "INSERT INTO sales_returns
                             (sale_id, customer_id, location_id, return_date, status, refund_type,
                              total_minor, total_refund_minor, cash_refund_minor, notes,
                              posted_by, posted_at, created_by, created_at)
                         VALUES (?, 1, ?, '2026-09-05', 'posted', 'exchange', 2_700_000, 2_700_000, 2_700_000,
                                 'Customer exchanged 1 armchair for a different colour', ?, ?, ?, ?)",
                    )
                    .bind(sale7_id).bind(showroom_id).bind(admin_id).bind(&now).bind(admin_id).bind(&now)
                    .execute(&mut *tx).await?.last_insert_rowid();
                    sqlx::query("UPDATE sales_returns SET return_number = 'RET-' || printf('%04d', id) WHERE id = ?")
                        .bind(ret2).execute(&mut *tx).await?;
                    sqlx::query(
                        "INSERT INTO sales_return_items
                             (return_id, sale_item_id, product_id, article_number, product_name,
                              quantity, unit_price_minor, unit_refund_minor, line_refund_minor, classification)
                         VALUES (?, ?, (SELECT id FROM products WHERE article_number='FS-031'),
                                 'FS-031', 'Single Seater Armchair', 1, 2_700_000, 2_700_000, 2_700_000, 'sellable')",
                    )
                    .bind(ret2).bind(si_arm).execute(&mut *tx).await?;
                    // Restock the returned item
                    sqlx::query(
                        "UPDATE stock_balances SET on_hand = on_hand + 1 WHERE product_id = (SELECT id FROM products WHERE article_number = 'FS-031') AND location_id = ?",
                    ).bind(showroom_id).execute(&mut *tx).await?;
                    sqlx::query(
                        "INSERT INTO stock_movements
                             (product_id, location_id, movement_type, quantity_delta, reason, created_by, created_at)
                         VALUES ((SELECT id FROM products WHERE article_number = 'FS-031'), ?, 'customer_return', 1, 'Exchange return restock', ?, ?)",
                    ).bind(showroom_id).bind(admin_id).bind(&now).execute(&mut *tx).await?;
                }

                let ret_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sales_returns")
                    .fetch_one(&mut *tx).await?;
                let cn_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM credit_notes")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // DELIVERIES
                // ══════════════════════════════════════════════════════════
                let existing_del: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM deliveries")
                    .fetch_one(&mut *tx).await?;

                if existing_del == 0 {
                    // Delivery 1: delivered (Sale 1, CUS-001)
                    let sale1_id: i64 = sqlx::query_scalar(
                        "SELECT id FROM sales WHERE status='confirmed' ORDER BY id LIMIT 1 OFFSET 0",
                    ).fetch_one(&mut *tx).await?;
                    let del1 = sqlx::query(
                        "INSERT INTO deliveries
                             (sale_id, customer_id, customer_name, location_id, status,
                              scheduled_at, address, contact_name, contact_phone,
                              delivery_charge_minor, delivered_at, delivered_by,
                              created_by, created_at, updated_at)
                         VALUES (?, 1, 'Ahmed Khan & Sons', ?, 'delivered',
                                 '2026-08-01', '23-C, Model Town, Lahore', 'Ahmed Khan', '0300-1234567',
                                 2_500_000, '2026-08-01', ?, ?, ?, ?)",
                    )
                    .bind(sale1_id).bind(showroom_id).bind(admin_id).bind(admin_id).bind(&now).bind(&now)
                    .execute(&mut *tx).await?.last_insert_rowid();
                    sqlx::query("UPDATE deliveries SET delivery_number = 'DEL-' || printf('%04d', id) WHERE id = ?")
                        .bind(del1).execute(&mut *tx).await?;
                    // Add delivery items from sale items
                    let sale1_items: Vec<(i64, i64, String, String, i64, i64, i64)> = sqlx::query_as(
                        "SELECT id, product_id, article_number, product_name, quantity, unit_price_minor, line_total_minor
                         FROM sale_items WHERE sale_id = ?",
                    ).bind(sale1_id).fetch_all(&mut *tx).await?;
                    for (si_id, pid, art, pname, qty, uprice, ltotal) in &sale1_items {
                        sqlx::query(
                            "INSERT INTO delivery_items (delivery_id, sale_item_id, product_id, article_number, product_name, quantity, unit_price_minor, line_total_minor)
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                        )
                        .bind(del1).bind(si_id).bind(pid).bind(art).bind(pname).bind(qty).bind(uprice).bind(ltotal)
                        .execute(&mut *tx).await?;
                    }

                    // Delivery 2: dispatched (Sale 7, CUS-001)
                    let sale7_id: i64 = sqlx::query_scalar(
                        "SELECT id FROM sales WHERE status='confirmed' ORDER BY id LIMIT 1 OFFSET 6",
                    ).fetch_one(&mut *tx).await?;
                    let del2 = sqlx::query(
                        "INSERT INTO deliveries
                             (sale_id, customer_id, customer_name, location_id, status,
                              scheduled_at, address, contact_name, contact_phone,
                              delivery_charge_minor, dispatched_at, dispatched_by,
                              created_by, created_at, updated_at)
                         VALUES (?, 1, 'Ahmed Khan & Sons', ?, 'dispatched',
                                 '2026-09-09', '23-C, Model Town, Lahore', 'Ahmed Khan', '0300-1234567',
                                 3_000_000, '2026-09-09', ?, ?, ?, ?)",
                    )
                    .bind(sale7_id).bind(showroom_id).bind(admin_id).bind(admin_id).bind(&now).bind(&now)
                    .execute(&mut *tx).await?.last_insert_rowid();
                    sqlx::query("UPDATE deliveries SET delivery_number = 'DEL-' || printf('%04d', id) WHERE id = ?")
                        .bind(del2).execute(&mut *tx).await?;
                    let sale7_items: Vec<(i64, i64, String, String, i64, i64, i64)> = sqlx::query_as(
                        "SELECT id, product_id, article_number, product_name, quantity, unit_price_minor, line_total_minor
                         FROM sale_items WHERE sale_id = ?",
                    ).bind(sale7_id).fetch_all(&mut *tx).await?;
                    for (si_id, pid, art, pname, qty, uprice, ltotal) in &sale7_items {
                        sqlx::query(
                            "INSERT INTO delivery_items (delivery_id, sale_item_id, product_id, article_number, product_name, quantity, unit_price_minor, line_total_minor)
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                        )
                        .bind(del2).bind(si_id).bind(pid).bind(art).bind(pname).bind(qty).bind(uprice).bind(ltotal)
                        .execute(&mut *tx).await?;
                    }

                    // Delivery 3: pending (Sale 9, CUS-005)
                    let sale9_id: i64 = sqlx::query_scalar(
                        "SELECT id FROM sales WHERE status='confirmed' ORDER BY id LIMIT 1 OFFSET 8",
                    ).fetch_one(&mut *tx).await?;
                    let del3 = sqlx::query(
                        "INSERT INTO deliveries
                             (sale_id, customer_id, customer_name, location_id, status,
                              scheduled_at, address, contact_name, contact_phone,
                              delivery_charge_minor, created_by, created_at, updated_at)
                         VALUES (?, 5, 'Habib Residence', ?, 'pending',
                                 '2026-09-15', '14-A, Wapda Town, Lahore', 'Habib', '0312-8889900',
                                 1_500_000, ?, ?, ?)",
                    )
                    .bind(sale9_id).bind(showroom_id).bind(admin_id).bind(&now).bind(&now)
                    .execute(&mut *tx).await?.last_insert_rowid();
                    sqlx::query("UPDATE deliveries SET delivery_number = 'DEL-' || printf('%04d', id) WHERE id = ?")
                        .bind(del3).execute(&mut *tx).await?;
                    let sale9_items: Vec<(i64, i64, String, String, i64, i64, i64)> = sqlx::query_as(
                        "SELECT id, product_id, article_number, product_name, quantity, unit_price_minor, line_total_minor
                         FROM sale_items WHERE sale_id = ?",
                    ).bind(sale9_id).fetch_all(&mut *tx).await?;
                    for (si_id, pid, art, pname, qty, uprice, ltotal) in &sale9_items {
                        sqlx::query(
                            "INSERT INTO delivery_items (delivery_id, sale_item_id, product_id, article_number, product_name, quantity, unit_price_minor, line_total_minor)
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                        )
                        .bind(del3).bind(si_id).bind(pid).bind(art).bind(pname).bind(qty).bind(uprice).bind(ltotal)
                        .execute(&mut *tx).await?;
                    }
                }

                let del_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM deliveries")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // DAMAGE RECORDS
                // ══════════════════════════════════════════════════════════
                let existing_dmg: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM damage_records")
                    .fetch_one(&mut *tx).await?;

                if existing_dmg == 0 {
                    // Damaged nightstand during delivery (Sale 5 items, but from stock)
                    sqlx::query(
                        "INSERT INTO damage_records
                             (product_id, location_id, quantity, damage_date, source, reason,
                              estimated_loss_minor, status, created_by, created_at, updated_at)
                         VALUES ((SELECT id FROM products WHERE article_number = 'FS-024'),
                                 ?, 1, '2026-08-22', 'in_hand', 'Scratched during warehouse move',
                                 1_400_000, 'open', ?, ?, ?)",
                    )
                    .bind(showroom_id).bind(admin_id).bind(&now).bind(&now)
                    .execute(&mut *tx).await?;
                    sqlx::query("UPDATE damage_records SET damage_number = 'DMG-' || printf('%04d', id) WHERE damage_number IS NULL")
                        .execute(&mut *tx).await?;
                }

                let dmg_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM damage_records")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // EXPENSES  (12 total, spread across dates)
                // ══════════════════════════════════════════════════════════
                let existing_exp: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM expenses")
                    .fetch_one(&mut *tx).await?;

                if existing_exp == 0 {
                    let rent_cat: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'rent'")
                        .fetch_one(&mut *tx).await?;
                    let salary_cat: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'salaries'")
                        .fetch_one(&mut *tx).await?;
                    let elec_cat: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'electricity'")
                        .fetch_one(&mut *tx).await?;
                    let transport_cat: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'transport'")
                        .fetch_one(&mut *tx).await?;
                    let internet_cat: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'internet'")
                        .fetch_one(&mut *tx).await?;
                    let office_cat: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'office'")
                        .fetch_one(&mut *tx).await?;
                    let marketing_cat: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'marketing'")
                        .fetch_one(&mut *tx).await?;
                    let food_cat: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'food'")
                        .fetch_one(&mut *tx).await?;
                    let repairs_cat: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'repairs'")
                        .fetch_one(&mut *tx).await?;
                    let misc_cat: i64 = sqlx::query_scalar("SELECT id FROM expense_categories WHERE code = 'misc'")
                        .fetch_one(&mut *tx).await?;

                    let expenses_data: &[(i64, i64, &str, &str)] = &[
                        (rent_cat,     150_000_000, "2026-08-01", "August shop rent"),
                        (salary_cat,    80_000_000, "2026-08-05", "Staff salaries — August"),
                        (elec_cat,      35_000_000, "2026-08-10", "Electricity bill — August"),
                        (transport_cat, 12_000_000, "2026-08-15", "Delivery fuel costs"),
                        (internet_cat,   5_000_000, "2026-08-18", "Broadband internet — August"),
                        (office_cat,     8_000_000, "2026-08-22", "Office stationery & supplies"),
                        (marketing_cat, 15_000_000, "2026-08-28", "Facebook & Instagram ads"),
                        (rent_cat,     150_000_000, "2026-09-01", "September shop rent"),
                        (salary_cat,    80_000_000, "2026-09-05", "Staff salaries — September"),
                        (food_cat,       6_000_000, "2026-09-06", "Staff lunch — sales event"),
                        (repairs_cat,    4_000_000, "2026-09-08", "Showroom AC repair"),
                        (misc_cat,       3_000_000, "2026-09-10", "Miscellaneous sundries"),
                    ];

                    for &(cat, amount, date, desc) in expenses_data {
                        sqlx::query(
                            "INSERT INTO expenses
                                 (expense_number, category_id, amount_minor, expense_date,
                                  cash_account_id, description, status, created_by, created_at,
                                  posted_by, posted_at)
                             VALUES (NULL, ?, ?, ?, ?, ?, 'posted', ?, ?, ?, ?)",
                        )
                        .bind(cat).bind(amount).bind(date).bind(cash_acct).bind(desc)
                        .bind(admin_id).bind(&now).bind(admin_id).bind(&now)
                        .execute(&mut *tx).await?;
                    }
                    sqlx::query(
                        "UPDATE expenses SET expense_number = 'EXP-' || printf('%04d', id) WHERE expense_number IS NULL",
                    ).execute(&mut *tx).await?;
                }

                let exp_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM expenses")
                    .fetch_one(&mut *tx).await?;

                // ══════════════════════════════════════════════════════════
                // OWNER CAPITAL  (idempotent)
                // ══════════════════════════════════════════════════════════
                let existing_own: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM owner_transactions")
                    .fetch_one(&mut *tx).await?;
                if existing_own == 0 {
                    sqlx::query(
                        "INSERT INTO owner_transactions
                             (transaction_number, kind, amount_minor, transaction_date,
                              cash_account_id, notes, created_by, created_at)
                         VALUES (NULL, 'capital_in', 500_000_000, '2026-07-01', ?,
                                 'Initial capital injection', ?, ?)",
                    )
                    .bind(cash_acct).bind(admin_id).bind(&now).execute(&mut *tx).await?;
                    sqlx::query(
                        "UPDATE owner_transactions SET transaction_number = 'OWN-' || printf('%04d', id) WHERE transaction_number IS NULL",
                    ).execute(&mut *tx).await?;
                }

                // ══════════════════════════════════════════════════════════
                // SUPPLIER LEDGER ENTRIES  (mirror of purchases & payments)
                // ══════════════════════════════════════════════════════════
                let existing_sle: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM supplier_ledger_entries")
                    .fetch_one(&mut *tx).await?;
                if existing_sle == 0 {
                    // Fetch all posted purchases
                    let all_purchases: Vec<(i64, i64, i64)> = sqlx::query_as(
                        "SELECT id, supplier_id, total_minor FROM purchases WHERE status = 'posted'",
                    ).fetch_all(&mut *tx).await?;
                    for (pid, sid, total) in &all_purchases {
                        sqlx::query(
                            "INSERT INTO supplier_ledger_entries
                                 (supplier_id, entry_type, document_type, document_id,
                                  amount_minor, balance_after_minor, notes, created_by, created_at)
                             VALUES (?, 'invoice', 'purchase', ?, ?, ?, 'Purchase invoice', ?, ?)",
                        )
                        .bind(sid).bind(pid).bind(total).bind(total).bind(admin_id).bind(&now)
                        .execute(&mut *tx).await?;
                    }
                    // Fetch all supplier payments
                    let all_spay: Vec<(i64, i64, i64)> = sqlx::query_as(
                        "SELECT id, supplier_id, amount_minor FROM supplier_payments WHERE status = 'posted'",
                    ).fetch_all(&mut *tx).await?;
                    for (spid, sid, amt) in &all_spay {
                        sqlx::query(
                            "INSERT INTO supplier_ledger_entries
                                 (supplier_id, entry_type, document_type, document_id,
                                  amount_minor, balance_after_minor, notes, created_by, created_at)
                             VALUES (?, 'payment', 'supplier_payment', ?, -?, 0, 'Payment received', ?, ?)",
                        )
                        .bind(sid).bind(spid).bind(amt).bind(admin_id).bind(&now)
                        .execute(&mut *tx).await?;
                    }
                }

                // ══════════════════════════════════════════════════════════
                // CUSTOMER LEDGER ENTRIES  (mirror of sales & payments)
                // ══════════════════════════════════════════════════════════
                let existing_cle: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM customer_ledger_entries")
                    .fetch_one(&mut *tx).await?;
                if existing_cle == 0 {
                    let all_confirmed_sales: Vec<(i64, Option<i64>, i64)> = sqlx::query_as(
                        "SELECT id, customer_id, total_minor FROM sales WHERE status = 'confirmed' AND customer_id IS NOT NULL",
                    ).fetch_all(&mut *tx).await?;
                    for (sid, cid, total) in &all_confirmed_sales {
                        if let Some(cid) = cid {
                            sqlx::query(
                                "INSERT INTO customer_ledger_entries
                                     (customer_id, entry_type, document_type, document_id,
                                      amount_minor, balance_after_minor, notes, created_by, created_at)
                                 VALUES (?, 'sale', 'sale', ?, ?, ?, 'Sale invoice', ?, ?)",
                            )
                            .bind(cid).bind(sid).bind(total).bind(total).bind(admin_id).bind(&now)
                            .execute(&mut *tx).await?;
                        }
                    }
                    let all_cpay: Vec<(i64, i64, i64)> = sqlx::query_as(
                        "SELECT id, customer_id, amount_minor FROM customer_payments WHERE status = 'posted'",
                    ).fetch_all(&mut *tx).await?;
                    for (crid, cid, amt) in &all_cpay {
                        sqlx::query(
                            "INSERT INTO customer_ledger_entries
                                 (customer_id, entry_type, document_type, document_id,
                                  amount_minor, balance_after_minor, notes, created_by, created_at)
                                 VALUES (?, 'payment', 'customer_receipt', ?, -?, 0, 'Payment received', ?, ?)",
                        )
                        .bind(cid).bind(crid).bind(amt).bind(admin_id).bind(&now)
                        .execute(&mut *tx).await?;
                    }
                }

                Ok(SeedResult {
                    categories: cat_count,
                    product_types: pt_count,
                    products: prod_count,
                    suppliers: sup_count,
                    customers: cust_count,
                    purchases: pur_count,
                    sales: sale_count,
                    expenses: exp_count,
                    bundles: bundle_count,
                    customer_payments: cp_count,
                    supplier_payments: sp_count,
                    sales_returns: ret_count,
                    credit_notes: cn_count,
                    deliveries: del_count,
                    damage_records: dmg_count,
                })
            })
        })
        .await?;

    Ok(result)
}

// ── helpers ────────────────────────────────────────────────────────────

async fn insert_category(
    tx: &mut sqlx::SqliteConnection,
    name: &str,
    sort: i32,
    now: &str,
) -> Result<i64, AppError> {
    let id = sqlx::query(
        "INSERT INTO categories (name, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?)",
    )
    .bind(name).bind(sort).bind(now).bind(now)
    .execute(&mut *tx).await?.last_insert_rowid();
    Ok(id)
}

async fn insert_category_child(
    tx: &mut sqlx::SqliteConnection,
    name: &str,
    parent_id: i64,
    sort: i32,
    now: &str,
) -> Result<i64, AppError> {
    let id = sqlx::query(
        "INSERT INTO categories (name, parent_id, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(name).bind(parent_id).bind(sort).bind(now).bind(now)
    .execute(&mut *tx).await?.last_insert_rowid();
    Ok(id)
}

async fn insert_pt(
    tx: &mut sqlx::SqliteConnection,
    category_id: i64,
    name: &str,
    now: &str,
) -> Result<i64, AppError> {
    let id = sqlx::query(
        "INSERT OR IGNORE INTO product_types (category_id, name, created_at, updated_at) VALUES (?, ?, ?, ?)",
    )
    .bind(category_id).bind(name).bind(now).bind(now)
    .execute(&mut *tx).await?.last_insert_rowid();
    Ok(id)
}

async fn insert_purchase_dated(
    tx: &mut sqlx::SqliteConnection,
    supplier_code: &str,
    location_id: i64,
    admin_id: i64,
    invoice_number: &str,
    date: &str,
    now: &str,
) -> Result<i64, AppError> {
    let supplier_name: String = sqlx::query_scalar("SELECT name FROM suppliers WHERE code = ?")
        .bind(supplier_code).fetch_one(&mut *tx).await?;
    let id = sqlx::query(
        "INSERT INTO purchases
             (supplier_id, supplier_name, location_id, invoice_number, invoice_date,
              purchase_date, status, created_by, created_at, updated_at)
         VALUES ((SELECT id FROM suppliers WHERE code = ?), ?, ?, ?, ?, ?, 'draft', ?, ?, ?)",
    )
    .bind(supplier_code).bind(&supplier_name).bind(location_id)
    .bind(invoice_number).bind(date).bind(date)
    .bind(admin_id).bind(now).bind(now)
    .execute(&mut *tx).await?.last_insert_rowid();
    Ok(id)
}

async fn insert_purchase_item(
    tx: &mut sqlx::SqliteConnection,
    purchase_id: i64,
    article: &str,
    qty: i64,
    cost_minor: i64,
) -> Result<(), AppError> {
    let (prod_id, prod_name): (i64, String) =
        sqlx::query_as("SELECT id, name FROM products WHERE article_number = ?")
            .bind(article).fetch_one(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO purchase_items
             (purchase_id, product_id, article_number, product_name, quantity,
              unit_cost_minor, line_total_minor)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(purchase_id).bind(prod_id).bind(article).bind(&prod_name)
    .bind(qty).bind(cost_minor).bind(qty * cost_minor)
    .execute(&mut *tx).await?;
    Ok(())
}

async fn insert_sale_dated(
    tx: &mut sqlx::SqliteConnection,
    customer_code: &str,
    location_id: i64,
    admin_id: i64,
    sale_date: &str,
    now: &str,
) -> Result<i64, AppError> {
    let customer_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM customers WHERE code = ?")
            .bind(customer_code).fetch_one(&mut *tx).await?;
    let id = sqlx::query(
        "INSERT INTO sales
             (customer_id, customer_name, location_id, sale_date, status, created_by,
              created_at, updated_at)
         VALUES ((SELECT id FROM customers WHERE code = ?), ?, ?, ?, 'draft', ?, ?, ?)",
    )
    .bind(customer_code).bind(&customer_name).bind(location_id).bind(sale_date)
    .bind(admin_id).bind(now).bind(now)
    .execute(&mut *tx).await?.last_insert_rowid();
    Ok(id)
}

async fn insert_sale_item(
    tx: &mut sqlx::SqliteConnection,
    sale_id: i64,
    sort_order: i32,
    article: &str,
    qty: i64,
    price_minor: i64,
    cost_minor: i64,
) -> Result<(), AppError> {
    let (prod_id, prod_name): (i64, String) =
        sqlx::query_as("SELECT id, name FROM products WHERE article_number = ?")
            .bind(article).fetch_one(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO sale_items
             (sale_id, sort_order, product_id, article_number, product_name, quantity,
              unit_price_minor, line_total_minor, unit_cost_minor, line_cost_minor)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(sale_id).bind(sort_order).bind(prod_id).bind(article).bind(&prod_name)
    .bind(qty).bind(price_minor).bind(qty * price_minor).bind(cost_minor).bind(qty * cost_minor)
    .execute(&mut *tx).await?;
    Ok(())
}
