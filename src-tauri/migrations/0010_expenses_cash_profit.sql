-- Furniture Shop Management System — Phase 9: expenses, cash management, and
-- profit.
--
-- Expenses are user-managed, categorized documents. Posting one expense records
-- the category, amount, date, payment account, and an optional attachment
-- reference in a single atomic transaction together with the cash outflow and
-- an audit event; posted expenses are reversed rather than edited. Owner
-- capital contributions and withdrawals are recorded as owner transactions that
-- move cash but never participate in operational profit. A statutory
-- profit/cash-flow view computed from posted document dates keeps operating
-- profit separate from cash movement dates.
--
-- Money is stored as signed 64-bit integer minor units (paisa), never floats.
-- Expense categories are seeded system defaults but remain user-managed.

-- ---------------------------------------------------------------------------
-- Expense categories. The seed rows have no creator (they ship with the app);
-- user-added categories carry the acting user id. FK enforcement is active
-- during migrations, so seeded rows cannot reference a user that does not
-- exist yet.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS expense_categories (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    code        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    is_active   INTEGER NOT NULL DEFAULT 1,
    created_by  INTEGER REFERENCES users(id),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- ---------------------------------------------------------------------------
-- Expenses: draft (not yet money moving) → posted (cash outflow) → reversed.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS expenses (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    expense_number  TEXT UNIQUE,
    category_id     INTEGER NOT NULL REFERENCES expense_categories(id),
    amount_minor    INTEGER NOT NULL CHECK (amount_minor > 0),
    expense_date    TEXT NOT NULL,
    cash_account_id INTEGER NOT NULL REFERENCES cash_accounts(id),
    description     TEXT NOT NULL,
    payee           TEXT,
    reference       TEXT,
    attachment_path TEXT,
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft', 'posted', 'reversed')),
    idempotency_key TEXT UNIQUE,
    created_by      INTEGER NOT NULL REFERENCES users(id),
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    posted_by       INTEGER REFERENCES users(id),
    posted_at       TEXT,
    reversed_by     INTEGER REFERENCES users(id),
    reversed_at     TEXT,
    reversal_reason TEXT,
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_expenses_date ON expenses(expense_date);
CREATE INDEX IF NOT EXISTS idx_expenses_status ON expenses(status);
CREATE INDEX IF NOT EXISTS idx_expenses_category ON expenses(category_id);

-- ---------------------------------------------------------------------------
-- Owner capital in/withdrawals. These move cash between the business and the
-- owner and are excluded from operational profit (they are financing flows).
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS owner_transactions (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_number TEXT UNIQUE,
    kind               TEXT NOT NULL CHECK (kind IN ('capital_in', 'withdrawal')),
    amount_minor       INTEGER NOT NULL CHECK (amount_minor > 0),
    transaction_date   TEXT NOT NULL,
    cash_account_id    INTEGER NOT NULL REFERENCES cash_accounts(id),
    notes              TEXT,
    idempotency_key    TEXT UNIQUE,
    created_by         INTEGER NOT NULL REFERENCES users(id),
    created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_owner_transactions_date ON owner_transactions(transaction_date);

-- ---------------------------------------------------------------------------
-- Widen the cash entry types for expenses and owner transfers (rebuild of the
-- Phase 6/8 table; cash_entries is a child table, so the rebuild is safe even
-- with foreign-key enforcement enabled).
-- ---------------------------------------------------------------------------
ALTER TABLE cash_entries RENAME TO cash_entries_0010_old;
CREATE TABLE cash_entries (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    cash_account_id  INTEGER NOT NULL REFERENCES cash_accounts(id),
    entry_type       TEXT    NOT NULL CHECK (entry_type IN (
                        'opening', 'purchase_payment', 'payment_void', 'supplier_refund',
                        'sale_payment', 'sale_refund', 'customer_receipt', 'receipt_void',
                        'return_void', 'expense', 'expense_reversal',
                        'owner_capital', 'owner_withdrawal'
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
  FROM cash_entries_0010_old;
DROP TABLE cash_entries_0010_old;
CREATE INDEX IF NOT EXISTS idx_cash_entries_account ON cash_entries(cash_account_id);
CREATE INDEX IF NOT EXISTS idx_cash_entries_ref ON cash_entries(reference_type, reference_id);
-- Maintain the invariant documented at creation: balance equals the opening
-- balance plus the signed sum of entries (the 0008 rebuild summed entries only,
-- dropping the opening-balance component; this corrected rebalance runs last
-- so it wins for any upgrade path).
UPDATE cash_accounts SET balance_minor =
    COALESCE(opening_balance_minor, 0) +
    (SELECT COALESCE(SUM(amount_minor), 0) FROM cash_entries e WHERE e.cash_account_id = cash_accounts.id);

-- Per-document number sequences for Phase 9 posted documents.
INSERT OR IGNORE INTO document_sequences (document_type, next_value, prefix, suffix) VALUES
  ('expense', 1, 'EXP-', ''),
  ('owner_transaction', 1, 'OWN-', '');

-- Seeded system expense categories (user-managed defaults).
INSERT OR IGNORE INTO expense_categories (code, name, is_active) VALUES
  ('rent', 'Rent', 1),
  ('salaries', 'Salaries', 1),
  ('electricity', 'Electricity', 1),
  ('transport', 'Transport', 1),
  ('loading', 'Loading / unloading', 1),
  ('repairs', 'Repairs', 1),
  ('marketing', 'Marketing', 1),
  ('internet', 'Internet', 1),
  ('office', 'Office supplies', 1),
  ('bank_fees', 'Bank fees', 1),
  ('food', 'Food', 1),
  ('misc', 'Miscellaneous', 1);

-- Phase 9 action-level permissions.
INSERT OR IGNORE INTO permissions (code, description) VALUES
  ('expense.view', 'View expenses and the cash book'),
  ('expense.create', 'Post expense documents'),
  ('expense.reverse', 'Reverse posted expense documents'),
  ('profit.view', 'View profit and cash-flow reports'),
  ('owner.transfer', 'Record owner capital and withdrawals');

-- Role template grants. Owner joins manager so every new permission stays in
-- the owner role.
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('expense.view', 'expense.create', 'expense.reverse', 'profit.view', 'owner.transfer')
WHERE r.code IN ('owner', 'manager');

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('expense.view', 'expense.create', 'expense.reverse', 'profit.view')
WHERE r.code = 'accountant';