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

    let existing_products: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
        .fetch_one(&state.pool)
        .await?;
    if existing_products > 0 {
        return Err(AppError::Conflict(
            "demo data cannot be seeded: products already exist".into(),
        ));
    }

    let result = state
        .write_coordinator
        .execute(&state.pool, move |tx| {
            Box::pin(async move {
                // ── Categories ──────────────────────────────────────────
                let cat_living = insert_category(tx, "Living Room", 1, &now).await?;
                let cat_sofas =
                    insert_category_child(tx, "Sofas", cat_living, 1, &now).await?;
                let cat_tables =
                    insert_category_child(tx, "Coffee Tables", cat_living, 2, &now).await?;
                let cat_tv =
                    insert_category_child(tx, "TV Units", cat_living, 3, &now).await?;

                let cat_bedroom = insert_category(tx, "Bedroom", 2, &now).await?;
                let cat_beds =
                    insert_category_child(tx, "Beds", cat_bedroom, 1, &now).await?;
                let cat_wardrobes =
                    insert_category_child(tx, "Wardrobes", cat_bedroom, 2, &now).await?;
                let cat_nightstands =
                    insert_category_child(tx, "Nightstands", cat_bedroom, 3, &now).await?;

                let cat_dining = insert_category(tx, "Dining", 3, &now).await?;
                let cat_dtables =
                    insert_category_child(tx, "Dining Tables", cat_dining, 1, &now).await?;
                let cat_dchairs =
                    insert_category_child(tx, "Dining Chairs", cat_dining, 2, &now).await?;

                let cat_office = insert_category(tx, "Office", 4, &now).await?;
                let cat_odesks =
                    insert_category_child(tx, "Office Desks", cat_office, 1, &now).await?;
                let cat_ochairs =
                    insert_category_child(tx, "Office Chairs", cat_office, 2, &now).await?;

                let cat_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM categories")
                    .fetch_one(&mut *tx)
                    .await?;

                // ── Product Types ───────────────────────────────────────
                let pt_3seater = insert_pt(tx, cat_sofas, "3-Seater Sofa", &now).await?;
                let pt_lshape = insert_pt(tx, cat_sofas, "L-Shape Sofa", &now).await?;
                let pt_recliner = insert_pt(tx, cat_sofas, "Recliner Sofa", &now).await?;
                let pt_center = insert_pt(tx, cat_tables, "Center Table", &now).await?;
                let pt_nesting = insert_pt(tx, cat_tables, "Nesting Tables", &now).await?;
                let pt_tvcab = insert_pt(tx, cat_tv, "TV Cabinet", &now).await?;
                let pt_tvwall = insert_pt(tx, cat_tv, "Wall-Mounted Unit", &now).await?;
                let pt_king = insert_pt(tx, cat_beds, "King Size Bed", &now).await?;
                let pt_double = insert_pt(tx, cat_beds, "Double Bed", &now).await?;
                let pt_single = insert_pt(tx, cat_beds, "Single Bed", &now).await?;
                let pt_4door = insert_pt(tx, cat_wardrobes, "4-Door Wardrobe", &now).await?;
                let pt_slide =
                    insert_pt(tx, cat_wardrobes, "Sliding Door Wardrobe", &now).await?;
                let pt_night =
                    insert_pt(tx, cat_nightstands, "2-Drawer Nightstand", &now).await?;
                let pt_6dining =
                    insert_pt(tx, cat_dtables, "6-Seater Dining Set", &now).await?;
                let pt_4dining =
                    insert_pt(tx, cat_dtables, "4-Seater Dining Set", &now).await?;
                let pt_dchair =
                    insert_pt(tx, cat_dchairs, "Dining Chair Set (4)", &now).await?;
                let pt_edesk = insert_pt(tx, cat_odesks, "Executive Desk", &now).await?;
                let pt_work = insert_pt(tx, cat_odesks, "Workstation", &now).await?;
                let pt_echair = insert_pt(tx, cat_ochairs, "Executive Chair", &now).await?;
                let pt_tchair = insert_pt(tx, cat_ochairs, "Task Chair", &now).await?;

                let pt_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_types")
                    .fetch_one(&mut *tx)
                    .await?;

                // ── Units ──────────────────────────────────────────────
                let unit_pcs: i64 =
                    sqlx::query_scalar("SELECT id FROM units WHERE code = 'pcs'")
                        .fetch_one(&mut *tx)
                        .await?;

                // ── Products ───────────────────────────────────────────
                // All prices in minor units (paisa): PKR * 100
                // e.g. 45,000 PKR = 4_500_000 paisa
                #[allow(clippy::type_complexity)]
                let products: &[(i64, i64, &str, &str, &str, i64, i64, i64)] = &[
                    (cat_sofas, pt_3seater, "FS-001", "Royal 3-Seater Sofa", "Sheesham Wood", 4_500_000, 6_500_000, 3),
                    (cat_sofas, pt_lshape, "FS-002", "Modern L-Shape Sofa", "Sheesham Wood", 6_500_000, 9_500_000, 2),
                    (cat_sofas, pt_recliner, "FS-003", "Comfort Recliner Sofa", "Pine Wood", 5_500_000, 8_000_000, 2),
                    (cat_tables, pt_center, "FS-004", "Marble Center Table", "Marble & Steel", 1_200_000, 1_800_000, 5),
                    (cat_tables, pt_nesting, "FS-005", "Wood Nesting Tables", "Oak Wood", 800_000, 1_200_000, 4),
                    (cat_tv, pt_tvcab, "FS-006", "Elegant TV Cabinet", "MDF & Glass", 1_800_000, 2_800_000, 3),
                    (cat_tv, pt_tvwall, "FS-007", "Wall-Mounted TV Unit", "MDF", 1_500_000, 2_200_000, 4),
                    (cat_beds, pt_king, "FS-008", "King Size Bed Frame", "Sheesham Wood", 3_500_000, 5_200_000, 2),
                    (cat_beds, pt_double, "FS-009", "Classic Double Bed", "Sheesham Wood", 2_500_000, 3_800_000, 3),
                    (cat_beds, pt_single, "FS-010", "Single Bed Frame", "Pine Wood", 1_500_000, 2_200_000, 4),
                    (cat_wardrobes, pt_4door, "FS-011", "4-Door Wardrobe", "Sheesham Wood", 4_500_000, 6_800_000, 2),
                    (cat_wardrobes, pt_slide, "FS-012", "Sliding Door Wardrobe", "MDF & Mirror", 5_500_000, 8_200_000, 1),
                    (cat_nightstands, pt_night, "FS-013", "2-Drawer Nightstand", "Sheesham Wood", 800_000, 1_200_000, 6),
                    (cat_dtables, pt_6dining, "FS-014", "6-Seater Dining Set", "Sheesham Wood", 5_500_000, 8_200_000, 2),
                    (cat_dtables, pt_4dining, "FS-015", "4-Seater Dining Set", "Oak Wood", 3_500_000, 5_200_000, 3),
                    (cat_dchairs, pt_dchair, "FS-016", "Dining Chair Set of 4", "Sheesham Wood", 1_200_000, 1_800_000, 5),
                    (cat_odesks, pt_edesk, "FS-017", "Executive Office Desk", "Sheesham Wood", 2_500_000, 3_800_000, 2),
                    (cat_odesks, pt_work, "FS-018", "L-Shaped Workstation", "MDF & Steel", 1_800_000, 2_800_000, 3),
                    (cat_ochairs, pt_echair, "FS-019", "Executive Office Chair", "Leather & Chrome", 1_200_000, 1_900_000, 4),
                    (cat_ochairs, pt_tchair, "FS-020", "Ergonomic Task Chair", "Mesh & Nylon", 800_000, 1_200_000, 6),
                ];

                for &(cat_id, pt_id, article, name, material, cost, sale, min_stock) in products {
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
                    .bind(pt_id)
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
                    .fetch_one(&mut *tx)
                    .await?;

                // ── Suppliers ──────────────────────────────────────────
                let suppliers: &[(&str, &str, &str, &str, &str, i64)] = &[
                    ("SUP-001", "Al-Hamd Furniture Industries", "042-35123456", "info@alhamd.pk", "Sialkot Road, Lahore", 5_000_000),
                    ("SUP-002", "Karachi Wood Works", "021-34567890", "sales@karachiwood.pk", "SITE Area, Karachi", 3_000_000),
                    ("SUP-003", "Pak Ceramic & Marble", "041-87654321", "orders@pakmarble.pk", "D-Ground, Faisalabad", 2_000_000),
                    ("SUP-004", "Sheesham Export Hub", "052-4567890", "export@sheesham.pk", "Bhai Wala, Gujrat", 4_000_000),
                ];

                for &(code, name, phone, email, address, opening) in suppliers {
                    sqlx::query(
                        "INSERT OR IGNORE INTO suppliers
                             (code, name, phone, email, address, opening_balance_minor,
                              is_active, created_by, created_at, updated_at)
                         VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?)",
                    )
                    .bind(code)
                    .bind(name)
                    .bind(phone)
                    .bind(email)
                    .bind(address)
                    .bind(opening)
                    .bind(admin_id)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }

                let sup_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM suppliers")
                    .fetch_one(&mut *tx)
                    .await?;

                // ── Customers ──────────────────────────────────────────
                let customers: &[(&str, &str, &str, &str, i64)] = &[
                    ("CUS-001", "Ahmed Khan & Sons", "0300-1234567", "ahmed.khan@gmail.com", 5_000_000),
                    ("CUS-002", "Fatima Enterprises", "0321-9876543", "fatima.ent@gmail.com", 3_000_000),
                    ("CUS-003", "Muhammad Ali Builders", "0333-5551234", "ali.builders@gmail.com", 10_000_000),
                    ("CUS-004", "Sara Interior Design Studio", "0345-2223344", "sara.interior@gmail.com", 2_000_000),
                    ("CUS-005", "Habib Residence", "0312-8889900", "habib.res@gmail.com", 1_500_000),
                ];

                for &(code, name, phone, email, credit) in customers {
                    sqlx::query(
                        "INSERT OR IGNORE INTO customers
                             (code, name, phone, email, credit_limit_minor,
                              is_active, created_by, created_at, updated_at)
                         VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?)",
                    )
                    .bind(code)
                    .bind(name)
                    .bind(phone)
                    .bind(email)
                    .bind(credit)
                    .bind(admin_id)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }

                let cust_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM customers")
                    .fetch_one(&mut *tx)
                    .await?;

                // ── Purchases (3 posted) ──────────────────────────────
                let pur1_id =
                    insert_purchase(tx, "SUP-001", showroom_id, admin_id, "INV-ALH-001", &now)
                        .await?;
                insert_purchase_item(tx, pur1_id, "FS-001", 5, 4_500_000).await?;
                insert_purchase_item(tx, pur1_id, "FS-002", 3, 6_500_000).await?;
                insert_purchase_item(tx, pur1_id, "FS-004", 10, 1_200_000).await?;
                let pur1_total: i64 =
                    5 * 4_500_000 + 3 * 6_500_000 + 10 * 1_200_000;
                sqlx::query(
                    "UPDATE purchases SET total_minor = ?, status = 'posted', posted_by = ?, posted_at = ? WHERE id = ?",
                )
                .bind(pur1_total)
                .bind(admin_id)
                .bind(&now)
                .bind(pur1_id)
                .execute(&mut *tx)
                .await?;

                let pur2_id =
                    insert_purchase(tx, "SUP-002", store_id, admin_id, "INV-KWW-102", &now)
                        .await?;
                insert_purchase_item(tx, pur2_id, "FS-008", 4, 3_500_000).await?;
                insert_purchase_item(tx, pur2_id, "FS-009", 6, 2_500_000).await?;
                insert_purchase_item(tx, pur2_id, "FS-011", 2, 4_500_000).await?;
                let pur2_total: i64 =
                    4 * 3_500_000 + 6 * 2_500_000 + 2 * 4_500_000;
                sqlx::query(
                    "UPDATE purchases SET total_minor = ?, status = 'posted', posted_by = ?, posted_at = ? WHERE id = ?",
                )
                .bind(pur2_total)
                .bind(admin_id)
                .bind(&now)
                .bind(pur2_id)
                .execute(&mut *tx)
                .await?;

                let pur3_id =
                    insert_purchase(tx, "SUP-004", showroom_id, admin_id, "INV-SEH-205", &now)
                        .await?;
                insert_purchase_item(tx, pur3_id, "FS-014", 3, 5_500_000).await?;
                insert_purchase_item(tx, pur3_id, "FS-016", 8, 1_200_000).await?;
                insert_purchase_item(tx, pur3_id, "FS-019", 5, 1_200_000).await?;
                let pur3_total: i64 =
                    3 * 5_500_000 + 8 * 1_200_000 + 5 * 1_200_000;
                sqlx::query(
                    "UPDATE purchases SET total_minor = ?, status = 'posted', posted_by = ?, posted_at = ? WHERE id = ?",
                )
                .bind(pur3_total)
                .bind(admin_id)
                .bind(&now)
                .bind(pur3_id)
                .execute(&mut *tx)
                .await?;

                let pur_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM purchases")
                    .fetch_one(&mut *tx)
                    .await?;

                // ── Stock Opening Balances ─────────────────────────────
                let stock_products: &[(i64, i64)] = &[
                    (1, 12),
                    (2, 8),
                    (3, 5),
                    (4, 20),
                    (5, 15),
                    (8, 6),
                    (9, 10),
                    (11, 4),
                    (14, 5),
                    (16, 12),
                    (19, 8),
                ];
                for &(prod_id, qty) in stock_products {
                    sqlx::query(
                        "INSERT OR IGNORE INTO stock_balances (product_id, location_id, on_hand, reserved, damaged)
                         VALUES (?, ?, ?, 0, 0)",
                    )
                    .bind(prod_id)
                    .bind(showroom_id)
                    .bind(qty)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "INSERT INTO stock_movements
                             (product_id, location_id, movement_type, quantity_delta, unit_cost_minor,
                              reason, created_by, created_at)
                         VALUES (?, ?, 'opening', ?, 0, 'Demo opening stock', ?, ?)",
                    )
                    .bind(prod_id)
                    .bind(showroom_id)
                    .bind(qty)
                    .bind(admin_id)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }

                // ── Sales (3 posted) ──────────────────────────────────
                let sale1_id =
                    insert_sale(tx, "CUS-001", showroom_id, admin_id, "2026-08-15", &now)
                        .await?;
                insert_sale_item(tx, sale1_id, 1, "FS-001", 1, 6_500_000, 4_500_000).await?;
                insert_sale_item(tx, sale1_id, 4, "FS-004", 2, 1_800_000, 1_200_000).await?;
                let sale1_total: i64 = 6_500_000 + 2 * 1_800_000;
                let sale1_cost: i64 = 4_500_000 + 2 * 1_200_000;
                sqlx::query(
                    "UPDATE sales SET subtotal_minor = ?, total_minor = ?, cost_minor = ?,
                      paid_minor = ?, due_minor = 0, status = 'confirmed',
                      confirmed_by = ?, confirmed_at = ? WHERE id = ?",
                )
                .bind(sale1_total)
                .bind(sale1_total)
                .bind(sale1_cost)
                .bind(sale1_total)
                .bind(admin_id)
                .bind(&now)
                .bind(sale1_id)
                .execute(&mut *tx)
                .await?;

                let sale2_id =
                    insert_sale(tx, "CUS-002", showroom_id, admin_id, "2026-08-20", &now)
                        .await?;
                insert_sale_item(tx, sale2_id, 1, "FS-008", 1, 5_200_000, 3_500_000).await?;
                insert_sale_item(tx, sale2_id, 2, "FS-013", 2, 1_200_000, 800_000).await?;
                let sale2_total: i64 = 5_200_000 + 2 * 1_200_000;
                let sale2_cost: i64 = 3_500_000 + 2 * 800_000;
                let sale2_paid: i64 = 4_000_000;
                sqlx::query(
                    "UPDATE sales SET subtotal_minor = ?, total_minor = ?, cost_minor = ?,
                      paid_minor = ?, due_minor = ?, status = 'confirmed',
                      confirmed_by = ?, confirmed_at = ? WHERE id = ?",
                )
                .bind(sale2_total)
                .bind(sale2_total)
                .bind(sale2_cost)
                .bind(sale2_paid)
                .bind(sale2_total - sale2_paid)
                .bind(admin_id)
                .bind(&now)
                .bind(sale2_id)
                .execute(&mut *tx)
                .await?;

                let sale3_id =
                    insert_sale(tx, "CUS-003", showroom_id, admin_id, "2026-09-01", &now)
                        .await?;
                insert_sale_item(tx, sale3_id, 1, "FS-014", 1, 8_200_000, 5_500_000).await?;
                insert_sale_item(tx, sale3_id, 2, "FS-016", 2, 1_800_000, 1_200_000).await?;
                insert_sale_item(tx, sale3_id, 3, "FS-017", 1, 3_800_000, 2_500_000).await?;
                let sale3_total: i64 = 8_200_000 + 2 * 1_800_000 + 3_800_000;
                let sale3_cost: i64 = 5_500_000 + 2 * 1_200_000 + 2_500_000;
                let sale3_paid: i64 = sale3_total / 2;
                sqlx::query(
                    "UPDATE sales SET subtotal_minor = ?, total_minor = ?, cost_minor = ?,
                      paid_minor = ?, due_minor = ?, status = 'confirmed',
                      confirmed_by = ?, confirmed_at = ? WHERE id = ?",
                )
                .bind(sale3_total)
                .bind(sale3_total)
                .bind(sale3_cost)
                .bind(sale3_paid)
                .bind(sale3_total - sale3_paid)
                .bind(admin_id)
                .bind(&now)
                .bind(sale3_id)
                .execute(&mut *tx)
                .await?;

                let sale_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sales")
                    .fetch_one(&mut *tx)
                    .await?;

                // ── Deduct stock for confirmed sales ──────────────────
                let sale_stock: &[(i64, i64)] = &[
                    (1, 1),
                    (4, 2),
                    (8, 1),
                    (13, 2),
                    (14, 1),
                    (16, 2),
                    (17, 1),
                ];
                for &(prod_id, qty) in sale_stock {
                    sqlx::query(
                        "UPDATE stock_balances SET on_hand = MAX(0, on_hand - ?) WHERE product_id = ? AND location_id = ?",
                    )
                    .bind(qty)
                    .bind(prod_id)
                    .bind(showroom_id)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "INSERT INTO stock_movements
                             (product_id, location_id, movement_type, quantity_delta, unit_cost_minor,
                              reason, created_by, created_at)
                         VALUES (?, ?, 'sale_issue', ?, 0, 'Demo sale', ?, ?)",
                    )
                    .bind(prod_id)
                    .bind(showroom_id)
                    .bind(-qty)
                    .bind(admin_id)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }

                // ── Expenses ──────────────────────────────────────────
                let rent_cat: i64 = sqlx::query_scalar(
                    "SELECT id FROM expense_categories WHERE code = 'rent'",
                )
                .fetch_one(&mut *tx)
                .await?;
                let salary_cat: i64 = sqlx::query_scalar(
                    "SELECT id FROM expense_categories WHERE code = 'salaries'",
                )
                .fetch_one(&mut *tx)
                .await?;
                let elec_cat: i64 = sqlx::query_scalar(
                    "SELECT id FROM expense_categories WHERE code = 'electricity'",
                )
                .fetch_one(&mut *tx)
                .await?;
                let transport_cat: i64 = sqlx::query_scalar(
                    "SELECT id FROM expense_categories WHERE code = 'transport'",
                )
                .fetch_one(&mut *tx)
                .await?;

                let cash_acct: i64 = sqlx::query_scalar(
                    "SELECT id FROM cash_accounts WHERE code = 'main_cash'",
                )
                .fetch_one(&mut *tx)
                .await?;

                // All amounts in paisa: PKR * 100
                let expenses_data: &[(i64, i64, &str, &str)] = &[
                    (rent_cat, 150_000_000, "2026-08-01", "August shop rent"),
                    (salary_cat, 80_000_000, "2026-08-05", "Staff salaries - August"),
                    (elec_cat, 35_000_000, "2026-08-10", "Electricity bill - August"),
                    (transport_cat, 12_000_000, "2026-08-15", "Delivery fuel costs"),
                    (rent_cat, 150_000_000, "2026-09-01", "September shop rent"),
                    (salary_cat, 80_000_000, "2026-09-05", "Staff salaries - September"),
                ];

                for &(cat, amount, date, desc) in expenses_data {
                    sqlx::query(
                        "INSERT INTO expenses
                             (expense_number, category_id, amount_minor, expense_date,
                              cash_account_id, description, status, created_by, created_at,
                              posted_by, posted_at)
                         VALUES (NULL, ?, ?, ?, ?, ?, 'posted', ?, ?, ?, ?)",
                    )
                    .bind(cat)
                    .bind(amount)
                    .bind(date)
                    .bind(cash_acct)
                    .bind(desc)
                    .bind(admin_id)
                    .bind(&now)
                    .bind(admin_id)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }
                sqlx::query(
                    "UPDATE expenses SET expense_number = 'EXP-' || printf('%04d', id) WHERE expense_number IS NULL",
                )
                .execute(&mut *tx)
                .await?;

                let exp_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM expenses")
                    .fetch_one(&mut *tx)
                    .await?;

                // ── Owner Capital ─────────────────────────────────────
                sqlx::query(
                    "INSERT INTO owner_transactions
                         (transaction_number, kind, amount_minor, transaction_date,
                          cash_account_id, notes, created_by, created_at)
                     VALUES (NULL, 'capital_in', 500_000_000, '2026-07-01', ?,
                             'Initial capital injection', ?, ?)",
                )
                .bind(cash_acct)
                .bind(admin_id)
                .bind(&now)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE owner_transactions SET transaction_number = 'OWN-' || printf('%04d', id) WHERE transaction_number IS NULL",
                )
                .execute(&mut *tx)
                .await?;

                Ok(SeedResult {
                    categories: cat_count,
                    product_types: pt_count,
                    products: prod_count,
                    suppliers: sup_count,
                    customers: cust_count,
                    purchases: pur_count,
                    sales: sale_count,
                    expenses: exp_count,
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
    .bind(name)
    .bind(sort)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
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
    .bind(name)
    .bind(parent_id)
    .bind(sort)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    Ok(id)
}

async fn insert_pt(
    tx: &mut sqlx::SqliteConnection,
    category_id: i64,
    name: &str,
    now: &str,
) -> Result<i64, AppError> {
    let id = sqlx::query(
        "INSERT INTO product_types (category_id, name, created_at, updated_at) VALUES (?, ?, ?, ?)",
    )
    .bind(category_id)
    .bind(name)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    Ok(id)
}

async fn insert_purchase(
    tx: &mut sqlx::SqliteConnection,
    supplier_code: &str,
    location_id: i64,
    admin_id: i64,
    invoice_number: &str,
    now: &str,
) -> Result<i64, AppError> {
    let supplier_name: String = sqlx::query_scalar("SELECT name FROM suppliers WHERE code = ?")
        .bind(supplier_code)
        .fetch_one(&mut *tx)
        .await?;

    let id = sqlx::query(
        "INSERT INTO purchases
             (supplier_id, supplier_name, location_id, invoice_number, invoice_date,
              purchase_date, status, created_by, created_at, updated_at)
         VALUES ((SELECT id FROM suppliers WHERE code = ?), ?, ?, ?, ?, ?, 'draft', ?, ?, ?)",
    )
    .bind(supplier_code)
    .bind(&supplier_name)
    .bind(location_id)
    .bind(invoice_number)
    .bind(&now[..10])
    .bind(&now[..10])
    .bind(admin_id)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
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
            .bind(article)
            .fetch_one(&mut *tx)
            .await?;

    sqlx::query(
        "INSERT INTO purchase_items
             (purchase_id, product_id, article_number, product_name, quantity,
              unit_cost_minor, line_total_minor)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(purchase_id)
    .bind(prod_id)
    .bind(article)
    .bind(&prod_name)
    .bind(qty)
    .bind(cost_minor)
    .bind(qty * cost_minor)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

async fn insert_sale(
    tx: &mut sqlx::SqliteConnection,
    customer_code: &str,
    location_id: i64,
    admin_id: i64,
    sale_date: &str,
    now: &str,
) -> Result<i64, AppError> {
    let customer_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM customers WHERE code = ?")
            .bind(customer_code)
            .fetch_one(&mut *tx)
            .await?;

    let id = sqlx::query(
        "INSERT INTO sales
             (customer_id, customer_name, location_id, sale_date, status, created_by,
              created_at, updated_at)
         VALUES ((SELECT id FROM customers WHERE code = ?), ?, ?, ?, 'draft', ?, ?, ?)",
    )
    .bind(customer_code)
    .bind(&customer_name)
    .bind(location_id)
    .bind(sale_date)
    .bind(admin_id)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
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
            .bind(article)
            .fetch_one(&mut *tx)
            .await?;

    sqlx::query(
        "INSERT INTO sale_items
             (sale_id, sort_order, product_id, article_number, product_name, quantity,
              unit_price_minor, line_total_minor, unit_cost_minor, line_cost_minor)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(sale_id)
    .bind(sort_order)
    .bind(prod_id)
    .bind(article)
    .bind(&prod_name)
    .bind(qty)
    .bind(price_minor)
    .bind(qty * price_minor)
    .bind(cost_minor)
    .bind(qty * cost_minor)
    .execute(&mut *tx)
    .await?;
    Ok(())
}
