-- Phase 7: Customer due control.
-- Credit terms on customers (used to compute invoice due dates) and a stored
-- due date snapshot on each confirmed sale. Both are backward-compatible
-- additive column changes; no table rebuilds required.

ALTER TABLE customers ADD COLUMN credit_days INTEGER NOT NULL DEFAULT 30;

ALTER TABLE sales ADD COLUMN due_date TEXT;

-- Backfill due dates for already-confirmed sales using the default terms.
UPDATE sales
   SET due_date = date(sale_date, '+30 days')
 WHERE status = 'confirmed' AND due_date IS NULL;

CREATE INDEX IF NOT EXISTS idx_sales_due_control
    ON sales(customer_id, due_date)
 WHERE status = 'confirmed' AND due_minor > 0;