# Business Rules and Atomic Transactions

## Universal rules

1. Rust is authoritative for totals, permissions, state transitions, stock, and ledger effects.
2. Money uses integer minor units and checked arithmetic.
3. Posted financial documents are immutable; corrections use reversal records.
4. A transaction either commits every related change or commits nothing.
5. Client-supplied totals, roles, costs, balances, and document numbers are ignored and recalculated.

## Completing a sale

Within one SQLite transaction:

1. Authorize the user and validate document status.
2. Load current products, bundle components, prices, tax settings, and stock.
3. Recalculate line totals, discount, tax, delivery charge, grand total, payment, and due.
4. Lock the logical workflow through the application write coordinator.
5. Validate component availability by product and location.
6. Assign the next invoice number.
7. Insert sale and snapshot line/component data.
8. Insert stock movements or reservations according to fulfilment policy.
9. Insert payment, customer ledger, and cash entries.
10. Insert delivery record if required.
11. Insert audit event and commit.

Any failure rolls back all steps.

## Sale status model

`draft -> confirmed -> partially_delivered -> delivered -> completed` with controlled `cancelled` and `returned` outcomes. A confirmed sale cannot return to draft. Cancellation creates reversals for any stock, money, ledger, or delivery effects.

## Payment rules

- A customer payment may be allocated to one invoice or remain unallocated as advance credit.
- Customer due = posted sales debits minus valid payments/credits/refunds, never a manually editable field.
- Overpayment becomes customer advance unless the user explicitly records an immediate refund.
- Supplier payment uses the equivalent payable rules.
- Voiding a payment creates reversing ledger and cash entries.

## Stock rules

- Stock on hand = sum of committed quantity deltas.
- Available = on hand - active reservations - non-sellable damaged quantity.
- Bundle availability = minimum floor of each component availability divided by required component quantity.
- Manual adjustments require product, location, quantity delta, reason, and authorized user.
- Historical movements are never edited or deleted.

## Purchase posting

Posting/receiving a purchase atomically creates the purchase snapshot, stock receipt movements, supplier ledger debit/credit according to ledger convention, payment/cash entries, updated product cost policy, and audit record.

## Returns

A return references original lines. It cannot exceed the net quantity previously sold or purchased. The user classifies returned stock as sellable, damaged, or disposed. Refund, customer/supplier ledger, cash, stock, document totals, and audit effects commit together.

## Document numbering

Sequences are assigned only during posting inside the transaction. Numbers are unique per document type and financial year/configuration, never reused after cancellation.

## Concurrency

Use a single application write coordinator plus short database transactions. Never hold a transaction open while showing a dialog, selecting a file, generating a PDF, or waiting for user input.

