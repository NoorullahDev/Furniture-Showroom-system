# Sales, POS, and Invoicing

## Sales workspace

Use a two-panel desktop layout: visual catalogue/search on the left and current sale summary on the right. The interface must remain clear at 1366×768 and scale to larger screens.

## Standard sale workflow

1. Start New Sale from Quick Add or shortcut.
2. Search by article number/name or browse photo/category.
3. Add individual products or furniture sets.
4. Set quantity, location, delivery need, and permitted price/discount.
5. Select existing customer or create one inline.
6. Enter delivery charge, tax if enabled, and notes.
7. Choose payment: full, partial, credit, or existing advance.
8. Review totals, stock effects, due, and delivery.
9. Confirm once; backend completes one atomic transaction.
10. Show success screen with Print Invoice, Save PDF, New Sale, and Schedule/Print Delivery Note.

## Document types

- **Quotation:** no financial or stock effect; optional expiry; convertible once.
- **Draft sale:** editable, no committed effects.
- **Confirmed invoice:** immutable commercial record.
- **Receipt:** evidence of a payment, separately numbered.
- **Delivery note:** quantities released/delivered, normally without cost.
- **Credit/return note:** controlled reversal of sold quantities/value.

## Pricing

Line total = quantity × unit price − line discount. Document discount allocation must be deterministic. Delivery charge and tax are explicit. The backend recalculates all values using checked integer arithmetic and configured rounding.

## Payment states

- Paid: due is zero.
- Partial: payment is greater than zero and less than amount payable.
- Credit: no immediate payment and a customer is required.
- Advance-funded: customer credit is allocated to the sale.
- Refunded/partially refunded: based on valid refund records.

## Customer requirement

Walk-in customer is allowed only for fully paid immediate sales without delivery. Credit, advance, scheduled delivery, quotation follow-up, and customer statement require a named customer.

## Invoice content

Shop logo/name/contact, invoice number/date, customer information, article numbers, item/set descriptions, quantities, rates, discounts, tax, delivery charge, total, paid, previous advance used, current due, payment method, delivery information, terms, creator, and optional signature areas.

## Editing and cancellation

Drafts may be edited. Posted invoices cannot. Cancellation requires permission and reason, reverses stock/reservations, customer ledger, payment/cash entries where policy allows, and delivery commitments. If money was already handed over, use refund/credit note rather than hiding history.

## Duplicate-click safety

Each completion request carries an idempotency key. Repeated submission returns the existing result or a clear conflict; it must never create duplicate invoices or payments.

