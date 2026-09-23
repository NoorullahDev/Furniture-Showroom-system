use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use chrono_tz::Tz;
use sqlx::Row;

use crate::application::auth::Principal;
use crate::dto::dashboard::{
    DashboardDeliveryDto, DashboardRecentSaleDto, DashboardStockItemDto, DashboardSummaryDto,
    DashboardTopProductDto, DashboardTransactionDto,
};
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::state::AppState;

fn allowed(principal: &Principal, any: &[&str]) -> bool {
    principal
        .permissions
        .iter()
        .any(|held| any.contains(&held.as_str()))
}

fn managed_image_path(state: &AppState, stored: Option<String>) -> Option<String> {
    let value = stored.filter(|value| !value.trim().is_empty())?;
    let path = std::path::Path::new(&value);
    if path.is_absolute() {
        Some(value)
    } else if path.components().count() == 1 {
        Some(
            state
                .paths
                .images_dir
                .join(path)
                .to_string_lossy()
                .into_owned(),
        )
    } else {
        None
    }
}

async fn configured_timezone(state: &AppState) -> Result<Tz, AppError> {
    let raw: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key = 'shop.timezone'")
            .fetch_optional(&state.pool)
            .await?;
    let name = raw
        .and_then(|value| serde_json::from_str::<String>(&value).ok())
        .unwrap_or_else(|| "Asia/Karachi".into());
    match name.parse::<Tz>() {
        Ok(timezone) => Ok(timezone),
        Err(_) => {
            tracing::warn!(
                timezone = name,
                "invalid shop timezone; using UTC for dashboard"
            );
            Ok(chrono_tz::UTC)
        }
    }
}

pub async fn dashboard_summary(
    state: &AppState,
    principal: &Principal,
) -> Result<DashboardSummaryDto, AppError> {
    dashboard_summary_at(state, principal, state.clock.now_utc()).await
}

async fn dashboard_summary_at(
    state: &AppState,
    principal: &Principal,
    now: DateTime<Utc>,
) -> Result<DashboardSummaryDto, AppError> {
    let timezone = configured_timezone(state).await?;
    let shop_today = now.with_timezone(&timezone).date_naive();
    let today = shop_today.format("%Y-%m-%d").to_string();
    let month_start = NaiveDate::from_ymd_opt(shop_today.year(), shop_today.month(), 1)
        .expect("valid first day of month")
        .format("%Y-%m-%d")
        .to_string();
    let top_products_start = (shop_today - Duration::days(29))
        .format("%Y-%m-%d")
        .to_string();

    let can_view_sales = allowed(principal, &["sale.create", "invoice.print"]);
    let can_view_receipts = allowed(principal, &["payment.receive", "customer.view"]);
    let can_view_customers = allowed(principal, &["payment.receive", "customer.view"]);
    let can_view_payables = allowed(principal, &["payable.view"]);
    let can_view_deliveries = allowed(principal, &["delivery.view"]);
    let can_view_damage = allowed(principal, &["damage.record"]);

    // Sales figures use sale/return document dates in the configured shop
    // timezone. Payments are queried separately and are never added to sales.
    let (today_sales_count, today_sales_minor, month_sales_minor) = if can_view_sales {
        let row = sqlx::query(
            "SELECT
                 (SELECT COUNT(*) FROM sales
                   WHERE status = 'confirmed' AND sale_date = ?1),
                 COALESCE((SELECT SUM(total_minor) FROM sales
                   WHERE status = 'confirmed' AND sale_date = ?1), 0)
                   - COALESCE((SELECT SUM(total_minor) FROM sales_returns
                   WHERE status = 'posted' AND return_date = ?1), 0),
                 COALESCE((SELECT SUM(total_minor) FROM sales
                   WHERE status = 'confirmed' AND sale_date BETWEEN ?2 AND ?1), 0)
                   - COALESCE((SELECT SUM(total_minor) FROM sales_returns
                   WHERE status = 'posted' AND return_date BETWEEN ?2 AND ?1), 0)",
        )
        .bind(&today)
        .bind(&month_start)
        .fetch_one(&state.pool)
        .await?;
        (
            Some(row.try_get(0)?),
            Some(row.try_get(1)?),
            Some(row.try_get(2)?),
        )
    } else {
        (None, None, None)
    };

    let (today_received_count, today_received_minor) = if can_view_receipts {
        let row = sqlx::query(
            "SELECT COUNT(*), COALESCE(SUM(amount_minor), 0)
               FROM customer_payments
              WHERE status = 'posted' AND payment_date = ?",
        )
        .bind(&today)
        .fetch_one(&state.pool)
        .await?;
        (Some(row.try_get(0)?), Some(row.try_get(1)?))
    } else {
        (None, None)
    };

    let (customer_dues_minor, overdue_customer_count, overdue_customer_minor) =
        if can_view_customers {
            let row = sqlx::query(
                "SELECT
                     COALESCE(SUM(CASE WHEN balance_minor > 0 THEN balance_minor ELSE 0 END), 0),
                     (SELECT COUNT(*) FROM sales
                       WHERE status = 'confirmed' AND due_minor > 0
                         AND due_date IS NOT NULL AND date(due_date) < date(?1)),
                     COALESCE((SELECT SUM(due_minor) FROM sales
                       WHERE status = 'confirmed' AND due_minor > 0
                         AND due_date IS NOT NULL AND date(due_date) < date(?1)), 0)
                 FROM (
                    SELECT COALESCE(
                        (SELECT balance_after_minor FROM customer_ledger_entries e
                          WHERE e.customer_id = c.id ORDER BY e.id DESC LIMIT 1),
                        c.opening_balance_minor) AS balance_minor
                    FROM customers c WHERE c.is_active = 1
                 )",
            )
            .bind(&today)
            .fetch_one(&state.pool)
            .await?;
            (
                Some(row.try_get(0)?),
                Some(row.try_get(1)?),
                Some(row.try_get(2)?),
            )
        } else {
            (None, None, None)
        };

    let (supplier_payables_minor, overdue_supplier_count, overdue_supplier_minor) =
        if can_view_payables {
            let row = sqlx::query(
                "SELECT
                     COALESCE(SUM(CASE WHEN balance_minor > 0 THEN balance_minor ELSE 0 END), 0),
                     (SELECT COUNT(*) FROM purchases
                       WHERE status = 'posted' AND due_minor > 0
                         AND date(invoice_date) < date(?1)),
                     COALESCE((SELECT SUM(due_minor) FROM purchases
                       WHERE status = 'posted' AND due_minor > 0
                         AND date(invoice_date) < date(?1)), 0)
                 FROM (
                    SELECT COALESCE(
                        (SELECT balance_after_minor FROM supplier_ledger_entries e
                          WHERE e.supplier_id = s.id ORDER BY e.id DESC LIMIT 1),
                        s.opening_balance_minor) AS balance_minor
                    FROM suppliers s WHERE s.is_active = 1
                 )",
            )
            .bind(&today)
            .fetch_one(&state.pool)
            .await?;
            (
                Some(row.try_get(0)?),
                Some(row.try_get(1)?),
                Some(row.try_get(2)?),
            )
        } else {
            (None, None, None)
        };

    let (pending_deliveries, upcoming_deliveries) = if can_view_deliveries {
        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM deliveries
              WHERE status NOT IN ('delivered', 'cancelled')",
        )
        .fetch_one(&state.pool)
        .await?;
        let rows = sqlx::query(
            "SELECT d.id, d.delivery_number, d.sale_id, d.customer_id,
                    COALESCE(NULLIF(d.customer_name, ''), 'Walk-in Customer'),
                    COALESCE((SELECT GROUP_CONCAT(item_text, ', ') FROM (
                        SELECT CAST(di.quantity AS TEXT) || ' x ' || di.product_name AS item_text
                          FROM delivery_items di WHERE di.delivery_id = d.id ORDER BY di.id
                    )), 'No items recorded'),
                    d.scheduled_at, d.status,
                    CASE WHEN d.scheduled_at IS NOT NULL
                              AND date(d.scheduled_at) < date(?1) THEN 1 ELSE 0 END
               FROM deliveries d
              WHERE d.status NOT IN ('delivered', 'cancelled')
              ORDER BY CASE WHEN d.scheduled_at IS NOT NULL
                                  AND date(d.scheduled_at) < date(?1) THEN 0 ELSE 1 END,
                       CASE WHEN d.scheduled_at IS NULL THEN 1 ELSE 0 END,
                       datetime(d.scheduled_at) ASC, d.id ASC
              LIMIT 8",
        )
        .bind(&today)
        .fetch_all(&state.pool)
        .await?;
        let deliveries = rows
            .into_iter()
            .map(|row| DashboardDeliveryDto {
                id: row.get(0),
                delivery_number: row.try_get(1).ok(),
                sale_id: row.get(2),
                customer_id: row.try_get(3).ok(),
                customer_name: row.get(4),
                items: row.get(5),
                scheduled_at: row.try_get(6).ok(),
                status: row.get(7),
                is_overdue: row.get::<i64, _>(8) != 0,
            })
            .collect();
        (Some(pending), deliveries)
    } else {
        (None, Vec::new())
    };

    let configured_threshold_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM products
          WHERE archived_at IS NULL AND track_stock = 1 AND minimum_stock > 0",
    )
    .fetch_one(&state.pool)
    .await?;
    let low_stock_uses_threshold = configured_threshold_count > 0;
    let stock_rows = sqlx::query(if low_stock_uses_threshold {
        "WITH stock AS (
             SELECT p.id, p.name,
                    (SELECT pi.thumbnail_path FROM product_images pi
                      WHERE pi.product_id = p.id
                      ORDER BY pi.is_primary DESC, pi.sort_order, pi.id LIMIT 1) AS thumbnail_path,
                    p.minimum_stock,
                    COALESCE(SUM(b.on_hand - b.reserved - b.damaged), 0) AS available
               FROM products p
               LEFT JOIN stock_balances b ON b.product_id = p.id
              WHERE p.archived_at IS NULL AND p.track_stock = 1
              GROUP BY p.id
         )
         SELECT id, name, thumbnail_path, available
           FROM stock
          WHERE (minimum_stock > 0 AND available < minimum_stock) OR available <= 0
          ORDER BY (minimum_stock - available) DESC, name COLLATE NOCASE
          LIMIT 5"
    } else {
        "WITH stock AS (
             SELECT p.id, p.name,
                    (SELECT pi.thumbnail_path FROM product_images pi
                      WHERE pi.product_id = p.id
                      ORDER BY pi.is_primary DESC, pi.sort_order, pi.id LIMIT 1) AS thumbnail_path,
                    COALESCE(SUM(b.on_hand - b.reserved - b.damaged), 0) AS available
               FROM products p
               LEFT JOIN stock_balances b ON b.product_id = p.id
              WHERE p.archived_at IS NULL AND p.track_stock = 1
              GROUP BY p.id
         )
         SELECT id, name, thumbnail_path, available
           FROM stock
          WHERE available <= 0
          ORDER BY available, name COLLATE NOCASE
          LIMIT 5"
    })
    .fetch_all(&state.pool)
    .await?;
    let low_stock_items: Vec<DashboardStockItemDto> = stock_rows
        .into_iter()
        .map(|row| DashboardStockItemDto {
            product_id: row.get(0),
            product_name: row.get(1),
            thumbnail_path: managed_image_path(state, row.try_get(2).ok().flatten()),
            available: row.get(3),
        })
        .collect();
    let low_stock_count: i64 = if low_stock_uses_threshold {
        sqlx::query_scalar(
            "WITH stock AS (
                 SELECT p.id, p.minimum_stock,
                        COALESCE(SUM(b.on_hand - b.reserved - b.damaged), 0) AS available
                   FROM products p
                   LEFT JOIN stock_balances b ON b.product_id = p.id
                  WHERE p.archived_at IS NULL AND p.track_stock = 1
                  GROUP BY p.id
             )
             SELECT COUNT(*) FROM stock
              WHERE (minimum_stock > 0 AND available < minimum_stock) OR available <= 0",
        )
        .fetch_one(&state.pool)
        .await?
    } else {
        sqlx::query_scalar(
            "WITH stock AS (
                 SELECT p.id, COALESCE(SUM(b.on_hand - b.reserved - b.damaged), 0) AS available
                   FROM products p
                   LEFT JOIN stock_balances b ON b.product_id = p.id
                  WHERE p.archived_at IS NULL AND p.track_stock = 1
                  GROUP BY p.id
             )
             SELECT COUNT(*) FROM stock WHERE available <= 0",
        )
        .fetch_one(&state.pool)
        .await?
    };
    let open_damage_count = if can_view_damage {
        Some(
            sqlx::query_scalar("SELECT COUNT(*) FROM damage_records WHERE status = 'open'")
                .fetch_one(&state.pool)
                .await?,
        )
    } else {
        None
    };

    let rows = sqlx::query(
        "SELECT id, customer_id, transaction_date, customer_name, reference,
                transaction_type, amount_minor
           FROM (
             SELECT s.id, s.customer_id, s.sale_date AS transaction_date,
                    COALESCE(NULLIF(s.customer_name, ''), 'Walk-in Customer') AS customer_name,
                    COALESCE(NULLIF(s.sale_number, ''), '#' || s.id) AS reference,
                    'Sale' AS transaction_type, s.total_minor AS amount_minor,
                    s.created_at AS sort_time
               FROM sales s
              WHERE ?1 = 1 AND s.status = 'confirmed'
             UNION ALL
             SELECT p.id, p.customer_id, p.payment_date AS transaction_date,
                    c.name AS customer_name,
                    COALESCE(NULLIF(p.receipt_number, ''), '#' || p.id) AS reference,
                    'Payment' AS transaction_type, p.amount_minor AS amount_minor,
                    p.created_at AS sort_time
               FROM customer_payments p
               JOIN customers c ON c.id = p.customer_id
              WHERE ?2 = 1 AND p.status = 'posted'
           ) recent
          ORDER BY transaction_date DESC, sort_time DESC, id DESC
          LIMIT 10",
    )
    .bind(if can_view_sales { 1 } else { 0 })
    .bind(if can_view_receipts { 1 } else { 0 })
    .fetch_all(&state.pool)
    .await?;
    let recent_transactions = rows
        .into_iter()
        .map(|row| DashboardTransactionDto {
            id: row.get(0),
            customer_id: row.try_get(1).ok(),
            transaction_date: row.get(2),
            customer_name: row.get(3),
            reference: row.get(4),
            transaction_type: row.get(5),
            amount_minor: row.get(6),
        })
        .collect();

    let recent_sales = if can_view_sales {
        sqlx::query(
            "SELECT s.id, COALESCE(NULLIF(s.sale_number, ''), '#' || s.id),
                    COALESCE(NULLIF(s.customer_name, ''), 'Walk-in Customer'),
                    s.sale_date, s.total_minor
               FROM sales s
              WHERE s.status = 'confirmed'
              ORDER BY s.sale_date DESC, s.created_at DESC, s.id DESC
              LIMIT 5",
        )
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|row| DashboardRecentSaleDto {
            id: row.get(0),
            invoice: row.get(1),
            customer_name: row.get(2),
            sale_date: row.get(3),
            amount_minor: row.get(4),
        })
        .collect()
    } else {
        Vec::new()
    };

    let top_products = if can_view_sales {
        sqlx::query(
            "SELECT COALESCE(si.product_id, si.bundle_id),
                    CASE WHEN si.product_id IS NOT NULL THEN 'product' ELSE 'bundle' END,
                    si.product_name,
                    CASE WHEN si.product_id IS NOT NULL THEN
                        (SELECT pi.thumbnail_path FROM product_images pi
                          WHERE pi.product_id = si.product_id
                          ORDER BY pi.is_primary DESC, pi.sort_order, pi.id LIMIT 1)
                    ELSE b.cover_image_path END,
                    SUM(si.quantity) AS units_sold,
                    SUM(si.line_total_minor) AS sales_amount_minor
               FROM sale_items si
               JOIN sales s ON s.id = si.sale_id
               LEFT JOIN bundles b ON b.id = si.bundle_id
              WHERE s.status = 'confirmed' AND s.sale_date BETWEEN ?1 AND ?2
              GROUP BY si.product_id, si.bundle_id, si.article_number, si.product_name,
                       CASE WHEN si.product_id IS NOT NULL THEN 'product' ELSE 'bundle' END
              ORDER BY sales_amount_minor DESC, units_sold DESC, si.product_name COLLATE NOCASE
              LIMIT 8",
        )
        .bind(&top_products_start)
        .bind(&today)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .map(|row| {
            let image_path = managed_image_path(state, row.try_get(3).ok().flatten());
            DashboardTopProductDto {
                item_id: row.get(0),
                item_type: row.get(1),
                product_name: row.get(2),
                image_path,
                units_sold: row.get(4),
                sales_amount_minor: row.get(5),
            }
        })
        .collect()
    } else {
        Vec::new()
    };

    Ok(DashboardSummaryDto {
        as_of: now.to_rfc3339(),
        shop_date: today,
        today_sales_count,
        today_sales_minor,
        today_received_count,
        today_received_minor,
        month_sales_minor,
        customer_dues_minor,
        supplier_payables_minor,
        pending_deliveries,
        overdue_customer_count,
        overdue_customer_minor,
        overdue_supplier_count,
        overdue_supplier_minor,
        low_stock_count,
        open_damage_count,
        upcoming_deliveries,
        recent_transactions,
        recent_sales,
        low_stock_items,
        low_stock_uses_threshold,
        top_products,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn state_and_principal() -> (std::path::PathBuf, AppState, Principal) {
        let dir =
            std::env::temp_dir().join(format!("furniture-shop-dashboard-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let paths = crate::infrastructure::FilePaths::init(&dir).unwrap();
        let (pool, _) = crate::infrastructure::db::open(&paths).await.unwrap();
        let state = AppState::new(pool, paths);
        let user_id = sqlx::query(
            "INSERT INTO users (username, password_hash, full_name) VALUES ('dashboard-owner', 'test-only', 'Owner')",
        )
        .execute(&state.pool)
        .await
        .unwrap()
        .last_insert_rowid();
        let principal = Principal {
            session_id: "dashboard-test".into(),
            user_id,
            username: "dashboard-owner".into(),
            full_name: "Owner".into(),
            roles: vec!["owner".into()],
            permissions: vec![
                "sale.create".into(),
                "payment.receive".into(),
                "customer.view".into(),
                "payable.view".into(),
                "delivery.view".into(),
                "damage.record".into(),
            ],
        };
        (dir, state, principal)
    }

    #[tokio::test]
    async fn totals_follow_document_status_returns_and_shop_timezone() {
        let (dir, state, principal) = state_and_principal().await;
        sqlx::query(
            "INSERT INTO settings (key, value_json, updated_by) VALUES ('shop.timezone', '\"Asia/Karachi\"', ?)",
        )
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap();
        let location_id: i64 = sqlx::query_scalar("SELECT id FROM locations LIMIT 1")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let method_id: i64 = sqlx::query_scalar("SELECT id FROM payment_methods LIMIT 1")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let account_id: i64 = sqlx::query_scalar("SELECT id FROM cash_accounts LIMIT 1")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        let customer_id = sqlx::query(
            "INSERT INTO customers (code, name, created_by) VALUES ('C001', 'Ali Customer', ?)",
        )
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap()
        .last_insert_rowid();
        sqlx::query(
            "INSERT INTO customer_ledger_entries
             (customer_id, entry_type, amount_minor, balance_after_minor, created_by)
             VALUES (?, 'sale', 5000, 5000, ?)",
        )
        .bind(customer_id)
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap();
        let supplier_id = sqlx::query(
            "INSERT INTO suppliers (code, name, created_by) VALUES ('S001', 'Wood Supplier', ?)",
        )
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap()
        .last_insert_rowid();
        sqlx::query(
            "INSERT INTO supplier_ledger_entries
             (supplier_id, entry_type, amount_minor, balance_after_minor, created_by)
             VALUES (?, 'invoice', 7000, 7000, ?)",
        )
        .bind(supplier_id)
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap();

        let old_sale = sqlx::query(
            "INSERT INTO sales
             (sale_number, customer_id, customer_name, location_id, sale_date, status,
              subtotal_minor, total_minor, due_minor, due_date, created_by, created_at)
             VALUES ('INV-OLD', ?, 'Ali Customer', ?, '2026-09-30', 'confirmed',
                     3000, 3000, 3000, '2026-09-30', ?, '2026-09-30T12:00:00Z')",
        )
        .bind(customer_id)
        .bind(location_id)
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap()
        .last_insert_rowid();
        let today_sale = sqlx::query(
            "INSERT INTO sales
             (sale_number, customer_id, customer_name, location_id, sale_date, status,
              subtotal_minor, total_minor, due_minor, due_date, created_by, created_at)
             VALUES ('INV-TODAY', ?, 'Ali Customer', ?, '2026-10-01', 'confirmed',
                     10000, 10000, 2000, '2026-10-31', ?, '2026-10-01T01:00:00Z')",
        )
        .bind(customer_id)
        .bind(location_id)
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap()
        .last_insert_rowid();
        sqlx::query(
            "INSERT INTO sales
             (sale_number, customer_id, customer_name, location_id, sale_date, status,
              subtotal_minor, total_minor, due_minor, created_by)
             VALUES ('INV-CANCELLED', ?, 'Ali Customer', ?, '2026-10-01', 'cancelled',
                     90000, 90000, 0, ?)",
        )
        .bind(customer_id)
        .bind(location_id)
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sales_returns
             (return_number, sale_id, customer_id, location_id, return_date, status,
              refund_type, total_minor, total_refund_minor, created_by)
             VALUES ('RET-1', ?, ?, ?, '2026-10-01', 'posted', 'credit', 1500, 1500, ?)",
        )
        .bind(today_sale)
        .bind(customer_id)
        .bind(location_id)
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO customer_payments
             (receipt_number, customer_id, payment_method_id, cash_account_id,
              payment_date, amount_minor, status, created_by, created_at)
             VALUES ('RCP-1', ?, ?, ?, '2026-10-01', 4000, 'posted', ?, '2026-10-01T02:00:00Z')",
        )
        .bind(customer_id)
        .bind(method_id)
        .bind(account_id)
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO customer_payments
             (receipt_number, customer_id, payment_method_id, cash_account_id,
              payment_date, amount_minor, status, created_by)
             VALUES ('RCP-VOID', ?, ?, ?, '2026-10-01', 99000, 'voided', ?)",
        )
        .bind(customer_id)
        .bind(method_id)
        .bind(account_id)
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO purchases
             (purchase_number, supplier_id, supplier_name, location_id, invoice_number,
              invoice_date, purchase_date, status, total_minor, due_minor, created_by)
             VALUES ('PUR-1', ?, 'Wood Supplier', ?, 'SUP-INV-1', '2026-09-29',
                     '2026-09-29', 'posted', 7000, 7000, ?)",
        )
        .bind(supplier_id)
        .bind(location_id)
        .bind(principal.user_id)
        .execute(&state.pool)
        .await
        .unwrap();

        for (status, scheduled, number) in [
            ("failed", "2026-09-30", "DLV-OVERDUE"),
            ("pending", "2026-10-02", "DLV-UPCOMING"),
            ("delivered", "2026-09-29", "DLV-DONE"),
            ("cancelled", "2026-09-29", "DLV-CANCELLED"),
        ] {
            sqlx::query(
                "INSERT INTO deliveries
                 (delivery_number, sale_id, customer_id, customer_name, location_id,
                  status, scheduled_at, created_by)
                 VALUES (?, ?, ?, 'Ali Customer', ?, ?, ?, ?)",
            )
            .bind(number)
            .bind(old_sale)
            .bind(customer_id)
            .bind(location_id)
            .bind(status)
            .bind(scheduled)
            .bind(principal.user_id)
            .execute(&state.pool)
            .await
            .unwrap();
        }

        // 2026-09-30 20:30 UTC is already 2026-10-01 in Asia/Karachi.
        let now = DateTime::parse_from_rfc3339("2026-09-30T20:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let summary = dashboard_summary_at(&state, &principal, now).await.unwrap();
        assert_eq!(summary.shop_date, "2026-10-01");
        assert_eq!(summary.today_sales_count, Some(1));
        assert_eq!(summary.today_sales_minor, Some(8500));
        assert_eq!(summary.month_sales_minor, Some(8500));
        assert_eq!(summary.today_received_count, Some(1));
        assert_eq!(summary.today_received_minor, Some(4000));
        assert_eq!(summary.customer_dues_minor, Some(5000));
        assert_eq!(summary.supplier_payables_minor, Some(7000));
        assert_eq!(summary.pending_deliveries, Some(2));
        assert_eq!(summary.overdue_customer_count, Some(1));
        assert_eq!(summary.overdue_customer_minor, Some(3000));
        assert_eq!(summary.overdue_supplier_count, Some(1));
        assert_eq!(summary.overdue_supplier_minor, Some(7000));
        assert_eq!(summary.upcoming_deliveries.len(), 2);
        assert!(summary.upcoming_deliveries[0].is_overdue);
        assert_eq!(summary.upcoming_deliveries[0].status, "failed");
        assert_eq!(summary.recent_transactions.len(), 3);
        assert_eq!(summary.recent_transactions[0].transaction_type, "Payment");

        state.pool.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn restricted_metrics_are_absent_not_fake_zeroes() {
        let (dir, state, mut principal) = state_and_principal().await;
        principal.permissions.clear();
        let now = DateTime::parse_from_rfc3339("2026-09-30T20:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let summary = dashboard_summary_at(&state, &principal, now).await.unwrap();
        assert!(summary.today_sales_minor.is_none());
        assert!(summary.today_received_minor.is_none());
        assert!(summary.customer_dues_minor.is_none());
        assert!(summary.supplier_payables_minor.is_none());
        assert!(summary.pending_deliveries.is_none());
        assert!(summary.upcoming_deliveries.is_empty());
        assert!(summary.recent_transactions.is_empty());
        assert!(summary.recent_sales.is_empty());
        assert!(summary.top_products.is_empty());
        state.pool.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
