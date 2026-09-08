# Product Vision and Scope

## Vision

Create a dependable but uncomplicated furniture-shop system that makes every physical item visually identifiable, keeps stock and money accurate, and gives the owner an immediate view of sales, dues, payables, delivery commitments, expenses, and profit.

## Primary users

- **Owner:** complete visibility, settings, approvals, reports, backup.
- **Manager:** products, prices, stock, sales, purchases, customers, suppliers.
- **Salesperson:** catalogue, quotations, sales, receipts, customer payments.
- **Accountant:** payments, expenses, supplier payables, statements, reports.
- **Storekeeper:** receiving, stock transfers, reservations, damage, delivery release.

## Core outcomes

1. Find any item by picture, article number, name, category, material, or color.
2. Know exactly what is available, reserved, damaged, sold, and awaiting delivery.
3. Sell individual items or a custom-priced set without corrupting component stock.
4. Track customer advances, partial payments, refunds, and dues.
5. Track supplier purchases, payments, credits, and outstanding balances.
6. Produce understandable invoices, statements, and management reports as PDFs.
7. Keep all critical changes traceable and recoverable.

## In scope for Version 1

- Dashboard and Quick Add.
- Product catalogue, categories, product types, variants, multiple images.
- One or more inventory locations on the same local database.
- Purchases, purchase payments, supplier returns.
- Sales, quotations, discounts, taxes if enabled, advances, credit, returns.
- Fixed furniture sets and ad-hoc sale packages.
- Customer and supplier ledgers.
- Deliveries and damage tracking.
- Expenses, cash movements, operational profit reports.
- PDF/print/CSV export.
- Users, RBAC, approvals, audit log.
- Backup, restore, settings, optional software licensing.

## Explicitly out of scope for Version 1

- E-commerce storefront.
- Native mobile application.
- Real-time cloud sync or unsafe network-shared SQLite.
- Full double-entry accounting, payroll, or tax filing.
- Manufacturing bills of materials and production planning.
- Automated bank reconciliation.

## Future-ready extensions

- Central PostgreSQL API for branches and multiple computers.
- Cloud backup and cross-device synchronization.
- Barcode label printing and scanner workflows.
- WhatsApp invoice/due reminders through an approved provider.
- E-commerce catalogue publishing.
- Full accounting integration.

## Success measures

- Common sale entered in under 60 seconds after products are configured.
- Article-number search results appear within 300 ms on normal shop hardware.
- No successful financial transaction can leave stock or ledgers half-updated.
- Owner can reconcile cash, dues, payables, and stock from system records.
- Restore drill recovers a recent backup on a clean installation.

