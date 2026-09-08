-- Furniture Shop Management System — foundation schema (Phase 1 baseline).

-- Every business table carries id, created_at, updated_at, and active/archive status.
-- Money is stored as signed 64-bit integer minor units (paisa), never floats.

CREATE TABLE IF NOT EXISTS settings (
    key          TEXT PRIMARY KEY,
    value_json   TEXT NOT NULL,
    updated_by   INTEGER,
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS roles (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    code         TEXT NOT NULL UNIQUE,
    name         TEXT NOT NULL,
    description  TEXT,
    is_system    INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS permissions (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    code         TEXT NOT NULL UNIQUE,
    description  TEXT,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    full_name     TEXT NOT NULL,
    is_active     INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS user_roles (
    user_id  INTEGER NOT NULL REFERENCES users(id),
    role_id  INTEGER NOT NULL REFERENCES roles(id),
    PRIMARY KEY (user_id, role_id)
);

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id       INTEGER NOT NULL REFERENCES roles(id),
    permission_id INTEGER NOT NULL REFERENCES permissions(id),
    PRIMARY KEY (role_id, permission_id)
);

CREATE TABLE IF NOT EXISTS sessions (
    id           TEXT PRIMARY KEY,
    user_id      INTEGER NOT NULL REFERENCES users(id),
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_used_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    locked_at    TEXT,
    active       INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);

CREATE TABLE IF NOT EXISTS audit_logs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id       INTEGER,
    action        TEXT NOT NULL,
    entity_type   TEXT,
    entity_id     TEXT,
    reason        TEXT,
    before_json   TEXT,
    after_json    TEXT,
    correlation_id TEXT,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_audit_entity ON audit_logs(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_logs(created_at);

CREATE TABLE IF NOT EXISTS document_sequences (
    document_type TEXT PRIMARY KEY,
    next_value    INTEGER NOT NULL DEFAULT 1,
    prefix        TEXT NOT NULL DEFAULT '',
    suffix        TEXT NOT NULL DEFAULT '',
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS locations (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL UNIQUE,
    type       TEXT NOT NULL DEFAULT 'showroom',
    is_active  INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Idempotent seeds. Products, stock, customers, and suppliers are import data,
-- never hard-coded seeds.

INSERT OR IGNORE INTO roles (code, name, description, is_system) VALUES
  ('owner',       'Owner',       'Full access incl. settings, backup, licensing, audit', 1),
  ('manager',     'Manager',     'Operations, approvals, reports, limited security settings', 1),
  ('salesperson', 'Salesperson', 'Catalogue, quotations, sales, receipts within limits', 1),
  ('accountant',  'Accountant',  'Payments, expenses, statements, financial reports', 1),
  ('storekeeper', 'Storekeeper', 'Products, receipts, stock, transfers, damage, delivery prep', 1);

INSERT OR IGNORE INTO permissions (code, description) VALUES
  ('product.create', 'Create products'),
  ('product.cost.view', 'View product cost'),
  ('sale.create', 'Create sales'),
  ('sale.discount.override', 'Override sale discount'),
  ('sale.cancel', 'Cancel sales'),
  ('payment.void', 'Void payments'),
  ('inventory.adjust', 'Adjust inventory'),
  ('supplier.pay', 'Pay suppliers'),
  ('profit.view', 'View profit figures'),
  ('report.export', 'Export reports'),
  ('user.manage', 'Manage users'),
  ('backup.restore', 'Restore backups'),
  ('audit.view', 'View audit log');

INSERT OR IGNORE INTO locations (name, type) VALUES
  ('Main Showroom', 'showroom');