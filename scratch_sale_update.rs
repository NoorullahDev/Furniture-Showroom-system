pub async fn sale_update(
    state: &AppState,
    principal: &Principal,
    input: crate::dto::sales::SaleUpdateInput,
) -> Result<SaleDto, AppError> {
    let actor_id = principal.require_any(&["sale.create"])?;
    let actor_session = principal.session_id().unwrap_or_default();
    
    // 1. Fetch current status
    let row = sqlx::query(
        "SELECT status, sale_number FROM sales WHERE id = ?"
    )
    .bind(input.sale_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("sale {}", input.sale_id)))?;
    
    let status: String = row.get(0);
    let old_sale_number: Option<String> = row.get(1);
    
    if status == "cancelled" {
        return Err(AppError::Conflict("Cannot edit a cancelled sale".into()));
    }
    
    let applied_cn: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM credit_notes WHERE sale_id = ?")
        .bind(input.sale_id)
        .fetch_one(&state.pool)
        .await?;
    if applied_cn > 0 {
        return Err(AppError::Conflict("Cannot edit a sale that has credit notes applied".into()));
    }

    // 2. If confirmed, reverse effects by calling sale_cancel internally
    if status == "confirmed" {
        sale_cancel(state, principal, SaleCancelInput {
            sale_id: input.sale_id,
            correlation_id: None,
        }).await?;
        
        // Reset status to draft to allow confirm again
        sqlx::query(
            "UPDATE sales SET status = 'draft', cancelled_by = NULL, cancelled_at = NULL WHERE id = ?"
        )
        .bind(input.sale_id)
        .execute(&state.pool)
        .await?;
    }
    
    // 3. Clear old items
    sqlx::query("DELETE FROM sale_item_components WHERE sale_id = ?")
        .bind(input.sale_id).execute(&state.pool).await?;
    sqlx::query("DELETE FROM sale_items WHERE sale_id = ?")
        .bind(input.sale_id).execute(&state.pool).await?;
        
    // 4. Update sales core info
    let (customer_name, _): (Option<String>, Option<i64>) = if let Some(cid) = input.customer_id {
        let cr = sqlx::query("SELECT name, credit_limit_minor FROM customers WHERE id = ?")
            .bind(cid).fetch_optional(&state.pool).await?
            .ok_or_else(|| AppError::Validation(format!("customer {cid} not found")))?;
        (Some(cr.get(0)), Some(cr.get(1)))
    } else {
        (None, None)
    };
    
    sqlx::query(
        "UPDATE sales SET customer_id = ?, customer_name = ?, location_id = ?, 
            discount_minor = ?, delivery_charge_minor = ?, notes = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?"
    )
    .bind(input.customer_id)
    .bind(customer_name)
    .bind(input.location_id)
    .bind(input.discount_minor.unwrap_or(0))
    .bind(input.delivery_charge_minor.unwrap_or(0))
    .bind(&input.notes)
    .bind(input.sale_id)
    .execute(&state.pool)
    .await?;

    // 5. Insert new items
    let mut tx = state.pool.begin().await?;
    let mut sort_order = 0;
    let mut subtotal = 0;
    
    for item in &input.items {
        let (article_number, product_name, unit_price): (String, String, i64) = if let Some(pid) = item.product_id {
            sqlx::query("SELECT article_number, name, default_price_minor FROM products WHERE id = ?")
                .bind(pid).fetch_optional(&mut *tx).await?
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .ok_or_else(|| AppError::Validation(format!("product {pid} not found")))?
        } else {
            let bid = item.bundle_id.unwrap();
            sqlx::query("SELECT code, name, default_price_minor FROM bundles WHERE id = ?")
                .bind(bid).fetch_optional(&mut *tx).await?
                .map(|r| (r.get(0), r.get(1), r.get(2)))
                .ok_or_else(|| AppError::Validation(format!("bundle {bid} not found")))?
        };
        
        let line_total = unit_price * item.quantity;
        subtotal += line_total;
        
        let sale_item_id = sqlx::query(
            "INSERT INTO sale_items (sale_id, product_id, bundle_id, article_number, product_name, quantity, unit_price_minor, line_total_minor, sort_order)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(input.sale_id)
        .bind(item.product_id)
        .bind(item.bundle_id)
        .bind(&article_number)
        .bind(&product_name)
        .bind(item.quantity)
        .bind(unit_price)
        .bind(line_total)
        .bind(sort_order)
        .execute(&mut *tx).await?.last_insert_rowid();
        
        if let Some(bid) = item.bundle_id {
            let components: Vec<(i64, String, String, i64)> = sqlx::query(
                "SELECT p.id, p.article_number, p.name, bi.quantity
                 FROM bundle_items bi JOIN products p ON bi.product_id = p.id
                 WHERE bi.bundle_id = ?"
            ).bind(bid).fetch_all(&mut *tx).await?
            .into_iter().map(|r| (r.get(0), r.get(1), r.get(2), r.get(3))).collect();
            
            for (cid, cart, cname, cqty) in components {
                sqlx::query(
                    "INSERT INTO sale_item_components (sale_id, sale_item_id, product_id, article_number, product_name, quantity)
                     VALUES (?, ?, ?, ?, ?, ?)"
                )
                .bind(input.sale_id).bind(sale_item_id).bind(cid)
                .bind(cart).bind(cname).bind(cqty * item.quantity)
                .execute(&mut *tx).await?;
            }
        }
        sort_order += 1;
    }
    
    let total = subtotal - input.discount_minor.unwrap_or(0) + input.delivery_charge_minor.unwrap_or(0);
    sqlx::query("UPDATE sales SET subtotal_minor = ?, total_minor = ? WHERE id = ?")
        .bind(subtotal).bind(total).bind(input.sale_id).execute(&mut *tx).await?;
        
    tx.commit().await?;
    
    // 6. If originally confirmed, re-confirm
    if status == "confirmed" {
        sale_confirm(state, principal, SaleConfirmInput {
            sale_id: input.sale_id,
            override_sale_number: old_sale_number,
            idempotency_key: None,
            paid_minor: input.paid_minor,
            cash_account_id: input.cash_account_id,
            payment_method_id: input.payment_method_id,
            advance_used_minor: Some(0),
            credit_note_id: None,
        }).await?;
    }
    
    sale_dto(state, input.sale_id).await
}
