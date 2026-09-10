-- Furniture Shop Management System — Phase 12: backup history.

CREATE TABLE IF NOT EXISTS backup_history (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL UNIQUE,
    size_bytes INTEGER NOT NULL,
    sha256     TEXT NOT NULL,
    verified   INTEGER NOT NULL DEFAULT 1,
    kind       TEXT NOT NULL DEFAULT 'manual', -- manual | restore-safety
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_backup_history_name ON backup_history(name);
CREATE INDEX IF NOT EXISTS idx_backup_history_created ON backup_history(created_at);