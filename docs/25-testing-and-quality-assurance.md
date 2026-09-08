# Testing and Quality Assurance

## Test pyramid

### Rust unit tests

Money calculations, discounts, tax, bundle availability/cost, FIFO layer cost and snapshots, due/payable balance, return limits, document transitions, permissions, number formatting, and reversal logic.

### Repository/integration tests

Use a fresh temporary SQLite database with production migrations. Test foreign keys, uniqueness, rollback, concurrent attempts, idempotency, views, indexes, migration upgrades, and backup/restore.

### Frontend tests

Form validation, keyboard behavior, table filters, image picker, permissions, Quick Add search/pinning, loading/error/empty states, and accessibility.

### End-to-end tests

Run the packaged-style application where possible:

1. First run and owner creation.
2. Add category/product/images/opening stock.
3. Create set and sell it partially paid.
4. Receive customer payment and print receipt.
5. Purchase on credit and pay supplier.
6. Schedule/complete delivery.
7. Return/damage/exchange workflow.
8. Record expense and verify profit/cash reports.
9. Backup, change data, restore, and reverify.
10. Permission/approval and audit behavior.

## Critical invariants

Tests must prove:

- No partial sale/purchase/payment/return commit.
- Stock equals movement ledger.
- Customer/supplier balances equal ledger entries.
- Cash account balance equals cash entries.
- Bundle sale affects every component exactly once.
- Reversal returns each affected subsystem to the correct net state.
- A duplicate idempotency key cannot duplicate business effects.

## Report verification

Use fixed fixtures with manually calculated expected totals. Compare screen, PDF, CSV, and source-query totals. Include date boundaries, negative/reversal entries, long text, Unicode, empty data, and multi-page layouts.

## Performance testing

Seed 50,000 products, 500,000 movements, 100,000 sale lines, and representative images. Measure startup, exact/prefix search, dashboard, posting, common reports, backup, and migration.

## Quality gates

- Formatting, lint, TypeScript type check, Rust clippy with warnings denied for application code.
- All unit/integration/component tests pass.
- Critical E2E smoke suite passes on Windows.
- No high-severity known dependency vulnerability without documented acceptance.
- Database migration and restore drill pass.
- UI review at 1366×768, 1920×1080, and 125%/150% Windows scaling.

## Bug severity

Critical: data loss, wrong financial/stock totals, authentication bypass. High: blocked primary workflow or incorrect report. Medium: workaround exists. Low: cosmetic. No Critical or High issue ships knowingly.

