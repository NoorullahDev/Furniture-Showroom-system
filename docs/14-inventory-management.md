# Inventory Management

## Stock model

Stock is a ledger of immutable movements, not a manually overwritten number. Current quantities are derived or cached from those movements.

## Quantities

- **On hand:** physically controlled stock.
- **Reserved:** committed to confirmed orders but not released.
- **Available:** on hand minus active reservations and non-sellable stock.
- **Damaged:** physically present but unavailable for normal sale.
- **In transit:** moving between configured locations.

## Movement types

Opening stock, purchase receipt, sale issue, customer return, supplier return, location transfer out/in, reservation/release, damage, repair recovery, manual adjustment, cancellation reversal, and stock count correction.

Every movement includes product, location, signed quantity, time, user, source document, and reason where needed.

## Receiving

Purchases may be ordered and received together for simple shops, or partially received when advanced mode is enabled. Only posted goods receipts increase stock. Unit cost snapshots support inventory valuation and profit reporting.

## Reservation and delivery policy

Recommended default: confirmed credit/order sales reserve stock; stock leaves on delivery. Immediate counter sales issue stock at confirmation. The shop can choose one documented fulfilment policy during setup, and historical documents retain their behavior.

## Stock adjustment

The user selects product, location, counted quantity or delta, reason, and notes. The system shows current and resulting stock before confirmation. Large or negative-impact adjustments can require manager approval.

## Stock count

1. Create count session by location/category.
2. Freeze the expected snapshot but allow sales to continue through tracked movements.
3. Enter physical counts.
4. Calculate variance at posting time.
5. Post correction movements with audit reference.

## Transfers

A transfer creates paired out/in effects. With transit tracking: dispatch moves quantity into in-transit; receive moves it into destination; cancellation reverses outstanding movement.

## Negative stock

Always blocked in Version 1 (decision D4). Sales and transfers beyond available
quantity are rejected with `INSUFFICIENT_STOCK`; no permission or reason path
allows negative on-hand. Bundle components follow the same rule.

## Inventory valuation

FIFO layered costing for Version 1 (decision D6). Purchases and stock receipts
create cost layers; issue consumes the oldest layers first. The algorithm must
be documented, deterministic, and tested for purchases, returns, reservations,
damage, and negative-stock restrictions. Historical sale lines keep cost
snapshots. Return re-entry rules (restore at original cost layer) are specified
in Phase 4.

## Alerts

Low stock occurs when available quantity is at or below product minimum. Dashboard alerts link to filtered inventory and purchase creation. Dismissal does not change actual stock status.

