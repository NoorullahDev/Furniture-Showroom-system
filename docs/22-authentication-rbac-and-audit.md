# Authentication, RBAC, and Audit

## Authentication

- Local username and password.
- Passwords hashed with Argon2id and unique salts.
- First-run owner account creation requires a strong password.
- Configurable inactivity lock; explicit logout.
- Rate-limit failed login attempts locally and audit repeated failures.
- Owner-controlled password reset; reset never reveals the old password.

## Roles

Default roles are templates and remain editable:

| Role | Typical access |
| --- | --- |
| Owner | All modules, settings, backup, licensing, audit |
| Manager | Operations, approvals, reports; limited security settings |
| Salesperson | Catalogue, customer entry, quotations, sales, receipts within limits |
| Accountant | Payments, expenses, statements, financial reports |
| Storekeeper | Products, receipts, stock, transfers, damage, delivery preparation |

## Permission model

Use action-level permissions such as `product.create`, `product.cost.view`, `sale.create`, `sale.discount.override`, `sale.cancel`, `payment.void`, `inventory.adjust`, `supplier.pay`, `profit.view`, `report.export`, `user.manage`, `backup.restore`, and `audit.view`.

Permission checks occur in every Rust command/use case. UI checks only simplify navigation.

## Approval rules

Configurable thresholds may require manager/owner authentication for:

- Selling below cost or minimum margin.
- Discount above user limit.
- Negative stock override.
- Large stock adjustment or write-off.
- Backdated transaction.
- Refund, payment void, or invoice cancellation.

The approving user must be different from the acting user when dual approval is enabled.

## Audit log

Record login outcome, user/role changes, product/article/cost changes, document posting/cancellation, price overrides, payment/refund, stock adjustment, return/damage/write-off, setting change, export of sensitive data, backup, restore, and license change.

Fields: actor, action, entity type/id, timestamp, device/app version, reason, before/after JSON with sensitive-field redaction, approval user, and correlation ID.

## Audit integrity

Normal application roles cannot edit or delete audit events. Consider a hash chain between events for tamper evidence. Backup includes audit history. Audit export requires explicit permission.

## Session model

Keep session state only in Rust. The frontend receives a safe user profile and permission list, never password hashes or internal secrets. A locked session rejects commands even if the screen remains open.

## Deactivation

Users referenced by history are deactivated, not deleted. Revocation invalidates active sessions immediately.

