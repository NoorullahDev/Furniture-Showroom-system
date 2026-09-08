# Software Requirements Specification

## Functional requirements

### Dashboard

- FR-D01: Show sales, receipts, expenses, customer dues, supplier payables, stock value, low stock, and pending deliveries.
- FR-D02: Provide a keyboard-accessible Quick Add menu.
- FR-D03: Allow role-based hiding of financial cards and shortcuts.

### Products

- FR-P01: Create a product with a unique user-entered article number.
- FR-P02: Store multiple images, name, category, product type, description, dimensions, material, color, cost, price, tax profile, and minimum stock.
- FR-P03: Support configurable attributes without schema changes.
- FR-P04: Archive referenced products instead of deleting them.
- FR-P05: Search and filter by article number, name, category, type, supplier, attribute, and stock status.

### Inventory

- FR-I01: Calculate stock from immutable stock movements.
- FR-I02: Track available, reserved, damaged, and in-transit quantities by location.
- FR-I03: Support receiving, sale, return, transfer, adjustment, reservation, release, and damage movements.
- FR-I04: Require a reason and permission for manual adjustment.

### Sets

- FR-B01: Define a named set containing two or more product quantities.
- FR-B02: Store a custom default set price independent of summed component prices.
- FR-B03: Check and deduct component availability when selling a set.
- FR-B04: Preserve a sale-time snapshot if the set definition later changes.

### Sales and customers

- FR-S01: Create quotations, sales, invoices, receipts, returns, and refunds.
- FR-S02: Support cash, bank, card, wallet, and custom payment methods.
- FR-S03: Support paid, partial, credit, and advance-funded sales.
- FR-S04: Track invoice total, paid, refunded, and due amounts.
- FR-S05: Prevent discount or price overrides without permission.
- FR-S06: Maintain a customer ledger and printable statement.
- FR-S07: Support optional anonymous walk-in cash sales.

### Purchases and suppliers

- FR-U01: Record purchase orders/invoices and stock receipts.
- FR-U02: Record full, partial, or credit supplier payments.
- FR-U03: Maintain supplier payables and statements.
- FR-U04: Support supplier returns with stock and payable corrections.

### Deliveries and operations

- FR-L01: Schedule partial or complete delivery of sold items.
- FR-L02: Track pending, ready, dispatched, delivered, failed, and cancelled states.
- FR-L03: Record customer address, contact, charge, vehicle/driver note, and proof note.
- FR-R01: Process sales returns, exchanges, refunds, and damaged items.
- FR-E01: Record categorized expenses and attachment references.

### Reports and administration

- FR-RP01: Filter all major reports by date and relevant dimensions.
- FR-RP02: Export supported reports and documents to PDF; tabular reports also to CSV.
- FR-A01: Authenticate local users and enforce RBAC in Rust, not only the UI.
- FR-A02: Audit sensitive actions with actor, time, entity, reason, and before/after values.
- FR-BK01: Create, verify, list, and restore backups.

## Non-functional requirements

- NFR-01: Operate without internet after installation and activation policy requirements.
- NFR-02: Use SQLite foreign keys, constraints, migrations, and atomic transactions.
- NFR-03: Never store passwords in plaintext; use Argon2id hashes.
- NFR-04: Use integer minor currency units, never floating-point money.
- NFR-05: Use integer quantities unless fractional quantities are explicitly enabled.
- NFR-06: Product search under 300 ms for 50,000 products on reference hardware.
- NFR-07: Normal command response under one second, excluding large reports and image import.
- NFR-08: Recover gracefully from expected errors without closing the application.
- NFR-09: Meet keyboard navigation and readable contrast requirements.
- NFR-10: Backward-compatible database migration or a recoverable pre-migration backup.

## Data retention

Financial, stock, and audit records are never hard-deleted through normal UI. They are cancelled, reversed, or archived. Draft documents may be deleted if they have caused no stock, ledger, or cash movement.

