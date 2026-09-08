# Performance and Reliability

## Reference scale

Design and test for at least 50,000 products, 500,000 stock movements, 100,000 invoices, 500,000 sale/purchase lines, 50,000 customers/suppliers, and several gigabytes of optimized images over the product lifetime.

## Performance budgets

- Warm startup to usable dashboard: target under 3 seconds on reference hardware.
- Exact article search: under 150 ms; broad catalogue search: under 300 ms.
- Product list first page: under 500 ms.
- Normal sale posting: under 1 second excluding print/PDF.
- Dashboard common range: under 1 second.
- Long report shows progress and never freezes the window.

## Database tuning

- Index real filter/join/order columns and inspect query plans.
- Avoid N+1 query patterns.
- Page large lists; never load full history into React.
- Use bounded connection pooling and a write coordinator.
- Enable WAL, foreign keys, busy timeout, and explicit transactions.
- Keep cached summaries rebuildable and validate them periodically.
- Run `ANALYZE` and safe maintenance under controlled conditions.

## Image performance

- Load thumbnails in grids and medium previews in detail screens.
- Lazy-load offscreen images.
- Cap decoded dimensions to avoid memory spikes.
- Cache thumbnails with bounded storage.
- Never put binary images directly in SQLite rows or base64 in normal IPC responses.

## Report performance

Use indexed aggregate queries, bounded date ranges, streaming CSV generation, and background PDF generation. Cache only expensive safe summaries with clear invalidation after affected transactions.

## Reliability patterns

- Checked arithmetic for money and quantities.
- Idempotency keys for document posting.
- Atomic file finalization and database transactions.
- Graceful application error boundary.
- Pre-migration backup and integrity checks.
- Structured logs with correlation IDs.
- Reversal rather than destructive correction.

## Crash recovery

On startup, detect incomplete staged files/jobs and clean or resume safely. SQLite transaction recovery protects committed data; run quick integrity and migration-state checks. Never infer that an invoice failed solely because the UI did not receive its response—query by idempotency key.

## Disk management

Dashboard/settings warn at configurable low free-space thresholds. Backup, image import, export, and update check space before work. Maintenance can preview orphaned files and old temporary exports before removal.

## Monitoring without cloud

Provide local health status: database size, image storage size, last successful backup, last integrity check, app/schema version, log directory size, and failed background jobs.

