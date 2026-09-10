use sqlx::Row;

use crate::application::auth::Principal;
use crate::application::expenses::profit_summary;
use crate::application::inventory::list_low_stock;
use crate::dto::dashboard::{DashboardActivityDto, DashboardSummaryDto, DashboardTrendDayDto};
use crate::error::AppError;
use crate::infrastructure::clock::Clock;
use crate::state::AppState;

/// True when the principal holds any of the listed permissions.
fn allowed(principal: &Principal, any: &[&str]) -> bool {
    principal
        .permissions
        .iter()
        .any(|held| any.contains(&held.as_str()))
}

/// Read landing summary. Every metric is filtered by the caller's permissions
/// before it enters the DTO; cost-derived figures (stock value, gross profit)
/// are dropped entirely for callers without the matching permission.
pub async fn dashboard_summary(
    state: &AppState,
    principal: &Principal,
) -> Result<DashboardSummaryDto, AppError> {
    let as_of = state.clock.now_iso();

    let can_view_sales = allowed(principal, &["sale.create", "invoice.print"]);
    let can_view_receipts = allowed(principal, &["payment.receive", "customer.view"]);
    let can_view_expenses = allowed(principal, &["expense.view"]);
    let can_view_cash = allowed(principal, &["payable.view"]);
    let can_view_customers = allowed(principal, &["payment.receive", "customer.view"]);
    let can_view_payables = allowed(principal, &["payable.view"]);
    let can_view_cost = allowed(principal, &["product.cost.view"]);
    let can_view_deliveries = allowed(principal, &["delivery.view"]);
    let can_view_damage = allowed(principal, &["damage.record"]);
    let can_view_profit = allowed(principal, &["profit.view"]);
    let can_view_audit = allowed(principal, &["audit.view"]);

    // --- Today's sales -------------------------------
    let (today_sales_count, today_sales_minor) = if can_view_sales {
        let row = sqlx::query(
            "SELECT COUNT(*), COALESCE(SUM(total_minor), 0)
             FROM sales
             WHERE status = 'confirmed' AND sale_date = date('now')",
        )
        .fetch_one(&state.pool)
        .await?;
        (row.try_get::<i64, _>(0)?, row.try_get::<i64, _>(1)?)
    } else {
        (0, 0)
    };

    // --- Today's customer receipts --------------------
    let today_receipts_minor = if can_view_receipts {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_minor), 0)
             FROM customer_payments
             WHERE status = 'posted' AND payment_date = date('now')",
        )
        .fetch_one(&state.pool)
        .await?
    } else {
        0
    };

    // --- Today's expenses -----------------------------
    let today_expenses_minor = if can_view_expenses {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_minor), 0)
             FROM expenses
             WHERE status = 'posted' AND expense_date = date('now')",
        )
        .fetch_one(&state.pool)
        .await?
    } else {
        0
    };

    // --- Net cash across all cash accounts ------------
    let net_cash_minor = if can_view_cash {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(
                       COALESCE(c.opening_balance_minor, 0) +
                       (SELECT COALESCE(SUM(amount_minor), 0)
                          FROM cash_entries e WHERE e.cash_account_id = c.id)
                   ), 0)
             FROM cash_accounts c",
        )
        .fetch_one(&state.pool)
        .await?
    } else {
        0
    };

    // --- Customer dues and overdue dues ----------------
    let (dues_minor, overdue_dues_minor) = if can_view_customers {
        let dues = sqlx::query_scalar(
            "SELECT COALESCE(SUM(b.balance_minor), 0) FROM (
                 SELECT COALESCE(
                            (SELECT balance_after_minor FROM customer_ledger_entries e
                              WHERE e.customer_id = c.id ORDER BY e.id DESC LIMIT 1),
                            c.opening_balance_minor) AS balance_minor
                   FROM customers c WHERE c.is_active = 1
             ) b WHERE b.balance_minor > 0",
        )
        .fetch_one(&state.pool)
        .await?;
        let overdue = sqlx::query_scalar(
            "SELECT COALESCE(SUM(due_minor), 0) FROM sales
             WHERE status = 'confirmed' AND due_minor > 0
               AND due_date IS NOT NULL AND due_date < date('now')",
        )
        .fetch_one(&state.pool)
        .await?;
        (dues, overdue)
    } else {
        (0, 0)
    };

    // --- Supplier payables ------------------------------
    let payables_minor = if can_view_payables {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(b.balance_minor), 0) FROM (
                 SELECT COALESCE(
                            (SELECT balance_after_minor FROM supplier_ledger_entries e
                              WHERE e.supplier_id = s.id ORDER BY e.id DESC LIMIT 1),
                            s.opening_balance_minor) AS balance_minor
                   FROM suppliers s WHERE s.is_active = 1
             ) b WHERE b.balance_minor > 0",
        )
        .fetch_one(&state.pool)
        .await?
    } else {
        0
    };

    // --- Stock value (cost-derived, hidden without cost view)
    let stock_value_minor = if can_view_cost {
        Some(
            sqlx::query_scalar("SELECT COALESCE(SUM(value_minor), 0) FROM current_valuation")
                .fetch_one(&state.pool)
                .await?,
        )
    } else {
        None
    };

    // --- Low stock (no permission gate, mirrors stock_low_list)
    let low_stock_count = list_low_stock(state, principal).await?.len() as i64;

    // --- Pending deliveries -----------------------------
    let pending_deliveries = if can_view_deliveries {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM deliveries
             WHERE status IN ('pending', 'ready', 'dispatched')",
        )
        .fetch_one(&state.pool)
        .await?
    } else {
        0
    };

    // --- Open damage records -----------------------------
    let open_damage_count = if can_view_damage {
        sqlx::query_scalar("SELECT COUNT(*) FROM damage_records WHERE status = 'open'")
            .fetch_one(&state.pool)
            .await?
    } else {
        0
    };

    // --- Month-to-date figures ---------------------------
    let today = state.clock.now_utc().format("%Y-%m-%d").to_string();
    let month_start = format!("{}-01", &today[..7]);

    let month_revenue_minor = if can_view_sales {
        sqlx::query_scalar(
            "SELECT COALESCE((
                     (SELECT SUM(total_minor) FROM sales
                       WHERE status = 'confirmed' AND sale_date BETWEEN ? AND ?)
                   - (SELECT COALESCE(SUM(total_minor), 0) FROM sales_returns
                       WHERE status = 'posted' AND return_date BETWEEN ? AND ?)
                 ), 0)",
        )
        .bind(&month_start)
        .bind(&today)
        .bind(&month_start)
        .bind(&today)
        .fetch_one(&state.pool)
        .await?
    } else {
        0
    };

    let month_expenses_minor = if can_view_expenses {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_minor), 0) FROM expenses
             WHERE status = 'posted' AND expense_date BETWEEN ? AND ?",
        )
        .bind(&month_start)
        .bind(&today)
        .fetch_one(&state.pool)
        .await?
    } else {
        0
    };

    let month_gross_profit_minor = if can_view_profit {
        Some(
            profit_summary(
                state,
                principal,
                Some(month_start.clone()),
                Some(today.clone()),
            )
            .await?
            .gross_profit_minor,
        )
    } else {
        None
    };

    // --- Trailing seven-day trend -------------------------
    let trend_rows = sqlx::query(
        "WITH RECURSIVE days(day) AS (
             SELECT date('now', '-6 days')
             UNION ALL
             SELECT date(day, '+1 day') FROM days WHERE day < date('now')
         )
         SELECT d.day,
                COALESCE((SELECT COUNT(*) FROM sales s
                           WHERE s.status = 'confirmed' AND s.sale_date = d.day), 0),
                COALESCE((SELECT SUM(s.total_minor) FROM sales s
                           WHERE s.status = 'confirmed' AND s.sale_date = d.day), 0),
                COALESCE((SELECT SUM(cp.amount_minor) FROM customer_payments cp
                           WHERE cp.status = 'posted' AND cp.payment_date = d.day), 0),
                COALESCE((SELECT SUM(e.amount_minor) FROM expenses e
                           WHERE e.status = 'posted' AND e.expense_date = d.day), 0)
         FROM days d ORDER BY d.day ASC",
    )
    .fetch_all(&state.pool)
    .await?;

    let trend = trend_rows
        .into_iter()
        .map(|r| {
            Ok(DashboardTrendDayDto {
                day: r.try_get(0)?,
                sales_count: if can_view_sales { r.try_get(1)? } else { 0 },
                sales_minor: if can_view_sales { r.try_get(2)? } else { 0 },
                receipts_minor: if can_view_receipts { r.try_get(3)? } else { 0 },
                expenses_minor: if can_view_expenses { r.try_get(4)? } else { 0 },
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    // --- Recent activity ----------------------------------
    let recent_activity = if can_view_audit {
        let rows = sqlx::query(
            "SELECT a.id, a.action, a.entity_type, a.entity_id, u.username, a.created_at
             FROM audit_logs a
             LEFT JOIN users u ON u.id = a.user_id
             ORDER BY a.id DESC LIMIT 10",
        )
        .fetch_all(&state.pool)
        .await?;
        rows.into_iter()
            .map(|r| {
                Ok(DashboardActivityDto {
                    id: r.try_get(0)?,
                    action: r.try_get(1)?,
                    entity_type: r.try_get(2)?,
                    entity_id: r.try_get(3)?,
                    username: r.try_get::<Option<String>, _>(4)?.filter(|s| !s.is_empty()),
                    created_at: r.try_get(5)?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?
    } else {
        Vec::new()
    };

    Ok(DashboardSummaryDto {
        as_of,
        today_sales_count,
        today_sales_minor,
        today_receipts_minor,
        today_expenses_minor,
        net_cash_minor,
        dues_minor,
        overdue_dues_minor,
        payables_minor,
        stock_value_minor,
        low_stock_count,
        pending_deliveries,
        open_damage_count,
        month_gross_profit_minor,
        month_revenue_minor,
        month_expenses_minor,
        trend,
        recent_activity,
    })
}
