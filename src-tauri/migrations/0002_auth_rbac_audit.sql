-- Furniture Shop Management System — Phase 2: authentication, RBAC, and audit.

-- Audit integrity: add a hash chain between audit events so any tampering is
-- detectable. All new columns are nullable for ALTER ADD COLUMN compatibility.

ALTER TABLE audit_logs ADD COLUMN prev_hash TEXT;
ALTER TABLE audit_logs ADD COLUMN session_id TEXT;
ALTER TABLE audit_logs ADD COLUMN app_version TEXT;
ALTER TABLE audit_logs ADD COLUMN approval_user_id INTEGER;

CREATE INDEX IF NOT EXISTS idx_audit_actor ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_logs(action);

-- Action-level permissions for Phase 2 and beyond.
INSERT OR IGNORE INTO permissions (code, description) VALUES
  ('user.create', 'Create users'),
  ('user.edit', 'Edit users'),
  ('user.deactivate', 'Deactivate users'),
  ('user.reset_password', 'Reset user passwords'),
  ('role.manage', 'Manage roles and permissions'),
  ('settings.manage', 'Change application and shop settings'),
  ('settings.danger', 'Change inventory/accounting-affecting settings'),
  ('backup.create', 'Create backups'),
  ('audit.export', 'Export the audit log');

-- Role template mappings. Owner receives every permission; the other templates
-- get focused subsets as a starting point that remains editable.
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p ON 1 = 1
WHERE r.code = 'owner';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN (
    'settings.manage',
    'product.cost.view',
    'sale.discount.override',
    'sale.cancel',
    'payment.void',
    'inventory.adjust',
    'profit.view',
    'report.export',
    'audit.view',
    'backup.create'
  )
WHERE r.code = 'manager';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('product.create', 'sale.create')
WHERE r.code = 'salesperson';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('payment.void', 'profit.view', 'report.export')
WHERE r.code = 'accountant';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p
  ON p.code IN ('product.create', 'inventory.adjust')
WHERE r.code = 'storekeeper';