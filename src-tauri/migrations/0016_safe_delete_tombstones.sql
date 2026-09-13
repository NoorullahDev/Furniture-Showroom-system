-- Audit-preserving deletion for party master records that still have linked
-- business history. Unlinked records continue to be physically deleted.
ALTER TABLE customers ADD COLUMN deleted_at TEXT;
ALTER TABLE suppliers ADD COLUMN deleted_at TEXT;
ALTER TABLE stock_movements ADD COLUMN deleted_at TEXT;

CREATE INDEX IF NOT EXISTS idx_customers_deleted ON customers(deleted_at);
CREATE INDEX IF NOT EXISTS idx_suppliers_deleted ON suppliers(deleted_at);
CREATE INDEX IF NOT EXISTS idx_stock_movements_deleted ON stock_movements(deleted_at);
