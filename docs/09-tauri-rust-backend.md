# Tauri and Rust Backend

## Responsibility

The Rust backend owns database connections, migrations, authentication sessions, authorization, business workflows, image storage, PDF/CSV creation, backup/restore, logging, and safe OS integration.

## Command conventions

Commands are small adapters:

```rust
#[tauri::command]
async fn complete_sale(
    state: State<'_, AppState>,
    request: CompleteSaleRequest,
) -> Result<SaleReceiptDto, AppErrorDto> {
    state.sales.complete(request, state.session.current_user()?).await
}
```

Commands must not contain SQL or duplicate calculation logic. Request and response DTOs are versioned and serializable. Error responses include a stable code, safe message, optional field errors, and correlation ID.

## Command groups

- Session: `login`, `logout`, `current_session`, `change_password`.
- Dashboard: `get_dashboard_summary`, `get_quick_add_config`.
- Catalogue: product/category/type/bundle CRUD and image operations.
- Inventory: balances, movements, adjustments, transfers, reservations.
- Sales: draft, quote, complete, cancel, return, payment, receipt.
- Purchasing: purchase, receipt, payment, return.
- Parties: customer/supplier CRUD, balance, statement.
- Operations: delivery, expense, cash, reporting.
- Administration: users, roles, settings, audit, backup, restore, licensing.

## Application state

`AppState` contains the database pool, authenticated session manager, application services, file-store roots, report engine, write coordinator, and configuration. Avoid global mutable variables.

## Database initialization

1. Resolve the Tauri app-data directory.
2. Create required directories with restrictive permissions where supported.
3. Open SQLite with foreign keys, WAL, busy timeout, and safe synchronous policy.
4. Create a verified backup before risky migrations.
5. Run ordered embedded migrations.
6. Validate schema version and seed only idempotent defaults.

## File safety

- Generate internal filenames; never trust a user filename as a path.
- Canonicalize and verify every read/write target is inside an approved app directory.
- Validate image type from decoded bytes, not extension alone.
- Use temporary files followed by atomic rename for PDFs, backups, and images.
- Remove newly created files if their database transaction fails.

## Error categories

`VALIDATION`, `UNAUTHORIZED`, `FORBIDDEN`, `NOT_FOUND`, `CONFLICT`, `INSUFFICIENT_STOCK`, `INVALID_STATE`, `DATABASE_BUSY`, `IO_ERROR`, `REPORT_ERROR`, `BACKUP_ERROR`, and `INTERNAL`. Technical detail goes to logs; the UI receives actionable safe text.

## Background work

Image resizing, large exports, backup verification, and report generation should run off the UI thread. Emit bounded progress events and allow safe cancellation before finalization.

