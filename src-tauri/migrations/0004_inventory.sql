-- Furniture Shop Management System — Phase 4: inventory engine and opening stock.
-- Stock is an append-only ledger of immutable, signed movements. Current stock is
-- derived (or cached) from those movements; balances are rebuilt from the ledger.
-- Money is stored as signed 64-bit integer minor units (paisa), never floats.

-- Per-location monotonic, never-reused sequence numbers for stock documents.
ALTER TABLE locations ADD COLUMN move_num_seq INTEGER NOT NULL DEFAULT 0;
ALTER TABLE locations ADD COLUMN session_num_seq INTEGER NOT NULL DEFAULT 0;

-- A second default location so stock can be moved between places out of the box.
INSERT OR IGNORE INTO locations (name, type) VALUES ('Store/Stockroom', 'store');

CREATE TABLE IF NOT EXISTS stock_movements (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id       INTEGER NOT NULL REFERENCES products(id),
    location_id      INTEGER NOT NULL REFERENCES locations(id),
    movement_type    TEXT    NOT NULL CHECK (movement_type IN (
                        'opening', 'purchase_receipt', 'sale_issue', 'customer_return',
                        'supplier_return', 'transfer_out', 'transfer_in', 'reservation',
                        'release', 'damage', 'repair_recovery', 'adjustment',
                        'cancellation_reversal', 'stock_count_correction'
                     )),
    quantity_delta   INTEGER NOT NULL CHECK (quantity_delta != 0),
    move_number      TEXT,
    unit_cost_minor  INTEGER,
    reference_type   TEXT,
    reference_id     INTEGER,
    reason           TEXT,
    created_by       INTEGER NOT NULL REFERENCES users(id),
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    reversal_of_id   INTEGER REFERENCES stock_movements(id)
);

CREATE INDEX IF NOT EXISTS idx_stock_movements_product    ON stock_movements(product_id);
CREATE INDEX IF NOT EXISTS idx_stock_movements_location   ON stock_movements(location_id);
CREATE INDEX IF NOT EXISTS idx_stock_movements_created    ON stock_movements(location_id, product_id, created_at);
CREATE INDEX IF NOT EXISTS idx_stock_movements_reference  ON stock_movements(reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_stock_movements_reversal   ON stock_movements(reversal_of_id);

CREATE TABLE IF NOT EXISTS stock_reservations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id  INTEGER NOT NULL REFERENCES products(id),
    location_id INTEGER NOT NULL REFERENCES locations(id),
    quantity    INTEGER NOT NULL CHECK (quantity > 0),
    status      TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'fulfilled', 'released')),
    expires_at  TEXT,
    created_by  INTEGER REFERENCES users(id),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    released_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_stock_reservations_active
    ON stock_reservations(product_id, location_id, status);

-- Transactionally maintained current and available balances per product/location.
-- Every inventory transaction must update these rows in the same write. They are
-- always rebuildable from stock_movements and stock_reservations.
CREATE TABLE IF NOT EXISTS stock_balances (
    product_id     INTEGER NOT NULL REFERENCES products(id),
    location_id    INTEGER NOT NULL REFERENCES locations(id),
    on_hand        INTEGER NOT NULL DEFAULT 0,
    reserved       INTEGER NOT NULL DEFAULT 0,
    damaged        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (product_id, location_id)
);
CREATE INDEX IF NOT EXISTS idx_stock_balances_product ON stock_balances(product_id);

CREATE TABLE IF NOT EXISTS inventory_count_sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    location_id INTEGER NOT NULL REFERENCES locations(id),
    session_number TEXT,
    status      TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'counted', 'posted', 'cancelled')),
    notes       TEXT,
    created_by  INTEGER NOT NULL REFERENCES users(id),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    posted_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_count_sessions_location ON inventory_count_sessions(location_id);

CREATE TABLE IF NOT EXISTS inventory_count_lines (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id    INTEGER NOT NULL REFERENCES inventory_count_sessions(id) ON DELETE CASCADE,
    product_id    INTEGER NOT NULL REFERENCES products(id),
    expected_qty  INTEGER NOT NULL DEFAULT 0,
    counted_qty   INTEGER NOT NULL DEFAULT 0,
    variance_qty  INTEGER NOT NULL DEFAULT 0,
    UNIQUE (session_id, product_id)
);
CREATE INDEX IF NOT EXISTS idx_count_lines_session ON inventory_count_lines(session_id);

-- Cost-layer ledger for FIFO valuation. Only posted stock receipts that create
-- cost ('opening', 'purchase_receipt', 'adjustment_up', 'customer_return') add
-- layers; issues withdraw from the oldest layer first within the same write.
CREATE TABLE IF NOT EXISTS inventory_cost_layers (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id   INTEGER NOT NULL REFERENCES products(id),
    quantity     INTEGER NOT NULL CHECK (quantity >= 0),
    unit_cost_minor INTEGER NOT NULL CHECK (unit_cost_minor >= 0),
    movement_id  INTEGER REFERENCES stock_movements(id),
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_cost_layers_product ON inventory_cost_layers(product_id, created_at, id);

-- Inventory control toggle, stored in settings. When absent, negative on-hand is
-- always blocked (the strict Version 1 default).
INSERT OR IGNORE INTO settings (key, value_json) VALUES ('inventory.allow_negative', 'false');

-- Phase 4 action-level permissions.
INSERT OR IGNORE INTO permissions (code, description) VALUES
  ('inventory.create', 'Post opening stock and adjustments'),
  ('inventory.valuation', 'View inventory cost layers and valuation'),
  ('inventory.count', 'Manage stock counts');

-- Role template grants. Owner joins manager so every new permission stays in
-- the owner role (the owner-wide grant in 0002 applied only to then-existing
-- permissions).
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('inventory.create', 'inventory.valuation', 'inventory.count')
WHERE r.code IN ('owner', 'manager');

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('inventory.create', 'inventory.count')
WHERE r.code = 'storekeeper';

-- Current sellable stock per product/location. Damaged units are reported
-- separately and excluded from `available`.
CREATE VIEW IF NOT EXISTS current_stock AS
SELECT
    p.id               AS product_id,
    p.article_number,
    p.name             AS product_name,
    images.thumbnail_path,
    balances.location_id,
    loc.name           AS location_name,
    loc.type           AS location_type,
    balances.on_hand,
    balances.reserved,
    balances.damaged,
    p.minimum_stock,
    (balances.on_hand - balances.reserved - balances.damaged) AS available
FROM stock_balances balances
JOIN products p    ON p.id = balances.product_id
JOIN locations loc ON loc.id = balances.location_id
LEFT JOIN product_images images
       ON images.product_id = p.id AND images.is_primary = 1
WHERE p.archived_at IS NULL;

-- FIFO valuation per product: the oldest remaining cost-layer rate as the
-- current unit cost, and the sum of all remaining layers as the on-hand value.
CREATE VIEW IF NOT EXISTS current_valuation AS
SELECT
    b.product_id,
    p.article_number,
    p.name                                  AS product_name,
    COALESCE(cheapest.unit_cost_minor, 0)   AS unit_cost_minor,
    (b.on_hand - b.damaged)                 AS sellable_qty,
    COALESCE(SUM(l.quantity * l.unit_cost_minor), 0) AS value_minor
FROM stock_balances b
JOIN products p ON p.id = b.product_id
LEFT JOIN inventory_cost_layers l ON l.product_id = b.product_id
LEFT JOIN (
    SELECT MIN(id) AS first_layer, product_id
      FROM inventory_cost_layers
     WHERE quantity > 0
     GROUP BY product_id
) oldest ON oldest.product_id = b.product_id
LEFT JOIN inventory_cost_layers cheapest ON cheapest.id = oldest.first_layer
WHERE p.archived_at IS NULL AND (b.on_hand - b.damaged) <> 0
GROUP BY b.product_id;