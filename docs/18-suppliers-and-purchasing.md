# Suppliers and Purchasing

## Supplier profile

Name, phone, alternate contact, address, notes, opening balance, active status, total purchases, total payments, returns/credits, and current payable.

## Purchase workflow

1. Open Record Purchase from Quick Add.
2. Select or create supplier.
3. Enter supplier invoice number and date.
4. Add products, destination locations, quantities, unit costs, discounts, tax, and freight.
5. Create missing product inline only through a controlled minimal form.
6. Enter immediate payment or leave full/partial payable.
7. Review stock and payable effects.
8. Post atomically and print purchase record/payment voucher.

## Duplicate protection

Warn or reject duplicate supplier invoice numbers for the same supplier. Completion uses an idempotency key. Purchase numbers are system-generated at posting.

## Receiving models

Simple mode posts purchase and receipt together. Advanced mode supports purchase order, partial goods receipts, and backordered quantities. Enable advanced mode only when the shop needs it.

## Cost handling

- Store the exact unit cost snapshot on each purchase line.
- Allocate freight or document discount using a documented proportional rule if it should affect inventory cost.
- Apply the FIFO layer cost update in the same transaction as stock receipt.
- Restrict cost visibility by permission.

## Supplier payment

Quick Add > Pay Supplier shows current payable and open purchases. Payment can allocate oldest-first or manually. It creates supplier ledger and cash entries atomically and generates a numbered voucher.

## Supplier returns

Select the original purchase and eligible quantities. Classify the settlement as supplier credit, replacement pending, or cash refund. Stock issue, supplier ledger, cash/refund, document state, and audit entries commit together.

## Opening balance

Opening supplier payable is entered once during setup as a specific ledger entry with date and note. It is not represented as a fake purchase.

## Supplier statement

Show opening balance, purchases, payments, returns/credits, adjustments, running payable, and closing balance. Export to PDF and CSV.

## Reports

Purchases by date/supplier/product/category, outstanding payables, payment history, purchase returns, last purchase cost, cost trend, and unpaid/overdue supplier documents.

