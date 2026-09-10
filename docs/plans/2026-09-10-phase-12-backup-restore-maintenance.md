# Phase 12: Backup, Restore & Maintenance (Core Scope)

## Scope

Authorized manual backups, backup listing/deletion, restart-safe restore, and a maintenance status page with on-demand integrity checks. **Licensing is explicitly deferred** to a later phase. Outcomes:

1. `backup.create` (Manager+, Owner) can create, list, and inspect maintenance.
2. `backup.restore` (Owner only by default seed) can delete backups and restore.
3. Restore always creates a safety snapshot of the current database first and
   performs the swap at next startup (atomic, Windows-safe, WAL-safe), so a
   failed restore never damages the live database.
4. History of every backup (who/when/size/hash/verified) is kept in `backup_history`.
5. Maintenance page shows DB size, image storage, backups size, free disk space,
   app version, schema version, pending migrations, last backup and last
   integrity check.

## Architecture

```
Frontend                            Rust Backend
─────────                           ────────────
maintenance-page.tsx                 commands/maintenance.rs
  ├─ status cards                     ├─ maintenance_status      → application::maintenance::status
  ├─ Backup now button                ├─ backup_create           → create_backup (backup.create)
  ├─ integrity check button           ├─ backup_list             → list_backups
  ├─ backup table                     ├─ backup_delete           → delete_backup (backup.restore)
  └─ restore dialog                   ├─ backup_restore          → restore_backup  (backup.restore)
       └─ "restart required"          └─ maintenance_integrity   → run_integrity_check
                                       application/m
```

## Implementation Steps

### Step 1: Migration `0012_backup_system.sql`

**File:** `src-tauri/migrations/0012_backup_system.sql` — **new**

```sql
-- Phase 12: backup history.
CREATE TABLE IF NOT EXISTS backup_history (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    name      TEXT NOT NULL UNIQUE,
    size_bytes INTEGER NOT NULL,
    sha256    TEXT NOT NULL,
    verified  INTEGER NOT NULL DEFAULT 1,
    kind      TEXT NOT NULL DEFAULT 'manual', -- manual | restore-safety
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_backup_history_name ON backup_history(name);
```

Settings rows used for maintenance state (created idempotently by the app, not
the migration):

- `maintenance.last_integrity_at` → ISO timestamp
- `maintenance.last_integrity_ok` → `"true"` / `"false"`

**Verification:** `cargo check` passes; migration applies during `db::open`.

### Step 2: Infrastructure — delete, restore marker, sizes

**Files:**
- `src-tauri/src/infrastructure/backup.rs` — extend
- `src-tauri/src/infrastructure/mod.rs` — re-export new helpers

**`delete_backup(backups_dir, name)`**
Validate the name is a plain file inside `backups_dir` (reuse `FilePaths::ensure_member`),
then remove the file. Returns `Ok(())`.

**Restore marker (new module `infrastructure/restore.rs`):**

```rust
pub struct RestoreMarker { pub backup_name: String, pub safety_backup: Option<String> }

// marker file lives at data_dir/restore_pending.json
pub fn read_restore_marker(data_dir: &Path) -> Result<Option<RestoreMarker>, AppError>
pub fn write_restore_marker(data_dir: &Path, marker: &RestoreMarker) -> Result<(), AppError>
pub fn clear_restore_marker(data_dir: &Path) -> Result<(), AppError>

/// Called in setup() BEFORE the pool is opened. If a marker exists, verify the
/// source backup file (sha256 + integrity), write a temp copy, atomically
/// rename it over the live DB, drop WAL/SHM sidecars, then clear the marker.
pub fn perform_pending_restore(paths: &FilePaths) -> Result<Option<String>, AppError>
```

`perform_pending_restore` returns the restored backup name (or `None`) for
logging. Because it runs before any connection opens, the `-wal`/`-shm` files
are absent and the rename is safe. The live DB is untouched if validation fails
and the marker is preserved.

**Storage sizes (`infrastructure/disk_size.rs`):**

```rust
pub fn dir_size(path: &Path) -> Result<u64, AppError>       // recursive bytes
pub fn free_disk_bytes(path: &Path) -> Result<Option<u64>, AppError>
```

`free_disk_bytes` uses `GetDiskFreeSpaceExW` via `windows-sys`
(`Win32_Storage_FileSystem`) on Windows; non-Windows returns `None`.

**`Cargo.toml`** — add under `[target.'cfg(windows)'.dependencies]`:

```toml
windows-sys = { version = "0.59", features = ["Win32_Storage_FileSystem"] }
```

**Verification:** `cargo check` passes on Windows.

### Step 3: Authorization + DTOs

**File:** `src-tauri/src/dto/maintenance.rs` — **new**

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceStatusDto {
    pub app_version: String,
    pub db_size_bytes: u64,
    pub images_size_bytes: u64,
    pub backups_size_bytes: u64,
    pub free_disk_bytes: Option<u64>,
    pub schema_version: i64,
    pub pending_migrations: usize,
    pub last_backup_name: Option<String>,
    pub last_backup_at: Option<String>,
    pub last_integrity_at: Option<String>,
    pub last_integrity_ok: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupListItemDto {
    pub name: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub created_at: String,
    pub kind: String,
    pub created_by: Option<String>,
    pub verified: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResultDto {
    pub restart_required: bool,
    pub safety_backup_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityResultDto {
    pub page_integrity_ok: bool,
    pub foreign_key_violations: i64,
    pub checked_at: String,
}
```

Add `pub mod maintenance;` to `src-tauri/src/dto/mod.rs`.
Add `pub mod maintenance;` to `src-tauri/src/application/mod.rs`.

### Step 4: Application layer `application/maintenance.rs`

**File:** `src-tauri/src/application/maintenance.rs` — **new**

All mutation flows run through `state.write_coordinator` and record an audit
event with the supplied `correlation_id`. Permission checks use `Principal::require`.

- `create_backup(state, principal, correlation_id) -> Result<BackupResultDto, AppError>`
  - `require("backup.create")`
  - call `infrastructure::create_backup(&state.paths.db_path, &state.paths.backups_dir)`
  - insert a `backup_history` row (kind `manual`, created_by principal)
  - audit `backup.create` (entity `backup`, name, size, sha256)
- `list_backups(state, principal) -> Result<Vec<BackupListItemDto>, AppError>`
  - `require("backup.create")`
  - filesystem list (via `infrastructure::list_backups`) merged with
    `backup_history` (kind, created_by, verified)
- `delete_backup(state, principal, name, correlation_id)`
  - `require("backup.restore")`
  - `infrastructure::backup::delete_backup(&state.paths.backups_dir, name)`
  - delete matching `backup_history` row; audit `backup.delete`
- `restore_backup(state, principal, name, correlation_id) -> Result<RestoreResultDto, AppError>`
  - `require("backup.restore")`
  - verify the target backup exists and integrity-checks clean
  - **safety snapshot**: create a `restore-safety` backup of the current DB via
    the existing `create_backup` infr., record in history.
  - write the restore marker (`backup_name`, `safety_backup`)
  - audit `backup.restore`
  - return `RestoreResultDto { restart_required: true, safety_backup_name }`
- `status(state, principal) -> Result<MaintenanceStatusDto, AppError>`
  - `require("backup.create")`
  - db size via `fs::metadata(db_path)`, images+backups size via `dir_size`,
    free disk via `free_disk_bytes`, schema version/pending via the same
    computation as `db::open` (`_sqlx_migrations` count vs `MIGRATOR`), last
    backup from `list_backups` max(created_at), integrity state from settings.
- `run_integrity_check(state, principal, correlation_id) -> Result<IntegrityResultDto, AppError>`
  - `require("backup.create")`
  - `infrastructure::db::integrity_check(&state.pool)`
  - persist `maintenance.last_integrity_at` / `_ok` via `SettingsRepository`
  - audit `maintenance.integrity`

### Step 5: Commands `commands/maintenance.rs`

**File:** `src-tauri/src/commands/maintenance.rs` — **new**; register in `mod.rs`.

```rust
#[tauri::command] pub async fn maintenance_status(session_id: String, state: State<'_, AppState>)
  -> Result<MaintenanceStatusDto, AppErrorDto>
#[tauri::command] pub async fn backup_create(session_id: String, correlation_id: String,
  state: State<'_, AppState>) -> Result<BackupResultDto, AppErrorDto>
#[tauri::command] pub async fn backup_list(session_id: String, state: State<'_, AppState>)
  -> Result<Vec<BackupListItemDto>, AppErrorDto>
#[tauri::command] pub async fn backup_delete(session_id: String, name: String,
  correlation_id: String, state: State<'_, AppState>) -> Result<(), AppErrorDto>
#[tauri::command] pub async fn backup_restore(session_id: String, name: String,
  correlation_id: String, state: State<'_, AppState>) -> Result<RestoreResultDto, AppErrorDto>
#[tauri::command] pub async fn maintenance_integrity(session_id: String, correlation_id: String,
  state: State<'_, AppState>) -> Result<IntegrityResultDto, AppErrorDto>
```

Each: `commands::authed(&state, &session_id, "<perm>")` then delegate, wrapped by
`wrapper::run_command`.

**`src-tauri/src/lib.rs`:** register all six in `invoke_handler`; and BEFORE
`infrastructure::db::open(&paths)` in the `.setup()` closure call
`infrastructure::restore::perform_pending_restore(&paths)` and log the result.

### Step 6: Frontend — shell + page

**Files:**
- `src/lib/shell.ts` — add `"maintenance"` to `ShellView`
- `src/components/app-shell.tsx` — add Administration nav item
  `{ id: "maintenance", label: "Backup & maintenance", icon: HardDrive, view: "maintenance", permission: "backup.create" }` + `VIEW_TITLES["maintenance"]` + render `<MaintenancePage />`
- `src/components/maintenance/maintenance-page.tsx` — **new**
- `src/lib/tauri/api.ts` — add wrappers + types

**`maintenance-page.tsx` structure:**

- Status card grid: Database size, Images storage, Backups storage, Free disk,
  App/Schema version, Pending migrations, Last backup, Last integrity check
- Actions: **Back up now** (calls `backupCreate`), **Run integrity check**
  (calls `maintenanceIntegrity`, shows result banner)
- Backups table: name, created_at, size, kind, created_by, verified badge;
  per-row **Delete** (confirm dialog, typed name for destructive ops per §14
  convention) and **Restore** (confirm dialog)
- Restore confirmed → show success banner: *"Restart the app to complete the
  restore. A safety backup was created: <name>."*
- Loading skeleton, empty state, error state (reuse existing table components)

**`api.ts` additions:**

```typescript
export interface MaintenanceStatus { appVersion: string; dbSizeBytes: number; ... }
export interface BackupListItem { name: string; sizeBytes: number; sha256: string; ... }
export interface RestoreResult { restartRequired: boolean; safetyBackupName?: string | null; }
export interface IntegrityResult { pageIntegrityOk: boolean; foreignKeyViolations: number; checkedAt: string; }
export const maintenanceStatus = (session: string) => invoke<MaintenanceStatus>("maintenance_status", { sessionId: session });
export const backupCreate = (session: string) => invoke<BackupResult>("backup_create", { sessionId: session, correlationId: uuid() });
export const backupList = (session: string) => invoke<BackupListItem[]>("backup_list", { sessionId: session });
export const backupDelete = (session: string, name: string) => invoke<void>("backup_delete", { sessionId: session, name, correlationId: uuid() });
export const backupRestore = (session: string, name: string) => invoke<RestoreResult>("backup_restore", { sessionId: session, name, correlationId: uuid() });
export const maintenanceIntegrity = (session: string) => invoke<IntegrityResult>("maintenance_integrity", { sessionId: session, correlationId: uuid() });
```

Align with whatever correlation-id helper the existing pages use (reuse the same
pattern from `reports-page.tsx`).

### Step 7: Tests

**File:** `src-tauri/tests/maintenance.rs` — **new** (model on `reports.rs` helpers)

- Unauthorized: a salesperson principal cannot create/list/delete/restore.
- `backup_create` writes a real `.db` file, a `backup_history` row, and an
  audit event; `backup_list` returns it.
- `backup_delete` removes the file and history row.
- `backup_restore` requires `backup.restore`; on success it writes the marker,
  creates a `restore-safety` snapshot, and `perform_pending_restore` actually
  swaps the file when called on a temp app dir (create data → backup → mutate →
  restore → assert original data is back).
- Corrupt backup: restoring a tampered file fails and leaves DB untouched.
- `status` returns correct sizes, version, pending migrations, integrity state
  after a run.
- `run_integrity_check` returns ok on a healthy DB and persists settings.

**Gates:** `cargo test` (all suites), `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`, `npm run typecheck`, `npm run lint`, `npm run build`.

### Step 8: Commit + push

```bash
git add -A
git -c user.name="Furniture Shop Devs" -c user.email="furnitureshop-dev@localhost" commit -m "Phase 12: backup management, restart-safe restore, maintenance status"
git push
```

## File Summary

| File | Action |
|------|--------|
| `src-tauri/migrations/0012_backup_system.sql` | **New** |
| `src-tauri/src/infrastructure/backup.rs` | Edit: add `delete_backup` |
| `src-tauri/src/infrastructure/restore.rs` | **New** (marker + `perform_pending_restore`) |
| `src-tauri/src/infrastructure/disk_size.rs` | **New** (`dir_size`, `free_disk_bytes`) |
| `src-tauri/src/infrastructure/mod.rs` | Edit: modules + re-exports |
| `src-tauri/Cargo.toml` | Edit: `windows-sys` (win target) |
| `src-tauri/src/dto/maintenance.rs` | **New** |
| `src-tauri/src/dto/mod.rs` | Edit: `pub mod maintenance;` |
| `src-tauri/src/application/mod.rs` | Edit: `pub mod maintenance;` |
| `src-tauri/src/application/maintenance.rs` | **New** |
| `src-tauri/src/commands/maintenance.rs` | **New** |
| `src-tauri/src/commands/mod.rs` | Edit: `pub mod maintenance;` |
| `src-tauri/src/lib.rs` | Edit: register commands + pending restore in setup |
| `src/lib/shell.ts` | Edit: add `"maintenance"` |
| `src/components/app-shell.tsx` | Edit: nav item + view title + render |
| `src/components/maintenance/maintenance-page.tsx` | **New** |
| `src/lib/tauri/api.ts` | Edit: wrappers + types |
| `src-tauri/tests/maintenance.rs` | **New** |

## Permissions

- `backup.create` — already seeded (0002) for **owner + manager**. Gates
  create/list/status/integrity.
- `backup.restore` — already seeded (0001), **owner only** by default. Gates
  delete + restore.
- No new permissions and no migration edits to 0001–0011. The ONLY migration
  change is the new `0012`.

## Out of Scope (Phase 12 deferred)

- Licensing / registration / trial enforcement (§18 licensing block)
- Automatic scheduled backups / backup rotation & retention policy
- Encrypted / compressed backup bundles
- Cloud backup targets
- Backup while app is already closing (window-close hook stays a stub)