use sqlx::Row;

use crate::application::auth::Principal;
use crate::dto::search::{SearchResultDto, SearchResultsDto};
use crate::error::AppError;
use crate::state::AppState;

/// True when the principal holds any of the listed permissions.
fn allowed(principal: &Principal, any: &[&str]) -> bool {
    principal
        .permissions
        .iter()
        .any(|held| any.contains(&held.as_str()))
}

struct Hit {
    kind: &'static str,
    id: i64,
    title: String,
    subtitle: Option<String>,
    ref_number: Option<String>,
    rank: i64,
}

impl Hit {
    fn new(
        kind: &'static str,
        id: i64,
        title: String,
        subtitle: Option<String>,
        ref_number: Option<String>,
        exact: bool,
        prefix: bool,
    ) -> Self {
        let rank = if exact {
            0
        } else if prefix {
            1
        } else {
            2
        };
        Hit {
            kind,
            id,
            title,
            subtitle,
            ref_number,
            rank,
        }
    }
}

/// Search products, customers, suppliers, sales, purchases, deliveries,
/// payments and expenses by reference/name/phone. Every kind is permission
/// gated with the same permission as its module list screen. The database
/// returns a tight per-kind candidate set ordered by exact/prefix match, and
/// final ordering puts exact matches before prefix and substring matches.
pub async fn global_search(
    state: &AppState,
    principal: &Principal,
    query: &str,
) -> Result<SearchResultsDto, AppError> {
    let qn = query.trim().to_lowercase();
    if qn.is_empty() {
        return Ok(SearchResultsDto {
            query: query.to_string(),
            results: Vec::new(),
        });
    }
    let pre = format!("{qn}%");
    let all = format!("%{qn}%");

    let mut hits: Vec<Hit> = Vec::new();

    hits.extend(search_products(state, &qn, &pre, &all).await?);

    if allowed(principal, &["customer.view"]) {
        hits.extend(search_customers(state, &qn, &pre, &all).await?);
        hits.extend(search_receipts(state, &qn, &pre, &all).await?);
    }
    if allowed(principal, &["payable.view"]) {
        hits.extend(search_suppliers(state, &qn, &pre, &all).await?);
        hits.extend(search_purchases(state, &qn, &pre, &all).await?);
        hits.extend(search_supplier_payments(state, &qn, &pre, &all).await?);
    }
    if allowed(principal, &["sale.create", "invoice.print"]) {
        hits.extend(search_sales(state, &qn, &pre, &all).await?);
    }
    if allowed(principal, &["delivery.view"]) {
        hits.extend(search_deliveries(state, &qn, &pre, &all).await?);
    }
    if allowed(principal, &["expense.view"]) {
        hits.extend(search_expenses(state, &qn, &pre, &all).await?);
    }

    let kind_order = |kind: &str| match kind {
        "product" => 0,
        "customer" => 1,
        "supplier" => 2,
        "sale" => 3,
        "purchase" => 4,
        "delivery" => 5,
        "supplier_payment" => 6,
        "receipt" => 7,
        "expense" => 8,
        _ => 99,
    };

    hits.sort_by_key(|h| (kind_order(h.kind), h.rank, h.title.to_lowercase()));

    let results = hits
        .into_iter()
        .take(40)
        .map(|h| SearchResultDto {
            kind: h.kind.to_string(),
            id: h.id,
            title: h.title,
            subtitle: h.subtitle,
            ref_number: h.ref_number,
            rank: h.rank,
        })
        .collect();

    Ok(SearchResultsDto {
        query: query.to_string(),
        results,
    })
}

async fn search_products(
    state: &AppState,
    qn: &str,
    pre: &str,
    all: &str,
) -> Result<Vec<Hit>, AppError> {
    let rows = sqlx::query(
        "SELECT p.id, p.name, p.article_number,
                (LOWER(p.article_number_norm) = ?) AS e_art,
                (LOWER(p.name) = ?) AS e_name,
                (LOWER(p.article_number_norm) LIKE ?) AS p_art,
                (LOWER(p.name) LIKE ?) AS p_name
         FROM products p
         WHERE p.archived_at IS NULL
           AND (LOWER(p.article_number_norm) LIKE ? OR LOWER(p.name) LIKE ?)
         ORDER BY e_art DESC, e_name DESC, p_art DESC, p_name DESC, p.id ASC
         LIMIT 20",
    )
    .bind(qn)
    .bind(qn)
    .bind(pre)
    .bind(pre)
    .bind(all)
    .bind(all)
    .fetch_all(&state.pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(Hit::new(
                "product",
                r.try_get(0)?,
                r.try_get(1)?,
                Some(format!("Article {}", r.try_get::<String, _>(2)?)),
                Some(r.try_get::<String, _>(2)?),
                r.try_get::<bool, _>(3)? || r.try_get::<bool, _>(4)?,
                r.try_get::<bool, _>(5)? || r.try_get::<bool, _>(6)?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()
}

async fn search_customers(
    state: &AppState,
    qn: &str,
    pre: &str,
    all: &str,
) -> Result<Vec<Hit>, AppError> {
    let rows = sqlx::query(
        "SELECT c.id, c.name, c.code, c.phone,
                (LOWER(c.name) = ?) AS e_name,
                (LOWER(c.code) = ?) AS e_code,
                (LOWER(c.phone) = ?) AS e_phone,
                (LOWER(c.name) LIKE ?) AS p_name,
                (LOWER(c.code) LIKE ?) AS p_code,
                (LOWER(c.phone) LIKE ?) AS p_phone
         FROM customers c
         WHERE c.is_active = 1
           AND (LOWER(c.name) LIKE ? OR LOWER(c.code) LIKE ? OR LOWER(c.phone) LIKE ?)
         ORDER BY e_name DESC, e_code DESC, e_phone DESC,
                  p_name DESC, p_code DESC, p_phone DESC, c.id ASC
         LIMIT 20",
    )
    .bind(qn)
    .bind(qn)
    .bind(qn)
    .bind(pre)
    .bind(pre)
    .bind(pre)
    .bind(all)
    .bind(all)
    .bind(all)
    .fetch_all(&state.pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            let phone = r.try_get::<Option<String>, _>(3)?;
            Ok(Hit::new(
                "customer",
                r.try_get(0)?,
                r.try_get(1)?,
                phone.filter(|s| !s.is_empty()),
                Some(r.try_get::<String, _>(2)?),
                r.try_get::<bool, _>(4)? || r.try_get::<bool, _>(5)? || r.try_get::<bool, _>(6)?,
                r.try_get::<bool, _>(7)? || r.try_get::<bool, _>(8)? || r.try_get::<bool, _>(9)?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()
}

async fn search_suppliers(
    state: &AppState,
    qn: &str,
    pre: &str,
    all: &str,
) -> Result<Vec<Hit>, AppError> {
    let rows = sqlx::query(
        "SELECT s.id, s.name, s.code, s.phone,
                (LOWER(s.name) = ?) AS e_name,
                (LOWER(s.code) = ?) AS e_code,
                (LOWER(s.phone) = ?) AS e_phone,
                (LOWER(s.name) LIKE ?) AS p_name,
                (LOWER(s.code) LIKE ?) AS p_code,
                (LOWER(s.phone) LIKE ?) AS p_phone
         FROM suppliers s
         WHERE s.is_active = 1
           AND (LOWER(s.name) LIKE ? OR LOWER(s.code) LIKE ? OR LOWER(s.phone) LIKE ?)
         ORDER BY e_name DESC, e_code DESC, e_phone DESC,
                  p_name DESC, p_code DESC, p_phone DESC, s.id ASC
         LIMIT 20",
    )
    .bind(qn)
    .bind(qn)
    .bind(qn)
    .bind(pre)
    .bind(pre)
    .bind(pre)
    .bind(all)
    .bind(all)
    .bind(all)
    .fetch_all(&state.pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            let phone = r.try_get::<Option<String>, _>(3)?;
            Ok(Hit::new(
                "supplier",
                r.try_get(0)?,
                r.try_get(1)?,
                phone.filter(|s| !s.is_empty()),
                Some(r.try_get::<String, _>(2)?),
                r.try_get::<bool, _>(4)? || r.try_get::<bool, _>(5)? || r.try_get::<bool, _>(6)?,
                r.try_get::<bool, _>(7)? || r.try_get::<bool, _>(8)? || r.try_get::<bool, _>(9)?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()
}

async fn search_sales(
    state: &AppState,
    qn: &str,
    pre: &str,
    all: &str,
) -> Result<Vec<Hit>, AppError> {
    let rows = sqlx::query(
        "SELECT s.id, s.sale_number, s.customer_name, s.sale_date,
                (LOWER(s.sale_number) = ?) AS e_num,
                (LOWER(s.customer_name) = ?) AS e_cust,
                (LOWER(s.sale_number) LIKE ?) AS p_num,
                (LOWER(s.customer_name) LIKE ?) AS p_cust
         FROM sales s
         WHERE s.status IN ('confirmed', 'quotation', 'draft')
           AND (LOWER(s.sale_number) LIKE ? OR LOWER(s.customer_name) LIKE ?)
         ORDER BY e_num DESC, e_cust DESC, p_num DESC, p_cust DESC, s.id DESC
         LIMIT 20",
    )
    .bind(qn)
    .bind(qn)
    .bind(pre)
    .bind(pre)
    .bind(all)
    .bind(all)
    .fetch_all(&state.pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            let num = r.try_get::<String, _>(1)?;
            Ok(Hit::new(
                "sale",
                r.try_get(0)?,
                num.clone(),
                Some(format!("Customer: {}", r.try_get::<String, _>(2)?)),
                Some(num),
                r.try_get::<bool, _>(4)? || r.try_get::<bool, _>(5)?,
                r.try_get::<bool, _>(6)? || r.try_get::<bool, _>(7)?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()
}

async fn search_receipts(
    state: &AppState,
    qn: &str,
    pre: &str,
    all: &str,
) -> Result<Vec<Hit>, AppError> {
    let rows = sqlx::query(
        "SELECT cp.id, cp.receipt_number, c.name,
                (LOWER(cp.receipt_number) = ?) AS e_num,
                (LOWER(c.name) = ?) AS e_cust,
                (LOWER(cp.receipt_number) LIKE ?) AS p_num,
                (LOWER(c.name) LIKE ?) AS p_cust
         FROM customer_payments cp
         JOIN customers c ON c.id = cp.customer_id
         WHERE cp.status = 'posted'
           AND (LOWER(cp.receipt_number) LIKE ? OR LOWER(c.name) LIKE ?)
         ORDER BY e_num DESC, e_cust DESC, p_num DESC, p_cust DESC, cp.id DESC
         LIMIT 20",
    )
    .bind(qn)
    .bind(qn)
    .bind(pre)
    .bind(pre)
    .bind(all)
    .bind(all)
    .fetch_all(&state.pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            let num = r.try_get::<String, _>(1)?;
            Ok(Hit::new(
                "receipt",
                r.try_get(0)?,
                num.clone(),
                Some(format!("Customer: {}", r.try_get::<String, _>(2)?)),
                Some(num),
                r.try_get::<bool, _>(3)? || r.try_get::<bool, _>(4)?,
                r.try_get::<bool, _>(5)? || r.try_get::<bool, _>(6)?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()
}

async fn search_purchases(
    state: &AppState,
    qn: &str,
    pre: &str,
    all: &str,
) -> Result<Vec<Hit>, AppError> {
    let rows = sqlx::query(
        "SELECT po.id, po.purchase_number, po.supplier_name, po.purchase_date,
                (LOWER(po.purchase_number) = ?) AS e_num,
                (LOWER(po.supplier_name) = ?) AS e_supp,
                (LOWER(po.purchase_number) LIKE ?) AS p_num,
                (LOWER(po.supplier_name) LIKE ?) AS p_supp
         FROM purchases po
         WHERE (LOWER(po.purchase_number) LIKE ? OR LOWER(po.supplier_name) LIKE ?)
         ORDER BY e_num DESC, e_supp DESC, p_num DESC, p_supp DESC, po.id DESC
         LIMIT 20",
    )
    .bind(qn)
    .bind(qn)
    .bind(pre)
    .bind(pre)
    .bind(all)
    .bind(all)
    .fetch_all(&state.pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            let num = r.try_get::<String, _>(1)?;
            Ok(Hit::new(
                "purchase",
                r.try_get(0)?,
                num.clone(),
                Some(format!("Supplier: {}", r.try_get::<String, _>(2)?)),
                Some(num),
                r.try_get::<bool, _>(4)? || r.try_get::<bool, _>(5)?,
                r.try_get::<bool, _>(6)? || r.try_get::<bool, _>(7)?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()
}

async fn search_supplier_payments(
    state: &AppState,
    qn: &str,
    pre: &str,
    all: &str,
) -> Result<Vec<Hit>, AppError> {
    let rows = sqlx::query(
        "SELECT sp.id, sp.payment_number, s.name,
                (LOWER(sp.payment_number) = ?) AS e_num,
                (LOWER(s.name) = ?) AS e_supp,
                (LOWER(sp.payment_number) LIKE ?) AS p_num,
                (LOWER(s.name) LIKE ?) AS p_supp
         FROM supplier_payments sp
         JOIN suppliers s ON s.id = sp.supplier_id
         WHERE sp.status = 'posted'
           AND (LOWER(sp.payment_number) LIKE ? OR LOWER(s.name) LIKE ?)
         ORDER BY e_num DESC, e_supp DESC, p_num DESC, p_supp DESC, sp.id DESC
         LIMIT 20",
    )
    .bind(qn)
    .bind(qn)
    .bind(pre)
    .bind(pre)
    .bind(all)
    .bind(all)
    .fetch_all(&state.pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            let num = r.try_get::<String, _>(1)?;
            Ok(Hit::new(
                "supplier_payment",
                r.try_get(0)?,
                num.clone(),
                Some(format!("Supplier: {}", r.try_get::<String, _>(2)?)),
                Some(num),
                r.try_get::<bool, _>(3)? || r.try_get::<bool, _>(4)?,
                r.try_get::<bool, _>(5)? || r.try_get::<bool, _>(6)?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()
}

async fn search_deliveries(
    state: &AppState,
    qn: &str,
    pre: &str,
    all: &str,
) -> Result<Vec<Hit>, AppError> {
    let rows = sqlx::query(
        "SELECT d.id, COALESCE(d.delivery_number, ''),
                COALESCE(d.customer_name, ''), d.status,
                (LOWER(COALESCE(d.delivery_number, '')) = ?) AS e_num,
                (LOWER(COALESCE(d.customer_name, '')) = ?) AS e_cust,
                (LOWER(COALESCE(d.delivery_number, '')) LIKE ?) AS p_num,
                (LOWER(COALESCE(d.customer_name, '')) LIKE ?) AS p_cust
         FROM deliveries d
         WHERE (LOWER(COALESCE(d.delivery_number, '')) LIKE ?
                OR LOWER(COALESCE(d.customer_name, '')) LIKE ?)
         ORDER BY e_num DESC, e_cust DESC, p_num DESC, p_cust DESC, d.id DESC
         LIMIT 20",
    )
    .bind(qn)
    .bind(qn)
    .bind(pre)
    .bind(pre)
    .bind(all)
    .bind(all)
    .fetch_all(&state.pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            let num = r.try_get::<String, _>(1)?;
            let status = r.try_get::<String, _>(3)?;
            Ok(Hit::new(
                "delivery",
                r.try_get(0)?,
                if num.is_empty() {
                    format!("Delivery #{}", r.try_get::<i64, _>(0)?)
                } else {
                    num.clone()
                },
                Some(format!(
                    "Customer: {} · {}",
                    r.try_get::<String, _>(2)?,
                    status
                )),
                Some(num),
                r.try_get::<bool, _>(4)? || r.try_get::<bool, _>(5)?,
                r.try_get::<bool, _>(6)? || r.try_get::<bool, _>(7)?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()
}

async fn search_expenses(
    state: &AppState,
    qn: &str,
    pre: &str,
    all: &str,
) -> Result<Vec<Hit>, AppError> {
    let rows = sqlx::query(
        "SELECT e.id, e.expense_number, c.name,
                COALESCE(e.description, ''), COALESCE(e.payee, ''),
                (LOWER(e.expense_number) = ?) AS e_num,
                (LOWER(COALESCE(e.description, '')) = ?) AS e_desc,
                (LOWER(COALESCE(e.payee, '')) = ?) AS e_payee,
                (LOWER(e.expense_number) LIKE ?) AS p_num,
                (LOWER(COALESCE(e.description, '')) LIKE ?) AS p_desc,
                (LOWER(COALESCE(e.payee, '')) LIKE ?) AS p_payee
         FROM expenses e
         JOIN expense_categories c ON c.id = e.category_id
         WHERE e.status = 'posted'
           AND (LOWER(e.expense_number) LIKE ?
                OR LOWER(COALESCE(e.description, '')) LIKE ?
                OR LOWER(COALESCE(e.payee, '')) LIKE ?)
         ORDER BY e_num DESC, e_desc DESC, e_payee DESC,
                  p_num DESC, p_desc DESC, p_payee DESC, e.id DESC
         LIMIT 20",
    )
    .bind(qn)
    .bind(qn)
    .bind(qn)
    .bind(pre)
    .bind(pre)
    .bind(pre)
    .bind(all)
    .bind(all)
    .bind(all)
    .fetch_all(&state.pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            let num = r.try_get::<String, _>(1)?;
            Ok(Hit::new(
                "expense",
                r.try_get(0)?,
                num.clone(),
                Some(r.try_get::<String, _>(2)?),
                Some(num),
                r.try_get::<bool, _>(5)? || r.try_get::<bool, _>(6)? || r.try_get::<bool, _>(7)?,
                r.try_get::<bool, _>(8)? || r.try_get::<bool, _>(9)? || r.try_get::<bool, _>(10)?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()
}
