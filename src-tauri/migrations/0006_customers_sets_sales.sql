-- Furniture Shop Management System — Phase 6: customers, furniture sets, sales POS, invoicing.
-- Customers carry an append-only ledger with due/advance semantics; furniture sets (bundles)
-- are priced units with component stock; sales are immutable posted documents with price/cost
-- snapshots. Money is stored as signed 64-bit integer minor units (paisa), never floats.

-- Customers. A positive ledger balance is money the customer owes (due); a negative balance
-- is a prepayment advance.
CREATE TABLE IF NOT EXISTS customers (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    code               TEXT NOT NULL UNIQUE CHECK (code = UPPER(code)),
    name               TEXT NOT NULL,
    phone              TEXT,
    email              TEXT,
    address            TEXT,
    credit_limit_minor INTEGER NOT NULL DEFAULT 0,
    opening_balance_minor INTEGER NOT NULL DEFAULT 0,
    is_active          INTEGER NOT NULL DEFAULT 1,
    created_by         INTEGER REFERENCES users(id),
    created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_customers_active ON customers(is_active);

-- Append-only customer ledger. Signed amounts; balance_after_minor is the running balance.
CREATE TABLE IF NOT EXISTS customer_ledger_entries (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id        INTEGER NOT NULL REFERENCES customers(id),
    entry_type         TEXT    NOT NULL CHECK (entry_type IN (
                           'opening_balance', 'sale', 'payment', 'advance_used',
                           'sale_cancellation', 'payment_refund', 'advance_restore'
                        )),
    document_type      TEXT,
    document_id        INTEGER,
    amount_minor       INTEGER NOT NULL CHECK (amount_minor != 0),
    balance_after_minor INTEGER NOT NULL,
    notes              TEXT,
    created_by         INTEGER NOT NULL REFERENCES users(id),
    created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_customer_ledger_customer ON customer_ledger_entries(customer_id);

-- Sales: draft/quotation → confirmed → (cancelled). Posting is atomic and idempotent.
CREATE TABLE IF NOT EXISTS sales (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_number          TEXT,
    kind                 TEXT NOT NULL DEFAULT 'sale' CHECK (kind IN ('sale', 'quote')),
    customer_id          INTEGER REFERENCES customers(id),
    customer_name        TEXT,
    location_id          INTEGER NOT NULL REFERENCES locations(id),
    sale_date            TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'draft' CHECK (
                             status IN ('draft', 'quotation', 'confirmed', 'cancelled')
                          ),
    subtotal_minor       INTEGER NOT NULL DEFAULT 0,
    discount_minor       INTEGER NOT NULL DEFAULT 0,
    delivery_charge_minor INTEGER NOT NULL DEFAULT 0,
    tax_minor            INTEGER NOT NULL DEFAULT 0,
    total_minor          INTEGER NOT NULL DEFAULT 0,
    paid_minor           INTEGER NOT NULL DEFAULT 0,
    advance_used_minor   INTEGER NOT NULL DEFAULT 0,
    due_minor            INTEGER NOT NULL DEFAULT 0,
    cost_minor           INTEGER NOT NULL DEFAULT 0,
    notes                TEXT,
    idempotency_key      TEXT,
    confirmed_by         INTEGER REFERENCES users(id),
    confirmed_at         TEXT,
    cancelled_by         INTEGER REFERENCES users(id),
    cancelled_at         TEXT,
    created_by           INTEGER NOT NULL REFERENCES users(id),
    created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_sales_number ON sales(sale_number) WHERE sale_number IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_sales_idempotency ON sales(idempotency_key) WHERE idempotency_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_sales_customer ON sales(customer_id);
CREATE INDEX IF NOT EXISTS idx_sales_status ON sales(status);

-- Sale lines. A line is either an individual product or a furniture set; set lines
-- carry component stock rows in sale_item_components. Price/cost snapshots always.
CREATE TABLE IF NOT EXISTS sale_items (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_id          INTEGER NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
    sort_order       INTEGER NOT NULL DEFAULT 0,
    product_id       INTEGER REFERENCES products(id),
    bundle_id        INTEGER REFERENCES bundles(id),
    article_number   TEXT NOT NULL,
    product_name     TEXT NOT NULL,
    quantity         INTEGER NOT NULL CHECK (quantity > 0),
    unit_price_minor INTEGER NOT NULL CHECK (unit_price_minor >= 0),
    line_total_minor INTEGER NOT NULL CHECK (line_total_minor >= 0),
    unit_cost_minor  INTEGER NOT NULL DEFAULT 0,
    line_cost_minor  INTEGER NOT NULL DEFAULT 0,
    CHECK (product_id IS NOT NULL OR bundle_id IS NOT NULL)
);
CREATE INDEX IF NOT EXISTS idx_sale_items_sale ON sale_items(sale_id);

CREATE TABLE IF NOT EXISTS sale_item_components (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_item_id     INTEGER NOT NULL REFERENCES sale_items(id) ON DELETE CASCADE,
    product_id       INTEGER NOT NULL REFERENCES products(id),
    article_number   TEXT NOT NULL,
    product_name     TEXT NOT NULL,
    quantity         INTEGER NOT NULL CHECK (quantity > 0),
    unit_cost_minor  INTEGER NOT NULL DEFAULT 0,
    line_cost_minor  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_sale_item_components_item ON sale_item_components(sale_item_id);

-- Money received from customers. Posted at sale confirmation (sale_id set) or as a
-- standalone receipt (sale_id NULL, allocations apply oldest-first / remainder advance).
CREATE TABLE IF NOT EXISTS customer_payments (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    receipt_number  TEXT,
    customer_id     INTEGER NOT NULL REFERENCES customers(id),
    sale_id         INTEGER REFERENCES sales(id),
    payment_method_id INTEGER NOT NULL REFERENCES payment_methods(id),
    cash_account_id INTEGER NOT NULL REFERENCES cash_accounts(id),
    payment_date    TEXT NOT NULL,
    amount_minor    INTEGER NOT NULL CHECK (amount_minor > 0),
    advance_alloc_minor INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'posted' CHECK (status IN ('posted', 'voided')),
    notes           TEXT,
    idempotency_key TEXT,
    created_by      INTEGER NOT NULL REFERENCES users(id),
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    voided_by       INTEGER REFERENCES users(id),
    voided_at       TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_customer_payments_number
    ON customer_payments(receipt_number) WHERE receipt_number IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_customer_payments_idempotency
    ON customer_payments(idempotency_key) WHERE idempotency_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_customer_payments_customer ON customer_payments(customer_id);

CREATE TABLE IF NOT EXISTS customer_payment_allocations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    payment_id  INTEGER NOT NULL REFERENCES customer_payments(id) ON DELETE CASCADE,
    sale_id     INTEGER NOT NULL REFERENCES sales(id),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0)
);
CREATE INDEX IF NOT EXISTS idx_customer_payment_alloc_payment
    ON customer_payment_allocations(payment_id);

-- Furniture sets (bundles): a priced bundle of stock components.
CREATE TABLE IF NOT EXISTS bundles (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    code              TEXT NOT NULL UNIQUE CHECK (code = UPPER(code)),
    name              TEXT NOT NULL,
    description       TEXT,
    cover_image_path  TEXT,
    default_price_minor INTEGER NOT NULL DEFAULT 0 CHECK (default_price_minor >= 0),
    is_active         INTEGER NOT NULL DEFAULT 1,
    created_by        INTEGER REFERENCES users(id),
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS bundle_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    bundle_id   INTEGER NOT NULL REFERENCES bundles(id) ON DELETE CASCADE,
    product_id  INTEGER NOT NULL REFERENCES products(id),
    quantity    INTEGER NOT NULL CHECK (quantity > 0),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    UNIQUE (bundle_id, product_id)
);

-- Extend the cash-entry type set for customer money (rebuild of the Phase 5 table).
ALTER TABLE cash_entries RENAME TO cash_entries_0006_old;
CREATE TABLE cash_entries (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    cash_account_id  INTEGER NOT NULL REFERENCES cash_accounts(id),
    entry_type       TEXT    NOT NULL CHECK (entry_type IN (
                        'opening', 'purchase_payment', 'payment_void', 'supplier_refund',
                        'sale_payment', 'sale_refund', 'customer_receipt', 'receipt_void'
                     )),
    amount_minor     INTEGER NOT NULL CHECK (amount_minor != 0),
    reference_type   TEXT,
    reference_id     INTEGER,
    reason           TEXT,
    created_by       INTEGER NOT NULL REFERENCES users(id),
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO cash_entries (id, cash_account_id, entry_type, amount_minor, reference_type,
                          reference_id, reason, created_by, created_at)
SELECT id, cash_account_id, entry_type, amount_minor, reference_type,
       reference_id, reason, created_by, created_at
  FROM cash_entries_0006_old;
DROP TABLE cash_entries_0006_old;
CREATE INDEX IF NOT EXISTS idx_cash_entries_account ON cash_entries(cash_account_id);
-- Maintain the invariant documented at creation: balance equals the signed sum of entries.
UPDATE cash_accounts SET balance_minor =
    (SELECT COALESCE(SUM(amount_minor), 0) FROM cash_entries e WHERE e.cash_account_id = cash_accounts.id);

-- Per-document number sequences for customer-facing documents.
INSERT OR IGNORE INTO document_sequences (document_type, next_value, prefix, suffix) VALUES
  ('sale', 1, 'INV-', ''),
  ('quotation', 1, 'QTN-', ''),
  ('customer_payment', 1, 'RCP-', '');

-- Phase 6 action-level permissions.
INSERT OR IGNORE INTO permissions (code, description) VALUES
  ('customer.create', 'Create and edit customer profiles'),
  ('customer.view', 'View customers and account statements'),
  ('payment.receive', 'Record and void customer payments'),
  ('sale.create', 'Create, post, and view sales and quotations'),
  ('sale.cancel', 'Cancel confirmed sales'),
  ('sale.discount.override', 'Allow discounts and below-cost prices'),
  ('sale.credit', 'Allow credit sales (leave a due balance)'),
  ('bundle.create', 'Create and edit furniture sets'),
  ('bundle.view', 'View furniture sets and availability'),
  ('invoice.print', 'Generate and reprint invoice PDFs');

-- Role template grants.
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('customer.create', 'customer.view', 'payment.receive', 'sale.create',
                'sale.cancel', 'sale.discount.override', 'sale.credit',
                'bundle.create', 'bundle.view', 'invoice.print')
WHERE r.code IN ('owner', 'manager');

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('customer.create', 'customer.view', 'payment.receive', 'sale.create',
                'sale.credit', 'bundle.view', 'invoice.print')
WHERE r.code = 'salesperson';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('customer.view', 'payment.receive', 'invoice.print')
WHERE r.code = 'accountant';