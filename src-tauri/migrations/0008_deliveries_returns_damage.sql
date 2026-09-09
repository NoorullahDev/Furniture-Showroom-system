-- Furniture Shop Management System — Phase 8: deliveries, returns, exchanges,
-- and damage.
--
-- Deliveries are fulfilment-tracked documents bound to confirmed sales; every
-- delivery keeps its quantities under the net sold quantities of the sale.
-- Returns reverse stock, cost layers, the customer ledger, and cash atomically
-- in one write; the returned value is capped by the net eligible quantity and
-- the customer's paid position, and the original discount is allocated fairly
-- across the returned lines. Damage records track the loss and its disposition
-- (repair recovery, supplier return, damaged sale, write-off) with movements
-- that reconcile back to the stock ledger.
--
-- Money is stored as signed 64-bit integer minor units (paisa), never floats.
-- Stock movements and the customer ledger are append-only ledgers.

-- ---------------------------------------------------------------------------
-- Widen the customer ledger entry types for returns (rebuild of the Phase 6
-- table; customer_ledger_entries is a child table, so the rebuild is safe even
-- with foreign-key enforcement enabled).
-- ---------------------------------------------------------------------------
ALTER TABLE customer_ledger_entries RENAME TO customer_ledger_entries_0008_old;
CREATE TABLE customer_ledger_entries (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id         INTEGER NOT NULL REFERENCES customers(id),
    entry_type          TEXT    NOT NULL CHECK (entry_type IN (
                            'opening_balance', 'sale', 'payment', 'advance_used',
                            'sale_cancellation', 'payment_refund', 'advance_restore',
                            'sales_return', 'return_void'
                         )),
    document_type       TEXT,
    document_id         INTEGER,
    amount_minor        INTEGER NOT NULL CHECK (amount_minor != 0),
    balance_after_minor INTEGER NOT NULL,
    notes               TEXT,
    created_by          INTEGER NOT NULL REFERENCES users(id),
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO customer_ledger_entries (id, customer_id, entry_type, document_type,
                                     document_id, amount_minor, balance_after_minor,
                                     notes, created_by, created_at)
SELECT id, customer_id, entry_type, document_type, document_id, amount_minor,
       balance_after_minor, notes, created_by, created_at
  FROM customer_ledger_entries_0008_old;
DROP TABLE customer_ledger_entries_0008_old;
CREATE INDEX IF NOT EXISTS idx_customer_ledger_customer ON customer_ledger_entries(customer_id);

-- ---------------------------------------------------------------------------
-- Widen the cash entry types for refund reversals (rebuild of the Phase 6
-- table; cash_entries is a child table, so the rebuild is safe).
-- ---------------------------------------------------------------------------
ALTER TABLE cash_entries RENAME TO cash_entries_0008_old;
CREATE TABLE cash_entries (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    cash_account_id  INTEGER NOT NULL REFERENCES cash_accounts(id),
    entry_type       TEXT    NOT NULL CHECK (entry_type IN (
                        'opening', 'purchase_payment', 'payment_void', 'supplier_refund',
                        'sale_payment', 'sale_refund', 'customer_receipt', 'receipt_void',
                        'return_void'
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
  FROM cash_entries_0008_old;
DROP TABLE cash_entries_0008_old;
CREATE INDEX IF NOT EXISTS idx_cash_entries_account ON cash_entries(cash_account_id);
CREATE INDEX IF NOT EXISTS idx_cash_entries_ref ON cash_entries(reference_type, reference_id);
-- Maintain the invariant documented at creation: balance equals the signed sum of entries.
UPDATE cash_accounts SET balance_minor =
    (SELECT COALESCE(SUM(amount_minor), 0) FROM cash_entries e WHERE e.cash_account_id = cash_accounts.id);

-- ---------------------------------------------------------------------------
-- Deliveries. Statuses: pending -> ready -> dispatched -> delivered | failed;
-- cancelled is terminal; failed may be rescheduled back to pending. Partial
-- deliveries are allowed but never exceed the sale's net sold quantity.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS deliveries (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    delivery_number       TEXT,
    sale_id               INTEGER NOT NULL REFERENCES sales(id),
    customer_id           INTEGER REFERENCES customers(id),
    customer_name         TEXT,
    location_id           INTEGER NOT NULL REFERENCES locations(id),
    status                TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
                              'pending', 'ready', 'dispatched', 'delivered',
                              'failed', 'cancelled'
                           )),
    scheduled_at          TEXT,
    address               TEXT,
    contact_name          TEXT,
    contact_phone         TEXT,
    driver_note           TEXT,
    vehicle_note          TEXT,
    receiver_name         TEXT,
    proof_reference       TEXT,
    delivery_charge_minor INTEGER NOT NULL DEFAULT 0,
    notes                 TEXT,
    reschedule_count      INTEGER NOT NULL DEFAULT 0,
    delivered_at          TEXT,
    delivered_by          INTEGER REFERENCES users(id),
    dispatched_at         TEXT,
    dispatched_by         INTEGER REFERENCES users(id),
    failed_reason         TEXT,
    failed_at             TEXT,
    failed_by             INTEGER REFERENCES users(id),
    cancelled_reason      TEXT,
    cancelled_at          TEXT,
    cancelled_by          INTEGER REFERENCES users(id),
    created_by            INTEGER NOT NULL REFERENCES users(id),
    created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_deliveries_number
    ON deliveries(delivery_number) WHERE delivery_number IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_deliveries_sale ON deliveries(sale_id);
CREATE INDEX IF NOT EXISTS idx_deliveries_customer ON deliveries(customer_id);
CREATE INDEX IF NOT EXISTS idx_deliveries_status ON deliveries(status);
CREATE INDEX IF NOT EXISTS idx_deliveries_scheduled ON deliveries(scheduled_at);

CREATE TABLE IF NOT EXISTS delivery_items (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    delivery_id      INTEGER NOT NULL REFERENCES deliveries(id) ON DELETE CASCADE,
    sale_item_id     INTEGER NOT NULL REFERENCES sale_items(id),
    product_id       INTEGER REFERENCES products(id),
    article_number   TEXT NOT NULL,
    product_name     TEXT NOT NULL,
    quantity         INTEGER NOT NULL CHECK (quantity > 0),
    unit_price_minor INTEGER NOT NULL CHECK (unit_price_minor >= 0),
    line_total_minor INTEGER NOT NULL CHECK (line_total_minor >= 0)
);
CREATE INDEX IF NOT EXISTS idx_delivery_items_delivery ON delivery_items(delivery_id);
CREATE INDEX IF NOT EXISTS idx_delivery_items_sale_item ON delivery_items(sale_item_id);

-- ---------------------------------------------------------------------------
-- Sales returns. A return is an immutable posted document; posting reverses
-- stock/finance atomically. Items classify as sellable, damaged, repair, or
-- disposed. Exchanges are recorded as a return whose refund becomes a credit
-- note that is applied to a new sale.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS sales_returns (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    return_number      TEXT,
    sale_id            INTEGER NOT NULL REFERENCES sales(id),
    customer_id        INTEGER REFERENCES customers(id),
    location_id        INTEGER NOT NULL REFERENCES locations(id),
    return_date        TEXT NOT NULL,
    status             TEXT NOT NULL DEFAULT 'posted' CHECK (status IN ('posted', 'voided')),
    refund_type        TEXT NOT NULL CHECK (refund_type IN ('credit', 'cash', 'exchange')),
    cash_account_id    INTEGER REFERENCES cash_accounts(id),
    total_minor        INTEGER NOT NULL DEFAULT 0,
    total_refund_minor INTEGER NOT NULL DEFAULT 0,
    cash_refund_minor  INTEGER NOT NULL DEFAULT 0,
    credit_note_minor  INTEGER NOT NULL DEFAULT 0,
    notes              TEXT,
    idempotency_key    TEXT,
    posted_by          INTEGER REFERENCES users(id),
    posted_at          TEXT,
    voided_by          INTEGER REFERENCES users(id),
    voided_at          TEXT,
    created_by         INTEGER NOT NULL REFERENCES users(id),
    created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_sales_returns_number
    ON sales_returns(return_number) WHERE return_number IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_sales_returns_idempotency
    ON sales_returns(idempotency_key) WHERE idempotency_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_sales_returns_sale ON sales_returns(sale_id);
CREATE INDEX IF NOT EXISTS idx_sales_returns_customer ON sales_returns(customer_id);

CREATE TABLE IF NOT EXISTS sales_return_items (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    return_id         INTEGER NOT NULL REFERENCES sales_returns(id) ON DELETE CASCADE,
    sale_item_id      INTEGER NOT NULL REFERENCES sale_items(id),
    product_id        INTEGER REFERENCES products(id),
    bundle_id         INTEGER REFERENCES bundles(id),
    article_number    TEXT NOT NULL,
    product_name      TEXT NOT NULL,
    quantity          INTEGER NOT NULL CHECK (quantity > 0),
    unit_price_minor  INTEGER NOT NULL CHECK (unit_price_minor >= 0),
    unit_refund_minor INTEGER NOT NULL CHECK (unit_refund_minor >= 0),
    line_refund_minor INTEGER NOT NULL CHECK (line_refund_minor >= 0),
    classification    TEXT NOT NULL CHECK (classification IN (
                          'sellable', 'damaged', 'repair', 'disposed'
                       ))
);
CREATE INDEX IF NOT EXISTS idx_sales_return_items_return ON sales_return_items(return_id);
CREATE INDEX IF NOT EXISTS idx_sales_return_items_sale_item ON sales_return_items(sale_item_id);

-- Credit notes balance the customer's account and may be applied (for example
-- as the allocated return credit in an exchange) to a later sale.
CREATE TABLE IF NOT EXISTS credit_notes (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    credit_number  TEXT,
    customer_id    INTEGER NOT NULL REFERENCES customers(id),
    return_id      INTEGER REFERENCES sales_returns(id),
    sale_id        INTEGER REFERENCES sales(id),
    amount_minor   INTEGER NOT NULL CHECK (amount_minor > 0),
    status         TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'applied', 'voided')),
    notes          TEXT,
    applied_at     TEXT,
    created_by     INTEGER NOT NULL REFERENCES users(id),
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_credit_notes_number
    ON credit_notes(credit_number) WHERE credit_number IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_credit_notes_customer ON credit_notes(customer_id);
CREATE INDEX IF NOT EXISTS idx_credit_notes_status ON credit_notes(status);

-- ---------------------------------------------------------------------------
-- Damage records. Recording damage posts a 'damage' movement (on-hand to
-- damaged). The disposition resolves the damaged bucket: 'repair' restores it
-- to on-hand, while 'write_off', 'supplier_return', and 'damaged_sale' remove
-- the damaged units from stock (each posts a resolution movement that keeps
-- the damaged-stock report reconcilable with the movement ledger).
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS damage_records (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    damage_number        TEXT,
    product_id           INTEGER NOT NULL REFERENCES products(id),
    location_id          INTEGER NOT NULL REFERENCES locations(id),
    quantity             INTEGER NOT NULL CHECK (quantity > 0),
    damage_date          TEXT NOT NULL,
    source               TEXT NOT NULL CHECK (source IN (
                             'in_hand', 'customer_return', 'count', 'other'
                          )),
    reason               TEXT,
    estimated_loss_minor INTEGER NOT NULL DEFAULT 0 CHECK (estimated_loss_minor >= 0),
    photo_path           TEXT,
    status               TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
    decision             TEXT CHECK (decision IN (
                              'repair', 'supplier_return', 'damaged_sale', 'write_off'
                           )),
    decision_note        TEXT,
    linked_sale_id       INTEGER REFERENCES sales(id),
    resolved_by          INTEGER REFERENCES users(id),
    resolved_at          TEXT,
    created_by           INTEGER NOT NULL REFERENCES users(id),
    created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_damage_records_number
    ON damage_records(damage_number) WHERE damage_number IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_damage_records_product ON damage_records(product_id);
CREATE INDEX IF NOT EXISTS idx_damage_records_status ON damage_records(status);

-- Per-document number sequences for Phase 8 posted documents.
INSERT OR IGNORE INTO document_sequences (document_type, next_value, prefix, suffix) VALUES
  ('delivery', 1, 'DLV-', ''),
  ('sales_return', 1, 'RET-', ''),
  ('credit_note', 1, 'CRN-', ''),
  ('damage_record', 1, 'DMG-', '');

-- Stock issue/reservation behaviour for deliveries. Version 1 issues stock at
-- sale confirmation; deliveries are fulfilment tracking only. The key is used
-- as a hook for future issue-at-dispatch modes and always stays in the allowed
-- set so reports and transitions behave the same regardless of the value.
INSERT OR IGNORE INTO settings (key, value_json) VALUES ('delivery.stock_behavior', '"none"');

-- Phase 8 action-level permissions.
INSERT OR IGNORE INTO permissions (code, description) VALUES
  ('delivery.create', 'Create delivery documents against confirmed sales'),
  ('delivery.view', 'View deliveries and print delivery notes'),
  ('delivery.update', 'Advance, cancel, or reschedule deliveries'),
  ('sale.return', 'Post and void sales returns, refunds, and credit notes'),
  ('credit.note.use', 'Apply credit notes to new sales (exchanges)'),
  ('damage.record', 'Record damage and resolve its disposition');

-- Role template grants. Owner joins manager so every new permission stays in
-- the owner role (the owner-wide grant in 0002 applied only to then-existing
-- permissions).
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('delivery.create', 'delivery.view', 'delivery.update',
                'sale.return', 'credit.note.use', 'damage.record')
WHERE r.code IN ('owner', 'manager');

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('delivery.view', 'delivery.update', 'sale.return', 'credit.note.use')
WHERE r.code = 'salesperson';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('delivery.view', 'sale.return', 'credit.note.use')
WHERE r.code = 'accountant';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('delivery.create', 'delivery.view', 'delivery.update', 'damage.record')
WHERE r.code = 'storekeeper';