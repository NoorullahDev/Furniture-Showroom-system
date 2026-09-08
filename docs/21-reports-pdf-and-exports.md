# Reports, PDF, Printing, and Exports

## Report catalogue

### Sales

Daily/weekly/monthly/yearly sales, invoice register, sales by product/article/category/customer/user/payment method, discounts, returns, bundle sales, gross profit and margin, best/slow sellers.

### Inventory

Current stock, stock movement ledger, low/out of stock, stock by location, stock valuation, reservations, damaged/repair/write-off stock, stock adjustments, count variance, stock aging where dates permit.

### Customers

Customer balances, overdue dues, due-aging buckets, receipts, advances, statements, credit-limit exceptions, customer purchase history.

### Suppliers and purchases

Purchases by supplier/product/category, supplier payables, payable aging, payments, returns, supplier statements, last cost and cost trend.

### Operations and finance

Pending/late deliveries, delivery performance, expenses, cash book, account balances, gross profit, operational net profit, user activity, reversals, and audit exceptions.

## Common filters

Date range with presets, status, location, category, product/article, customer/supplier, user, payment method, document number, and amount range. Active filters appear in both screen and exported output.

## Report behavior

- Default to meaningful safe date ranges, not all history.
- Show totals and record count.
- Allow drill-down from summary to source documents.
- Large reports load in pages and export through background jobs.
- Cancelled/reversed entries are excluded by default but available through status filters.

## PDF standards

- A4 portrait for invoices/statements; landscape for wide reports.
- Embed a Unicode font supporting English and Urdu if Urdu is enabled.
- Repeat headers and page numbers.
- Include shop identity, report title, filters, generated time, generator user, and totals.
- Use consistent PKR and date formatting.
- Prevent rows and totals from being clipped at page boundaries.

## Documents

Templates: sales invoice, quotation, receipt, credit/return note, delivery note, purchase record, supplier payment voucher, customer statement, supplier statement, and management report.

## Export safety

Users choose a destination through a controlled save dialog. Suggested filenames contain document/report name and date but no unnecessary personal data. CSV output neutralizes spreadsheet formula injection by safely prefixing dangerous leading characters.

## Print workflow

Preview first, then select configured printer/page format. Support A4 initially; optional thermal receipt format is a future/configurable template. A print failure never rolls back an already completed business transaction; the user can reprint from document history.

## Accuracy tests

For every report, compare sample totals to source ledger/movement queries, test empty/single/multi-page output, extreme amounts, long names, images, Unicode, cancelled records, date boundaries, and refunds.

