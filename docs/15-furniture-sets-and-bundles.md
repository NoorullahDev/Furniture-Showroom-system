# Furniture Sets and Bundles

## Purpose

A bundle sells multiple existing products as one named package with one custom price while preserving correct component inventory.

## Example

`Modern Bedroom Set` contains 1 bed, 2 side tables, 1 dressing table, and 1 cupboard. The components may total PKR 250,000 individually while the default set price is PKR 225,000.

## Bundle master data

- Unique set code and name.
- Cover image, description, active status.
- Two or more component products and quantities.
- Default set price.
- Optional validity dates and internal notes.
- Derived available-set count based on component stock.

## Sale behavior

- The salesperson selects the bundle as one line.
- UI can expand it to show components.
- Authorized users may set a custom sale price/discount.
- Rust validates every component and quantity.
- Sale snapshots the set name, custom price, and components.
- Stock/reservations apply to components, never to a fictional bundle stock count.

## Availability calculation

For each component, calculate `floor(available quantity / required quantity)`. Bundle availability is the minimum result. A bundle containing a non-stock service item ignores that item for availability.

## Cost and profit

Bundle cost is the sum of component cost snapshots multiplied by component quantities. Revenue is the bundle line total. Gross profit is bundle revenue minus component cost. Do not distribute revenue among components unless a component-level sales report requires an explicit allocation policy.

## Price control

The master default price may differ from component retail total. The sale screen shows both and the savings. Price below configurable cost/margin threshold warns the user and can require manager approval.

## Changes and archive

Changing a bundle affects future sales only. Past sale snapshots remain unchanged. A bundle referenced in history is archived, not deleted.

## Ad-hoc packages

A salesperson with permission may group selected products into an unnamed one-time package and assign a package price. The system records all component quantities and the pricing approval. It does not silently create permanent bundle master data.

## Returns

Users may return the entire set or permitted components. Refund calculation follows the recorded return policy and cannot exceed net paid/sold amounts. Each returned component is classified as sellable, damaged, or disposed.

