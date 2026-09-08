# Deliveries, Returns, Exchanges, and Damages

## Delivery workflow

1. Sale records which items require delivery.
2. Create one or more delivery schedules for eligible undelivered quantities.
3. Assign date, time window, address, contact, charge, and driver/vehicle notes.
4. Mark ready, dispatched, delivered, failed, or cancelled.
5. Capture delivery note, receiver name, and optional proof reference.

## Delivery states

`pending -> ready -> dispatched -> delivered`. `failed` can return to pending through rescheduling. `cancelled` is terminal. State transitions require valid quantities and role permission.

## Partial delivery

Delivery items reference sale lines and quantities. Total delivered plus active scheduled quantity cannot exceed sold quantity minus returned/cancelled quantity. Sale fulfilment status is derived from its delivery items.

## Stock effect

Under reservation-first policy, sale confirmation reserves items and delivery dispatch/confirmation issues stock according to the configured rule. Every state transition is defined once; the UI never decides whether stock moves.

## Sales return

1. Find invoice.
2. Select eligible lines/quantities and reason.
3. Inspect and classify each item: sellable, damaged, repair, or disposed.
4. Calculate refundable/credit value from original sale and discounts.
5. Choose refund method, customer credit, or exchange.
6. Confirm one atomic return transaction and issue return note.

## Exchange

Represent an exchange as a return linked to a new sale. Apply approved return credit to the new sale. This preserves transparent inventory, revenue, due, and cash histories.

## Damaged stock

Damage record includes product, quantity, location, source, reason, photo/attachment reference, estimated loss, decision, user, and time. Moving to damaged status removes it from available stock.

Outcomes:

- Repaired and returned to sellable stock.
- Sold as damaged with clear pricing and permission.
- Returned to supplier.
- Written off/disposed with approval.

## Controls

- Return quantity cannot exceed net sold quantity.
- Refund cannot exceed refundable paid value.
- A delivered item cannot be silently cancelled.
- Every return, exchange, damage, write-off, or failed delivery records an audit event.

## Reports

Pending/late deliveries, deliveries by date/status, failed attempts, sales returns, refund value, damage by reason/product, write-off loss, and items awaiting repair or supplier action.

