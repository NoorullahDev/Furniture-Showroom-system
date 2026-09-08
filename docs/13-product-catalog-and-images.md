# Product Catalogue and Images

## Product model

Each sellable stock item is a product with a unique article number. Categories and product types are user-managed. Flexible attributes cover furniture-specific details without database changes.

## Required fields

- Article number: entered by user, trimmed and normalized for uniqueness.
- Product name.
- Category.
- Selling price.
- Stock-tracking choice.

## Optional fields

Product type, description, purchase cost, minimum stock, material, color, dimensions, brand, supplier reference, location, tax profile, barcode, warranty, and notes.

## Article-number rules

- Preserve the displayed form but maintain a normalized value for case-insensitive lookup.
- Reject duplicates before save and again with a database unique constraint.
- Article numbers are stable after transactions exist; changing one requires permission and audit.
- Invoices keep the sale-time article snapshot.

## Images

- Up to a configurable limit, recommended 8 images per product.
- Accept JPEG, PNG, and WebP after decoding and validation.
- Generate a small grid thumbnail and medium preview; retain an optimized original.
- Strip unnecessary metadata, correct orientation, and cap dimensions/file size.
- Store files under app data using generated names; store relative paths and hashes in SQLite.
- One image is primary; reorder through drag and drop or accessible controls.
- Delete an image only after confirming it is not referenced; cleanup happens safely after database commit.

## Catalogue views

### Grid

Primary photo, name, article number, price, category, and stock badge. Best for sales and visual identification.

### Table

Article, product, category/type, cost if permitted, price, on hand, reserved, available, and status. Best for administration.

## Search

Exact article matches rank first, followed by prefix matches, product name, category, type, and attributes. Normalize case and whitespace. A future barcode scanner should populate the same search field.

## Variants

For Version 1, each independently stocked color/size combination should be its own product/article number. A parent `style/group` reference can visually group variants. This is simpler and prevents ambiguous stock.

## Archive and duplicate

Referenced products cannot be hard-deleted. Archive hides them from new sales while retaining history. `Duplicate product` copies descriptive data and selected images but requires a new article number and starts with zero stock.

## Bulk operations

CSV import/export may create or update master data, but images are added separately. Import validates the whole file and presents an error report before commit. Opening stock import produces stock movements, not direct balance edits.

