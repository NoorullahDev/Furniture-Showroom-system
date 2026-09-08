# Acceptance Criteria and User Acceptance Testing

## Release definition of done

- All required workflows implemented and authorized in Rust.
- Database constraints, migrations, and atomic tests pass.
- Documents and reports reconcile with source records.
- Backup and clean-machine restore succeed.
- Windows installer, upgrade, uninstall-with-data-preservation policy, and scaling are tested.
- No open Critical/High defects.

## Catalogue acceptance

- User can create dynamic category/type and product with unique article number and multiple valid images.
- Duplicate normalized article number is rejected.
- Primary image appears in grid, picker, and sale line.
- Archived product remains on historical invoices and cannot be newly sold.
- Exact article search returns correct product quickly.

## Inventory acceptance

- Opening stock, purchase, sale/delivery, return, transfer, damage, and adjustment create traceable movements.
- Displayed balance equals movement sum.
- Negative stock is blocked by default.
- Low-stock list matches configured thresholds.

## Bundle acceptance

- Set availability reflects its limiting component.
- Custom set price does not alter component retail prices.
- Selling one set affects each component by required quantity once.
- Later bundle edits do not alter old invoices.

## Sales/customer acceptance

- Cash, partial, credit, and advance-funded sales calculate correct paid/due values.
- Credit/delivery sale requires a customer.
- Duplicate confirmation does not create a second sale.
- Receipt and invoice show correct totals.
- Later payment reduces customer due and appears on statement/cash book.
- Cancellation/return creates explicit reversals and audit history.

## Purchase/supplier acceptance

- Posted purchase increases correct stock/location and payable.
- Payment reduces payable and cash account.
- Supplier return reverses eligible stock and creates correct settlement.
- Supplier statement matches underlying ledger.

## Operations acceptance

- Partial deliveries cannot exceed sold quantity.
- Return cannot exceed net sold quantity or refund allowance.
- Damaged items are excluded from available stock.
- Expense affects cash and operational profit exactly once.

## Security acceptance

- Unauthorized menu is hidden and direct command still returns forbidden.
- Password hashes are not readable through UI/log/export.
- Locked session rejects commands.
- Sensitive change includes actor, timestamp, reason, and before/after audit where appropriate.

## UAT script

Use realistic demo products/photos. Owner, salesperson, accountant, and storekeeper each execute their normal day. Reconcile physical stock sample, customer due, supplier payable, cash balance, and daily profit manually. Record pass/fail, evidence, severity, owner, and retest result.

