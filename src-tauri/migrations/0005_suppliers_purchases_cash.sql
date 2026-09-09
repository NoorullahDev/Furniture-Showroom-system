-- Furniture Shop Management System — Phase 5: suppliers, purchases, payables, and cash.
-- Purchases and returns are immutable, posted documents. Stock effects reuse the
-- Phase 4 movement/balance engine; payables live in an append-only supplier ledger;
-- cash is an append-only entry ledger with a transactionally maintained balance.
-- Money is stored as signed 64-bit integer minor units (paisa), never floats.

-- Suppliers with a normalized unique code and an account opening balance.
CREATE TABLE IF NOT EXISTS suppliers (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    code                  TEXT NOT NULL UNIQUE CHECK (code = UPPER(code)),
    name                  TEXT NOT NULL,
    phone                 TEXT,
    email                 TEXT,
    address               TEXT,
    opening_balance_minor INTEGER NOT NULL DEFAULT 0,
    is_active             INTEGER NOT NULL DEFAULT 1,
    created_by            INTEGER REFERENCES users(id),
    created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_suppliers_active ON suppliers(is_active);

-- Append-only supplier ledger. Signed balances: a positive amount raises the
-- payable owed to the supplier; a negative amount lowers it. balance_after is
-- the running balance after this entry and must equal the sum of all entries.
CREATE TABLE IF NOT EXISTS supplier_ledger_entries (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    supplier_id      INTEGER NOT NULL REFERENCES suppliers(id),
    entry_type       TEXT    NOT NULL CHECK (entry_type IN (
                        'opening_balance', 'invoice', 'payment', 'return', 'void'
                     )),
    document_type    TEXT,
    document_id      INTEGER,
    amount_minor     INTEGER NOT NULL CHECK (amount_minor != 0),
    balance_after_minor INTEGER NOT NULL,
    notes            TEXT,
    created_by       INTEGER NOT NULL REFERENCES users(id),
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_supplier_ledger_supplier ON supplier_ledger_entries(supplier_id);

-- Payment methods and cash accounts.
CREATE TABLE IF NOT EXISTS payment_methods (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    code        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    is_active   INTEGER NOT NULL DEFAULT 1
);
INSERT OR IGNORE INTO payment_methods (code, name) VALUES
  ('cash', 'Cash'),
  ('bank_transfer', 'Bank transfer'),
  ('cheque', 'Cheque'),
  ('card', 'Card');

CREATE TABLE IF NOT EXISTS cash_accounts (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    code                  TEXT NOT NULL UNIQUE,
    name                  TEXT NOT NULL,
    kind                  TEXT NOT NULL DEFAULT 'cash' CHECK (kind IN ('cash', 'bank')),
    opening_balance_minor INTEGER NOT NULL DEFAULT 0,
    balance_minor         INTEGER NOT NULL DEFAULT 0,
    is_active             INTEGER NOT NULL DEFAULT 1,
    created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT OR IGNORE INTO cash_accounts (code, name, kind, opening_balance_minor, balance_minor)
VALUES ('main_cash', 'Main Cash', 'cash', 0, 0);

-- Append-only cash ledger. + amount is a cash inflow, - amount is an outflow.
-- balance_minor on cash_accounts is maintained in the same write and is always
-- rebuildable as opening_balance + SUM(cash_entries.amount_minor).
CREATE TABLE IF NOT EXISTS cash_entries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    cash_account_id INTEGER NOT NULL REFERENCES cash_accounts(id),
    entry_type      TEXT    NOT NULL CHECK (entry_type IN (
                        'opening', 'purchase_payment', 'payment_void', 'supplier_refund'
                     )),
    amount_minor    INTEGER NOT NULL CHECK (amount_minor != 0),
    reference_type  TEXT,
    reference_id    INTEGER,
    reason          TEXT,
    created_by      INTEGER NOT NULL REFERENCES users(id),
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_cash_entries_account ON cash_entries(cash_account_id);
CREATE INDEX IF NOT EXISTS idx_cash_entries_ref ON cash_entries(reference_type, reference_id);

-- Purchases. A draft carries items and supplier-invoice details but no document
-- number; posting allocates the number, posts stock/cost/ledger/cash, and sets
-- the status to 'posted'. Idempotency keys make posting safe to retry.
CREATE TABLE IF NOT EXISTS purchases (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    purchase_number  TEXT,
    supplier_id      INTEGER NOT NULL REFERENCES suppliers(id),
    supplier_name    TEXT NOT NULL,
    location_id      INTEGER NOT NULL REFERENCES locations(id),
    invoice_number   TEXT NOT NULL,
    invoice_date     TEXT NOT NULL,
    purchase_date    TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted')),
    total_minor      INTEGER NOT NULL DEFAULT 0,
    paid_minor       INTEGER NOT NULL DEFAULT 0,
    due_minor        INTEGER NOT NULL DEFAULT 0,
    notes            TEXT,
    idempotency_key  TEXT,
    posted_by        INTEGER REFERENCES users(id),
    posted_at        TEXT,
    created_by       INTEGER NOT NULL REFERENCES users(id),
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_purchases_number
    ON purchases(purchase_number) WHERE purchase_number IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_purchases_idempotency
    ON purchases(idempotency_key) WHERE idempotency_key IS NOT NULL;
-- Duplicate supplier-invoice rejection: the same invoice may only be recorded
-- once per supplier (a team also stores the planned/entered state).
CREATE UNIQUE INDEX IF NOT EXISTS idx_purchases_invoice_supplier
    ON purchases(supplier_id, invoice_number);

CREATE TABLE IF NOT EXISTS purchase_items (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    purchase_id      INTEGER NOT NULL REFERENCES purchases(id) ON DELETE CASCADE,
    product_id       INTEGER NOT NULL REFERENCES products(id),
    article_number   TEXT NOT NULL,
    product_name     TEXT NOT NULL,
    quantity         INTEGER NOT NULL CHECK (quantity > 0),
    unit_cost_minor  INTEGER NOT NULL CHECK (unit_cost_minor >= 0),
    line_total_minor INTEGER NOT NULL CHECK (line_total_minor >= 0)
);
CREATE INDEX IF NOT EXISTS idx_purchase_items_purchase ON purchase_items(purchase_id);

-- Supplier payments with oldest-first allocation to open invoices.
CREATE TABLE IF NOT EXISTS supplier_payments (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    payment_number   TEXT,
    supplier_id      INTEGER NOT NULL REFERENCES suppliers(id),
    payment_method_id INTEGER NOT NULL REFERENCES payment_methods(id),
    cash_account_id  INTEGER NOT NULL REFERENCES cash_accounts(id),
    payment_date     TEXT NOT NULL,
    amount_minor     INTEGER NOT NULL CHECK (amount_minor > 0),
    status           TEXT NOT NULL DEFAULT 'posted' CHECK (status IN ('posted', 'voided')),
    notes            TEXT,
    idempotency_key  TEXT,
    voided_by        INTEGER REFERENCES users(id),
    voided_at        TEXT,
    created_by       INTEGER NOT NULL REFERENCES users(id),
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_supplier_payments_number
    ON supplier_payments(payment_number) WHERE payment_number IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_supplier_payments_idempotency
    ON supplier_payments(idempotency_key) WHERE idempotency_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_supplier_payments_supplier ON supplier_payments(supplier_id);

CREATE TABLE IF NOT EXISTS supplier_payment_allocations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    payment_id      INTEGER NOT NULL REFERENCES supplier_payments(id) ON DELETE CASCADE,
    purchase_id     INTEGER NOT NULL REFERENCES purchases(id),
    amount_minor    INTEGER NOT NULL CHECK (amount_minor > 0)
);
CREATE INDEX IF NOT EXISTS idx_payment_allocations_payment ON supplier_payment_allocations(payment_id);

-- Supplier returns. Return quantities are capped by net received per product;
-- posting reduces payable (oldest-first across open invoices) and optionally
-- refunds cash.
CREATE TABLE IF NOT EXISTS supplier_returns (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    return_number        TEXT,
    supplier_id          INTEGER NOT NULL REFERENCES suppliers(id),
    supplier_name        TEXT NOT NULL,
    purchase_id          INTEGER REFERENCES purchases(id),
    location_id          INTEGER NOT NULL REFERENCES locations(id),
    return_date          TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted')),
    total_minor          INTEGER NOT NULL DEFAULT 0,
    refund_minor         INTEGER NOT NULL DEFAULT 0,
    due_reduction_minor  INTEGER NOT NULL DEFAULT 0,
    notes                TEXT,
    idempotency_key      TEXT,
    posted_by            INTEGER REFERENCES users(id),
    posted_at            TEXT,
    created_by           INTEGER NOT NULL REFERENCES users(id),
    created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_supplier_returns_number
    ON supplier_returns(return_number) WHERE return_number IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_supplier_returns_idempotency
    ON supplier_returns(idempotency_key) WHERE idempotency_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_supplier_returns_supplier ON supplier_returns(supplier_id);

CREATE TABLE IF NOT EXISTS supplier_return_items (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    return_id        INTEGER NOT NULL REFERENCES supplier_returns(id) ON DELETE CASCADE,
    product_id       INTEGER NOT NULL REFERENCES products(id),
    article_number   TEXT NOT NULL,
    product_name     TEXT NOT NULL,
    quantity         INTEGER NOT NULL CHECK (quantity > 0),
    unit_cost_minor  INTEGER NOT NULL CHECK (unit_cost_minor >= 0),
    line_total_minor INTEGER NOT NULL CHECK (line_total_minor >= 0)
);
CREATE INDEX IF NOT EXISTS idx_return_items_return ON supplier_return_items(return_id);

-- Per-document number sequences for posted purchase documents.
INSERT OR IGNORE INTO document_sequences (document_type, next_value, prefix, suffix) VALUES
  ('purchase', 1, 'PUR-', ''),
  ('supplier_payment', 1, 'PAY-', ''),
  ('supplier_return', 1, 'SRN-', '');

-- Phase 5 action-level permissions.
INSERT OR IGNORE INTO permissions (code, description) VALUES
  ('supplier.create', 'Create and edit suppliers'),
  ('purchase.create', 'Create and post purchases'),
  ('supplier.return', 'Post supplier returns'),
  ('payable.view', 'View supplier balances and payable aging');

-- Role template grants. Owner joins manager so every new permission stays in
-- the owner role (the owner-wide grant in 0002 applied only to then-existing
-- permissions).
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('supplier.create', 'purchase.create', 'supplier.return', 'payable.view')
WHERE r.code IN ('owner', 'manager');

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('purchase.create', 'payable.view')
WHERE r.code = 'accountant';