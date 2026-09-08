# Detailed Implementation Plan

## 1. Purpose

This plan defines the recommended build order for the Furniture Shop Management System. It converts the product requirements into implementation phases, technical tasks, validation gates, and releasable milestones.

The target is an offline-first Windows desktop application using:

- Tauri 2 as the desktop shell.
- Next.js and TypeScript as a statically exported frontend.
- Rust for trusted application and business logic.
- SQLite for local structured data.
- Local application-data storage for product images, attachments, reports, and backups.

The plan assumes Version 1 runs on one Windows computer with multiple local user accounts. Multi-computer synchronization is not included in Version 1.

## 2. Delivery principles

1. Build vertical slices that include database, Rust, Tauri command, frontend, tests, and audit behavior.
2. Keep financial, inventory, permissions, and document-state logic in Rust and database constraints—not only in React.
3. Complete master data and transaction foundations before dashboards and advanced reports.
4. Use immutable movements and ledgers for posted stock and financial history.
5. Generate a migration for every schema change; never edit an already released migration.
6. Treat every multi-record business workflow as one atomic transaction.
7. Deliver a working, testable application at the end of every milestone.
8. Prefer simple shop workflows by default and hide advanced modes until enabled.

## 3. Work tracking structure

Track work with the following hierarchy:

```text
Milestone
└── Feature
    └── User story
        ├── Database task
        ├── Rust/domain task
        ├── Tauri command task
        ├── Frontend task
        ├── Automated test task
        └── Acceptance test task
```

Every story must identify the affected permission, audit event, error cases, migration, reports, and backup implications.

## 4. Definition of ready

A feature can enter implementation only when:

- User outcome and workflow are understood.
- Required fields, states, validations, and permissions are listed.
- Stock, ledger, cash, due, payable, and reporting effects are defined.
- Database entities and transaction boundary are known.
- UI loading, empty, error, success, and permission-denied states are defined.
- Acceptance criteria and test fixtures exist.
- Dependencies on unfinished features are identified.

## 5. Definition of done

A feature is complete only when:

- Database migration and rollback/recovery approach are reviewed.
- Rust service owns all authoritative rules and uses checked arithmetic.
- Tauri commands validate input and enforce permissions.
- Frontend uses the design system and typed command wrapper.
- Unit, repository, component, and relevant end-to-end tests pass.
- Posted workflows are idempotent and atomic.
- Audit events and error messages are implemented.
- Keyboard navigation and common Windows display scaling are checked.
- Documentation and acceptance checklist are updated.
- No Critical or High severity defect remains.

## 6. Phase 0 — Product decisions and technical proof

### Objective

Remove high-risk uncertainty before building business modules.

### Business decisions

- Confirm supported Windows versions and minimum hardware.
- Confirm one-computer Version 1 scope.
- Choose English-only launch or English plus Urdu.
- Confirm PKR formatting and whether tax is used.
- Decide whether stock leaves at sale confirmation or delivery. Recommended: immediate counter sales issue at confirmation; scheduled sales reserve first and issue at dispatch/delivery according to one configured policy.
- Confirm whether negative stock is always blocked.
- Confirm A4 invoice and report format; decide whether thermal printing is required in Version 1.
- Confirm FIFO layered inventory costing.
- Define discount, below-cost sale, refund, cancellation, and stock-adjustment approval thresholds.
- Confirm trial/license/expiry behavior.

### Technical prototypes

- Create a minimal Tauri 2 application with statically exported Next.js.
- Invoke one typed Rust command from the frontend and return a typed error.
- Open SQLite in the correct app-data directory and run an embedded migration.
- Import a large furniture photo, validate it, create thumbnails, and display it through a safe local asset path.
- Generate a multi-page Unicode PDF containing PKR amounts and sample Urdu text.
- Create and validate a consistent SQLite backup while WAL mode is active.
- Build an installer and test it on a clean Windows virtual machine.

### Deliverables

- Approved decision record.
- Running technical proof application.
- Initial risk register.
- Confirmed design direction and one POS wireframe.

### Exit criteria

- Tauri build, database, image, PDF, backup, and installer prototypes work on target Windows hardware.
- No unresolved architectural blocker remains.

## 7. Phase 1 — Repository, tooling, and application foundation

### Objective

Create a maintainable base that every later module follows.

### Repository setup

- Initialize Git with protected main branch and pull-request review.
- Add Node, Rust, and Tauri version requirements.
- Commit package and Cargo lockfiles.
- Configure formatting, ESLint, TypeScript strict mode, Rustfmt, and Clippy.
- Add CI jobs for frontend checks, Rust checks, tests, dependency audits, and release build smoke test.
- Add sample environment/configuration documentation without secrets.

### Tauri and application shell

- Configure product identity, icons, application directories, and Windows bundle settings.
- Define least-privilege Tauri capabilities.
- Apply a strict Content Security Policy.
- Disable unnecessary shell, network, and filesystem access.
- Implement graceful startup failure and a safe global error view.

### Rust foundation

- Create `commands`, `application`, `domain`, `repositories`, `infrastructure`, and `dto` modules.
- Implement `AppState`, database pool, session manager, write coordinator, clock, ID generator, and file-root service.
- Define stable error codes and correlation IDs.
- Configure structured rolling logs with sensitive-value redaction.
- Create one typed command wrapper pattern and one transactional service example.

### Database foundation

- Enable foreign keys, WAL, busy timeout, and selected synchronous policy on each connection.
- Create schema migration table and embedded migration runner.
- Add base tables for settings, users, roles, permissions, sessions, audit logs, document sequences, and locations.
- Add timestamps, archive/status conventions, and indexes.
- Implement startup integrity and schema-version checks.

### Frontend foundation

- Configure Next.js static export and TypeScript path aliases.
- Add Tailwind and accessible component primitives.
- Implement app shell, sidebar, top bar, route protection, page header, dialogs, form controls, money input, data table, status badges, and toast/error patterns.
- Add TanStack Query and the centralized Tauri command wrapper.
- Implement responsive behavior for 1366×768 through 1920×1080 and Windows scaling.

### Tests

- Clean install and startup.
- Migration from empty database.
- Database configuration is applied to every connection.
- Rust error mapping and frontend error display.
- Capability tests for denied filesystem/shell access.
- Basic accessibility and keyboard navigation.

### Exit criteria

- The app installs, opens, migrates, logs errors safely, and renders the production application shell.
- CI blocks type, lint, test, or build failures.

## 8. Phase 2 — First-run setup, authentication, RBAC, and audit

### Objective

Secure the system before exposing business commands.

### First-run setup

- Collect shop name, logo, address, contact details, currency, timezone, first location, owner credentials, invoice defaults, and backup location.
- Validate all fields in frontend and Rust.
- Create shop settings and owner account atomically.
- Make first-run completion idempotent so a crash cannot create multiple owners.

### Authentication

- Hash passwords using Argon2id with unique salt.
- Add login, logout, current session, inactivity lock, change password, and owner-controlled reset.
- Rate-limit repeated failed login attempts locally.
- Keep authenticated session state in Rust.

### RBAC

- Seed Owner, Manager, Salesperson, Accountant, and Storekeeper role templates.
- Define action-level permissions.
- Add role and permission management UI.
- Enforce permissions inside every Rust use case.
- Add optional approval authentication for sensitive actions.

### Audit

- Add reusable audit service with actor, action, entity, reason, before/after JSON, approval actor, time, and correlation ID.
- Redact passwords, license secrets, and sensitive technical values.
- Add read-only audit viewer with permission and filters.

### Tests and exit criteria

- Unauthorized commands fail even when invoked outside the visible UI.
- Locked/deactivated sessions cannot perform actions.
- Passwords never appear in database exports, logs, or UI responses.
- Sensitive settings and user changes produce correct audit events.

## 9. Phase 3 — Catalogue, categories, and product images

### Objective

Allow the shop to create and visually recognize every furniture item.

### Database

- Add categories, product types, units, products, attributes, product images, and optional product-style grouping.
- Add case-insensitive normalized unique article-number constraint.
- Add indexes for article number, name, category, type, and active/archive state.

### Rust/domain

- Implement category/type/product create, update, archive, duplicate, and read services.
- Normalize article numbers and reject duplicates in the service and database.
- Prevent hard deletion of referenced master data.
- Implement image decoding, MIME validation, orientation correction, metadata stripping, resizing, thumbnail generation, hashing, staged write, and cleanup.
- Enforce app-directory path boundaries.

### Frontend

- Product list with grid and table modes.
- Fast exact and prefix article-number search.
- Filters for category, type, supplier, price, attributes, and stock status placeholder.
- Product form with multiple images, primary image, reorder, preview, replace, and remove.
- Dynamic category/type management without leaving the product workflow.
- Product detail page with images, pricing, stock placeholder, and activity tabs.

### Tests

- Duplicate articles with case/space differences.
- Invalid, oversized, corrupted, or renamed image files.
- File failure during product creation leaves no broken record/orphan.
- Product archive preserves historical references.
- Search ranking returns exact article first.

### Exit criteria

- A shop user can create, find, edit, duplicate, and archive photo-based products safely.

## 10. Phase 4 — Inventory engine and opening stock

### Objective

Establish trustworthy stock before purchases and sales are added.

### Database

- Add stock movements, reservations, transfer records, stock-count sessions, and optional transactionally maintained balances.
- Create current-stock and available-stock views.
- Index product, location, movement time, type, and source reference.

### Domain and transactions

- Define movement types and signed-quantity rules.
- Implement on-hand, reserved, damaged, in-transit, and available quantities.
- Add opening stock posting, manual adjustment, transfer, reservation, release, damage, repair recovery, and reversal services.
- Apply negative-stock policy in Rust and database-safe transaction flow.
- Implement FIFO cost layers and cost snapshots.

### Frontend

- Inventory overview with article photo, on-hand, reserved, available, damaged, minimum, and value.
- Product/location movement ledger.
- Quick Add stock adjustment and transfer workflows.
- Low-stock list and product stock history.
- Opening-stock import with preview and error report.
- Stock-count session workflow.

### Critical tests

- Displayed stock always equals movement ledger.
- Concurrent/double posting cannot apply a movement twice.
- Transfer produces balanced source/destination effects.
- Negative stock is blocked unless policy and permission allow it.
- Reversal restores the correct net position without deleting history.

### Exit criteria

- Opening stock and every supported manual movement reconcile by product and location.

## 11. Phase 5 — Suppliers, purchases, payables, and cash accounts

### Objective

Capture stock acquisition and supplier debt accurately.

### Database

- Add suppliers, purchases, purchase items, goods receipts if advanced receiving is enabled, supplier payments, supplier ledger entries, supplier returns, payment methods, cash accounts, and cash entries.
- Add unique document sequence and supplier-invoice duplicate checks.

### Domain and transaction services

- Create supplier and opening-payable services.
- Implement draft and posted purchase states.
- Post purchase/receipt atomically: document snapshots, stock movements, FIFO cost layer creation, supplier ledger, payment, cash entry, sequence, and audit.
- Implement full/partial supplier payments with oldest-first or manual allocation.
- Implement payment void through reversal.
- Implement supplier return with stock and payable/cash settlement.
- Add idempotency keys for all posting commands.

### Frontend

- Supplier list, profile, activity, balance, and statement.
- Purchase entry table optimized for keyboard use.
- Inline minimal product/supplier creation with controlled validation.
- Quick Add Record Purchase and Pay Supplier.
- Purchase detail, payment voucher, payable aging, and supplier-return screens.

### Tests

- Paid, partial, and credit purchase.
- Duplicate supplier invoice warning/rejection.
- Purchase failure rolls back stock, ledger, cash, and document number.
- Supplier payment and void reconcile payable and cash.
- Return cannot exceed net received quantity.

### Exit criteria

- A posted purchase changes stock, cost, payable, cash, and audit exactly once.

## 12. Phase 6 — Customers, furniture sets, sales POS, and invoicing

### Objective

Deliver the primary revenue workflow.

### Customers

- Add customer profiles, credit limits, due dates, customer ledger entries, payments, allocations, refunds, and advance balances.
- Implement opening balance as a dedicated ledger entry.
- Create customer search by name and phone plus detailed account timeline.

### Furniture sets

- Add bundles and bundle items with custom default price and cover image.
- Calculate available-set count from the limiting component.
- Snapshot set name, price, and components at sale time.
- Implement permanent sets and permission-controlled ad-hoc packages.
- Calculate bundle cost from component cost snapshots.

### Sales transaction

- Add quotations, sales, sale items, sale-item components, payments, allocations, reservations, and document sequences.
- Implement draft, quotation, confirmed, cancelled, returned, and fulfilment states.
- Recalculate all prices, discounts, tax, delivery charge, paid amount, advance allocation, due, and cost in Rust.
- Validate product/component availability and customer requirements.
- Enforce discount, below-cost, credit-limit, backdate, and negative-stock approvals.
- Complete sale atomically: invoice snapshots, stock issue/reservation, customer ledger, payment, cash, delivery placeholder, audit, and sequence.
- Use idempotency key to prevent duplicate sale submission.

### POS frontend

- Two-panel layout with photo catalogue and current cart.
- Search by exact article, name, category, type, material, and color.
- Add product/set, change quantity, show availability, and expand bundle components.
- Select/create customer inline.
- Support full, partial, credit, and advance-funded payment.
- Show totals and consequences before confirmation.
- Add success screen with invoice, receipt, new sale, and delivery actions.

### PDF and printing foundation

- Build reusable document layout engine.
- Implement A4 invoice, quotation, and receipt templates with embedded Unicode font.
- Store data snapshots, not dependence on current product names/prices.
- Allow reprint from history; print failure never reverses a completed sale.

### Tests

- Individual product and set sale.
- Bundle limited by one component.
- Full, partial, credit, and advance-funded payment.
- Customer due and advance never count the same payment twice.
- Unauthorized price/discount override fails in Rust.
- Duplicate click produces one invoice only.
- Transaction failure leaves no partial stock, ledger, cash, or sequence effect.

### Exit criteria

- A trained salesperson can complete a standard sale in under 60 seconds.
- Invoice, stock, customer balance, cash, and gross-profit source data reconcile.

## 13. Phase 7 — Customer payments, statements, and due control

### Objective

Make receivables simple enough for daily shop use.

### Implementation tasks

- Complete Quick Add Receive Customer Payment.
- Show customer due, advance, overdue documents, and payment allocation preview.
- Support oldest-first automatic allocation and manual allocation.
- Generate separately numbered payment receipt.
- Implement payment void/reversal with permission and reason.
- Add customer statement with opening balance, transactions, running balance, and closing balance.
- Add due-soon, overdue, high-balance, and credit-limit exception screens.
- Add copyable reminder text without automated messaging in Version 1.

### Tests and exit criteria

- Every receipt appears once in customer ledger and cash account.
- Unallocated amount becomes advance.
- Statement balance equals underlying ledger for any tested date range.
- Voiding a payment restores the correct due/advance and cash position.

## 14. Phase 8 — Deliveries, reservations, returns, exchanges, and damage

### Objective

Support the real furniture fulfilment lifecycle after invoice creation.

### Deliveries

- Add deliveries and delivery items.
- Implement pending, ready, dispatched, delivered, failed, rescheduled, and cancelled transitions.
- Track address, contact, schedule, charge, driver/vehicle note, receiver, and proof reference.
- Support partial deliveries without exceeding net sold quantities.
- Connect configured stock issue/reservation behavior to transitions.
- Create delivery calendar/list, detail page, and delivery note PDF.

### Returns and exchanges

- Add sales return, return item, refund, and credit-note records.
- Restrict return quantity to net eligible sold quantity.
- Allocate original discount fairly and cap refundable value.
- Classify item as sellable, damaged, repair, or disposed.
- Represent exchange as linked return plus new sale and allocated return credit.
- Reverse all stock, customer ledger, cash/refund, fulfilment, and audit effects atomically.

### Damage and repair

- Record damage source, product, quantity, location, reason, estimated loss, photo, and decision.
- Add repair recovery, supplier return, damaged sale, and write-off outcomes.
- Exclude non-sellable damaged quantities from availability.

### Tests and exit criteria

- Delivery quantities never exceed net sold quantities.
- Return/refund never exceeds net eligible quantity/value.
- Exchange maintains transparent return and new-sale histories.
- Damaged stock and write-off reports reconcile with stock movements.

## 15. Phase 9 — Expenses, cash management, and profit

### Objective

Give the owner reliable operating visibility without pretending to be a full accounting system.

### Implementation tasks

- Add dynamic expense categories and expense records.
- Implement Quick Add Expense with optional attachment.
- Post expense and cash outflow atomically.
- Reverse rather than edit posted expenses.
- Add owner capital and withdrawal cash types excluded from operational profit.
- Optionally add cash-session opening, expected closing, counted closing, and variance.
- Implement revenue, COGS, gross profit, operational expenses, damage/write-off loss, and operational net-profit queries.
- Clearly separate document-date profit from payment-date cash flow.

### Tests and exit criteria

- Expense appears once in expense report and selected cash account.
- Customer receipt is not counted again as revenue.
- Supplier payment is not counted again as cost.
- Profit fixtures match manually calculated expected values.

## 16. Phase 10 — Dashboard, Quick Add, global search, and notifications

### Objective

Turn completed modules into a fast daily command center.

### Dashboard backend

- Build one permission-filtered summary service.
- Return today's sales, receipts, expenses, net cash, dues, payables, stock value, low stock, monthly gross profit, pending deliveries, recent activity, and seven-day trend.
- Add optimized indexes and explain-query review.
- Do not expose hidden cost/profit fields to unauthorized users.

### Dashboard frontend

- Summary strip, management cards, attention queue, activity table, and compact trend chart.
- Skeleton loading, empty setup checklist, actionable errors, and refresh behavior.
- Links from every attention item to the filtered source screen.

### Quick Add

- Implement `Ctrl+Shift+A`, searchable commands, role-based visibility, arrow-key navigation, and recent actions.
- Add user-pinned actions with a limit of six.
- Default actions: New Sale, Add Product, Receive Customer Payment, Record Purchase, Add Expense, Schedule Delivery.
- Use short modal forms only for simple customer, supplier, payment, and expense actions; open full pages for multi-line workflows.
- Invalidate only related cached data after success.

### Global search

- Search article/product, invoice, quotation, customer, phone, supplier, purchase, receipt, and delivery reference.
- Rank exact article and document-number matches first.
- Restrict results by permission.

### Exit criteria

- Dashboard figures reconcile with module reports.
- A keyboard user can open and complete common Quick Add actions.
- Hidden financial data is absent from both UI and backend response.

## 17. Phase 11 — Reports, PDFs, CSV, and printing

### Objective

Provide accurate operational and management information that can be exported.

### Report framework

- Create shared filter DTOs, date presets, pagination, totals, drill-down links, and background export jobs.
- Apply consistent treatment of posted, cancelled, reversed, and returned data.
- Include active filters and generation metadata in exports.

### Required reports

- Sales by period, product, article, category, customer, user, payment method, and bundle.
- Gross profit and margin.
- Current stock, movements, low stock, valuation, reservations, adjustments, damage, and count variance.
- Customer dues, aging, advances, receipts, and statements.
- Purchases, supplier payables, payments, returns, and statements.
- Deliveries, expenses, cash book, account balances, net operational profit, reversals, and audit exceptions.

### PDF and CSV

- A4 portrait and landscape templates with repeated headings, page totals, final totals, filters, and page numbers.
- Embed fonts and test English/Urdu text.
- Stream large CSV exports and neutralize spreadsheet-formula injection.
- Use controlled save dialogs and safe suggested filenames.
- Add print preview and configured printer preferences.

### Tests and exit criteria

- Screen, PDF, CSV, and source-query totals match fixed fixtures.
- Empty, one-page, multi-page, long-name, Unicode, large-value, refund, and date-boundary cases render correctly.
- Large report does not freeze the main window.

## 18. Phase 12 — Backup, restore, maintenance, and licensing

### Backup and restore

- Use SQLite backup API/checkpoint-aware snapshot.
- Package database, images, attachments, settings, templates, manifest, versions, and checksums.
- Implement daily/weekly/monthly retention.
- Support manual and automatic backup plus optional password encryption.
- Validate checksum, integrity, version compatibility, and disk space before restore.
- Create a safety backup, stage restore, verify, and switch atomically.
- Ensure restore failure leaves current data usable.

### Maintenance

- Show database size, image size, free disk, last backup, last integrity check, version, and failed jobs.
- Add previewable orphan-file and temporary-export cleanup.
- Add controlled database analyze/maintenance action.
- Generate sanitized diagnostic package.

### Licensing

- Define trial, active, grace, expired, suspended, and invalid states.
- Verify signed license payload locally using an embedded public key.
- Support offline request/response activation.
- Keep signing private key outside application and repository.
- On expiry, preserve read-only access and backup/export; never destroy customer data.
- Separate EagleNest Creations application branding from customer invoice branding.

### Exit criteria

- A clean installation can restore a verified backup and reproduce balances, images, users, and reports.
- License-state manipulation cannot bypass signature validation through normal file editing.

## 19. Phase 13 — Hardening, performance, and release candidate

### Performance

- Seed at least 50,000 products, 500,000 movements, 100,000 invoices, and representative images.
- Measure startup, article search, list loading, sale posting, dashboard, reports, backup, and migration.
- Review query plans, add justified indexes, remove N+1 queries, and bound image memory.
- Keep exact article search under 150 ms and broad catalogue search under 300 ms on reference hardware.

### Security review

- Review all Tauri capabilities and command authorization.
- Test path traversal, invalid files, SQL injection resistance, permission bypass, locked session, backup tampering, and CSV injection.
- Run Rust and npm dependency audits.
- Confirm logs and diagnostics redact secrets and personal data.

### Reliability

- Test application termination during sale, image import, backup, export, and migration.
- Verify idempotency recovery when backend committed but frontend missed the response.
- Run SQLite integrity checks and transaction-invariant tests.
- Confirm low-disk behavior and database-busy error recovery.

### UX and accessibility

- Test all major screens at 1366×768, 1920×1080, and 125%/150% Windows scaling.
- Complete keyboard-only workflows.
- Verify contrast, focus, labels, errors, dialogs, and reduced motion.
- Conduct owner, salesperson, accountant, and storekeeper usability sessions.

### Release-candidate exit

- All automated quality gates pass.
- No open Critical or High defect.
- Performance budgets meet target or have approved evidence-based exception.
- Security, migration, backup, and restore reviews pass.

## 20. Phase 14 — Pilot, production rollout, and stabilization

### Pilot preparation

- Import cleaned real product/category/opening-stock data.
- Configure users, permissions, shop branding, invoice sequences, payment methods, locations, printers, and backup destination.
- Take a signed-off opening data snapshot.
- Train each role using its daily workflow.

### Pilot execution

- Run a controlled pilot using real transactions.
- Reconcile daily sales, receipts, customer dues, purchases, supplier payables, stock sample, expenses, cash, and gross profit.
- Log issues with severity, reproduction steps, owner, fix version, and retest evidence.
- Avoid direct database corrections; use approved migration or correction transactions.

### Production rollout

- Install stable signed/release build.
- Verify app version, database version, license, backup, printer, shop profile, opening balances, and stock.
- Complete smoke test: login, product search, sale, receipt, customer payment, purchase, expense, PDF, backup.
- Keep the previous system/read-only records available during agreed transition.

### Stabilization

- Review logs and reconciliation daily during the initial period.
- Fix Critical/High issues through controlled release process.
- Collect usability feedback separately from correctness defects.
- Freeze nonessential features until financial and inventory accuracy is proven.

### Final acceptance

- Owner signs off stock, dues, payables, cash, reports, print documents, permissions, and backup/restore.
- Restore drill succeeds on a clean machine.
- User guide and support process are handed over.

## 21. Milestones and dependency order

| Milestone | Included phases | Depends on | Production value |
| --- | --- | --- | --- |
| M0 Technical proof | Phase 0 | None | Critical risks resolved |
| M1 Secure foundation | Phases 1–2 | M0 | Installable authenticated shell |
| M2 Catalogue and stock | Phases 3–4 | M1 | Products, photos, and reliable inventory |
| M3 Purchasing | Phase 5 | M2 | Stock acquisition and supplier payables |
| M4 Selling | Phases 6–7 | M2–M3 | POS, sets, invoices, dues, receipts |
| M5 Fulfilment | Phase 8 | M4 | Delivery, returns, exchanges, damages |
| M6 Management | Phases 9–11 | M3–M5 | Expenses, dashboard, Quick Add, reports |
| M7 Resilience | Phase 12 | M1–M6 | Backup, maintenance, licensing |
| M8 Release | Phases 13–14 | All | Piloted production system |

Phases may overlap only when database contracts and dependencies are stable. For example, UI work can follow approved DTO mocks while Rust services are implemented, but final integration and acceptance remain one story.

## 22. Suggested team responsibilities

- **Product owner:** decisions, workflow approval, acceptance, scope control.
- **Technical lead:** architecture, transaction design, code review, migrations, release approval.
- **Rust developer:** domain rules, services, repositories, commands, files, backup, PDF.
- **Frontend developer:** workflows, components, accessibility, query integration, print previews.
- **QA owner:** fixtures, automation, regression, performance, UAT evidence.
- **Designer:** design system, POS/catalogue usability, empty/error states, print layout.

One person may hold multiple roles, but the responsibility must still be explicit. Database/financial logic and restore behavior require peer review whenever possible.

## 23. Branching, review, and release workflow

1. Create a small branch tied to one accepted story.
2. Add or update tests with implementation.
3. Add migration only when required; include forward-compatibility notes.
4. Run local format, lint, type, Rust, database, and UI tests.
5. Open review with screenshots and transaction/report evidence.
6. Reviewer checks domain rules, permissions, error cases, indexes, and audit effects.
7. Merge only after CI and review pass.
8. Build pilot release from a version tag, never from an unreviewed developer machine state.

## 24. Data migration and seed strategy

- Keep schema migrations monotonic and immutable after release.
- Seed roles, permissions, units, payment methods, and expense categories idempotently.
- Treat shop products, stock, customers, and suppliers as import data—not hard-coded seeds.
- Validate imports completely and show row-level errors before commit.
- Opening stock, customer dues, and supplier payables create dated opening ledger/movement entries.
- Before every production migration, create a verified backup and test the upgrade from the previous supported version.
- Never run a destructive migration without a staged copy, verification query, and documented recovery path.

## 25. Test data scenarios

Maintain fixed fixtures for:

- Individual chair, table, bed, sofa, cupboard, and non-stock service.
- Bedroom set with one limiting component.
- Multiple images, missing image, Urdu product/customer name, and very long description.
- Cash, partial, credit, advance, refund, and payment reversal.
- Purchase with freight/discount, supplier credit, and supplier return.
- Immediate sale, reserved delivery, partial delivery, failed delivery, exchange, damage, repair, and write-off.
- Discounts within and above permission threshold.
- Exact stock depletion and attempted negative stock.
- Daily/month-end/year boundary report cases.

The expected stock, cash, due, payable, revenue, cost, and profit values must be calculated manually and stored with the fixtures.

## 26. Risk register

| Risk | Effect | Mitigation |
| --- | --- | --- |
| Business logic duplicated in UI | Incorrect totals and bypasses | Rust-authoritative use cases and integration tests |
| Product images consume excessive disk/memory | Slow catalogue or failed backup | Resize, thumbnail, lazy load, limits, disk checks |
| SQLite used over network share | Corruption/locking risk | Local-only database; future API for multiple devices |
| Ambiguous stock issue timing | Incorrect availability/delivery | Decide policy in Phase 0 and encode state transitions |
| Posted records edited directly | Broken audit and balances | Immutable documents plus reversals |
| PDF works in English but fails in Urdu | Broken customer documents | Prototype embedded Unicode font in Phase 0 |
| Duplicate clicks/retries | Duplicate invoices/payments | Idempotency keys and unique constraints |
| Backup exists but cannot restore | False safety | Checksums plus routine clean-machine restore drills |
| Scope expansion delays release | Unfinished core system | Versioned backlog and milestone change control |
| License expiry blocks customer data | Trust and legal risk | Read-only/export/backup access always retained |

## 27. Change-control process

Every new request must record:

- Problem and expected user benefit.
- Proposed version and priority.
- Modules, tables, transactions, permissions, reports, and migrations affected.
- Effect on offline operation and future multi-device design.
- Acceptance criteria and test work.
- Estimated risk and effort.

Changes affecting money, stock, ledger, document state, restore, licensing, or permissions require technical-lead approval and updated test fixtures. New features do not enter an active milestone if they threaten its acceptance gate.

## 28. Recommended first development backlog

1. Technical proof for Tauri, SQLite migration, image, Unicode PDF, backup, and installer.
2. Repository checks and layered Rust/frontend skeleton.
3. First-run shop setup and owner creation.
4. Login, session lock, RBAC, and audit foundation.
5. Category and product-type management.
6. Product creation with unique article number and multi-image pipeline.
7. Catalogue grid/table and article search.
8. Locations, opening stock, movement ledger, and current-stock view.
9. Stock adjustment, low-stock list, and movement history.
10. Supplier and purchase posting vertical slice.
11. Customer and individual-product sale vertical slice.
12. Furniture set and component-stock sale.
13. Payments, dues, advances, receipt, and statements.
14. Delivery and return vertical slices.
15. Expenses, dashboard, Quick Add, reports, backup, and release hardening.

This order creates the smallest safe path to a usable shop system while preserving the architecture required for the complete product.
