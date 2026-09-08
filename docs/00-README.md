# Furniture Shop Management System

Implementation-ready documentation for an offline-first Windows application built with Tauri, Next.js, TypeScript, Rust, and SQLite.

## Product goal

Give a furniture shop one simple application for product photos, article numbers, stock, sets, sales, customer dues, suppliers, purchases, deliveries, expenses, and PDF reports. A new salesperson should be able to complete a normal sale with minimal training.

## Confirmed baseline

- Windows desktop application.
- Offline-first; daily work never depends on the internet.
- Product catalogue with multiple images and user-entered article numbers.
- Dynamic categories and product types.
- Individual items and custom-priced furniture sets.
- Inventory, sales, purchases, suppliers, customers, dues, advances, expenses, returns, deliveries, and reporting.
- Dashboard **Quick Add** menu for common actions.
- Role-based permissions, audit history, backup, restore, printing, and PDF export.

## Documentation map

| File | Purpose |
| --- | --- |
| `01-product-vision-and-scope.md` | Goals, users, scope, boundaries |
| `02-software-requirements-specification.md` | Functional and non-functional requirements |
| `03-system-architecture.md` | Layered system architecture |
| `04-technology-stack.md` | Selected technologies and rules |
| `05-project-structure.md` | Repository and module layout |
| `06-database-design.md` | Entities, relationships, constraints |
| `07-data-dictionary.md` | Important tables and fields |
| `08-business-rules-and-transactions.md` | Atomic operations and invariants |
| `09-tauri-rust-backend.md` | Tauri commands and Rust services |
| `10-frontend-architecture.md` | Next.js application structure |
| `11-ui-ux-design-system.md` | Production-grade light UI |
| `12-dashboard-and-quick-add.md` | Dashboard and Quick Add behavior |
| `13-product-catalog-and-images.md` | Products, variants, article numbers, images |
| `14-inventory-management.md` | Stock movements, locations, adjustments |
| `15-furniture-sets-and-bundles.md` | Set creation, pricing, stock behavior |
| `16-sales-pos-and-invoicing.md` | Sales workflow and invoices |
| `17-customers-dues-and-payments.md` | Credit, advances, statements |
| `18-suppliers-and-purchasing.md` | Purchases, payables, returns |
| `19-deliveries-returns-and-damages.md` | Fulfilment and reverse flows |
| `20-expenses-profit-and-cash.md` | Expenses, cash, and profit rules |
| `21-reports-pdf-and-exports.md` | Reports, PDFs, CSV exports |
| `22-authentication-rbac-and-audit.md` | Users, permissions, audit log |
| `23-backup-restore-and-data-lifecycle.md` | Backup and recovery |
| `24-security-and-privacy.md` | Desktop and data security |
| `25-testing-and-quality-assurance.md` | Test strategy and quality gates |
| `26-implementation-plan.md` | Development phases and order |
| `27-acceptance-criteria.md` | Definition of done and UAT |
| `28-packaging-deployment-and-updates.md` | Windows installer and releases |
| `29-performance-and-reliability.md` | Performance budgets and resilience |
| `30-settings-licensing-and-localization.md` | Shop settings, license, language |
| `31-phase-0-decision-record.md` | Approved Phase 0 decisions (see authority note) |
| `32-pos-wireframe.md` | POS new-sale wireframe (Phase 0 proof) |

## Document authority

The SRS defines what the product must do. Architecture, database, and module documents define how it should be implemented. If two documents conflict, use this priority: business rules and transactions, database constraints, SRS, module specification, UI guidance. Confirmed Phase 0 decisions in `31-phase-0-decision-record.md` supersede earlier recommendations in module documents (for example, FIFO replaces the earlier weighted-average suggestion in `14` and the implementation plan).

## Initial product boundary

Version 1 is designed for one shop database on one Windows computer. Multiple user accounts can share the application on that computer. LAN multi-device access and cloud synchronization are future extensions and must not be simulated with unsafe shared SQLite files.

