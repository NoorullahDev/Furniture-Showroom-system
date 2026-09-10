-- Furniture Shop Management System — Phase 10: dashboard rollups and global search.
-- Adds composite status+date indexes so daily/weekly dashboard figures and the
-- permission-filtered summary stay small reads, plus name/phone indexes for the
-- global search palette. All additive; released migrations are never edited.

CREATE INDEX IF NOT EXISTS idx_sales_status_date
    ON sales(status, sale_date);

CREATE INDEX IF NOT EXISTS idx_sales_returns_status_date
    ON sales_returns(status, return_date);

CREATE INDEX IF NOT EXISTS idx_customer_payments_status_date
    ON customer_payments(status, payment_date);

CREATE INDEX IF NOT EXISTS idx_supplier_payments_status_date
    ON supplier_payments(status, payment_date);

CREATE INDEX IF NOT EXISTS idx_customers_name ON customers(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_customers_phone ON customers(phone COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_suppliers_name ON suppliers(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_suppliers_phone ON suppliers(phone COLLATE NOCASE);

CREATE INDEX IF NOT EXISTS idx_audit_actor_created ON audit_logs(user_id, id);