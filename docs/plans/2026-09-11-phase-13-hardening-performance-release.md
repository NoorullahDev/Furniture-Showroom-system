# Phase 13: Hardening, Performance & Release Candidate

## Scope

Turn the shipped Phase 0–12 system into a **release candidate** before pilot use
(Phase 14). No new user-facing features. Workstreams from master-plan §19:

1. **Performance** — a large real-data seed, measured budgets for hot paths
   (exact search < 150 ms, broad catalogue search < 300 ms on a reference
   machine), query-plan review with only *justified* new indexes, N+1 removal,
   and bounded image memory.
2. **Security review** — verify every command is authorized, extend path
   traversal / invalid-file defenses, run an adversarial test suite (SQL
   injection, permission bypass, locked session, backup tampering, CSV
   injection), dependency audits, and secrets-redaction verification.
3. **Reliability** — kill/idempotency tests for sale/import/backup/export/
   migration, SQLite integrity + transaction-invariant tests, low-disk and
   database-busy recovery.
4. **UX & accessibility** — resolution/scaling matrix, keyboard-only
   workflows, contrast/focus/reduced-motion review, and role usability checks.
5. **Release-candidate exit** — all gates green, performance report,
   security/kill-test/reliability findings documented, and the Phase 13 report
   committed.

Licensing is **still out of scope** (deferred from Phase 12); Phase 14's
production rollout checklist must not assume a license is enforced.

## Architecture

```
Perf harness (dev-only, #[ignore] tests)   Security/reliability harness
───────────────────────────────────────   ───────────────────────────────
tests/perf_support/mod.rs                 tests/security.rs
  ├─ seed_big_state() → (paths, pool)      ├─ RBAC matrix: role × command
  │    category/units/types/suppliers      ├─ path traversal / invalid files
  │    └─ products+images / purchases      ├─ SQL injection probes
  │       └─ invoices+sales / movements    ├─ locked-session disable
  │          └─ customers / ledger         ├─ backup tamper → restore reject
  └─ timing helper (std::time::Instant)    ├─ CSV injection on export cells
tests/perf_measure.rs                     ├─ redaction of audit/log secrets
  └─ startup/search/list/post/dash/        └─ dependency-audit notes
     report/backup/migrate + budgets       tests/reliability.rs
                                           ├─ kill: mid-commit drop + retry
                                           ├─ invariants: stock/ledger/cash
                                           └─ low-disk + SQLITE_BUSY recovery
```

Production code touched only where a defect/precedent demands it (paths.rs
`ensure_member` on image/export paths, image decode limits, an `SQLITE_BUSY`
retry path, a justified index migration `0013_*`). Benchmarks and seeds are
`#[ignore]` integration tests so the shipped binary is unaffected.

Constraints honored: **never edit released migrations 0001–0012** (a new `0013`
is allowed only if an index is justified by EXPLAIN QUERY PLAN), money stays
i64 minor units, commands stay wrapped in `wrapper::run_command`, writes stay
inside `write_coordinator`, DTOs stay camelCase.

## Implementation Steps

### Step 1: Performance — large-data seed

**Files:**
- `src-tauri/Cargo.toml` — add `[dev-dependencies]` `rand = "0.8"` (feature
  optional) for the seed harness
- `src-tauri/tests/perf_support/mod.rs` — **new** shared helper (model on the
  `temp_dir` + `open_state` helpers already used by `tests/maintenance.rs`)
- `src-tauri/tests/performance.rs` — **new** registered test that calls the seed

`perf_support::seed_big_state()` opens a temp app dir, runs real migrations
(the same 0001–0012 the app runs), creates an owner user, then inserts in one
big transaction (prepared statements via `sqlx::Query`/`execute_many` loops):

| Aggregate | Target | Notes |
|---|---|---|
| categories | 500 | nested at depth ≤ 3 |
| product_types / units | seeded by migration | reuse existing rows |
| products | ≥ 50,000 | names generated per search spec (see Step 2) so exact/broad matches exist |
| product_attributes | ~2 per product | bounded |
| product_images | ≥ 50,000 rows | reference ~200 small JPEG files physically written to `paths.images_dir` |
| suppliers + purchases + purchase_items | ≥ 4,000 / 60,000 / 200,000 | populated ledger entries |
| stock_movements | ≥ 500,000 | sale/purchase/open/reserve kinds with consistent stock_balances |
| customers + sales + sale_items | ≥ 20,000 / 100,000 / 300,000 | with sale_item_components for sets |
| customer_ledger_entries / customer_payments | ≥ 400,000 | debt/receipt pairs |
| expenses + expense_categories | ≥ 20,000 | payments/cash_entries entries too |
| audit_logs | ≥ 5,000 | only via the real `AuditService` path for a sample, rest bulk |

The seed asserts post-insert counts so a broken generator cannot silently pass.
The test is `#[ignore]`ed and run explicitly with `cargo test --test
performance -- --ignored --nocapture`.

**Verification:** `cargo test --test performance -- --ignored` finishes and the
count assertions hold. Bulking a 500k-row insert may take a few minutes in
debug; if so the plan allows `#[cfg(debug_assertions)]` skip or this step to be
run as `cargo test --release`.

### Step 2: Performance — measurements + budgets

**File:** `src-tauri/tests/perf_measure.rs` — **new** (`#[ignore]`, reuses
`perf_support::seed_big_state`). Times each hot path with `std::time::Instant`,
prints a table, and asserts budgets:

| Hot path | Command / call | Budget |
|---|---|---|
| Cold startup | `perf_support` → `infrastructure::db::open` + migrations | ≤ 2 s (seeded) |
| Article search (exact) | `search::global_search` with an exact product name/ref lookup | ≤ 150 ms |
| Broad catalogue search | `search::global_search` substring / partial | ≤ 300 ms |
| List loading | `product_list` / `sale_list` first page | ≤ 300 ms |
| Product get | `product_get` with image row resolution | ≤ 200 ms |
| Sale posting | `sale_create` + `sale_confirm` (single small sale) | ≤ 500 ms |
| Dashboard | `dashboard_summary` | ≤ 400 ms |
| Reports | `report_export` top-3 heavy reports to PDF | ≤ 1.5 s each |
| Backup | `maintenance_backup_create` | ≤ 3 s |
| Integrity | `maintenance_integrity` | ≤ 2 s |

Budgets are measured **as an average of 3 runs** after one warm-up run. The
reference machine is "the developer's local box" and results are recorded to
`docs/phase-13-report.md` (Step 11) rather than CI-failing flakily. A
`#[ignore]` "budget asserts" test runs the suite and fails if any budget is
exceeded — run on demand, not in CI.

**Verification:** `cargo test --test perf_measure -- --ignored --nocapture`
prints the table; the report records real numbers.

### Step 3: Performance — query-plan review, justified indexes, N+1

**Files:**
- `docs/phase-13-performance-review.md` — **new** findings doc
- `src-tauri/migrations/0013_perf_indexes.sql` — **new**, only if warranted

Manual review pass over EXPLAIN QUERY PLAN for the queries behind the hot paths
above (search, product list, dashboard aggregates, sale post reservation,
stock valuation, P&L report, backup history list). For each: record the plan,
add an index only when the plan's scan improves materially, and add it as a
single new migration **0013** (never edit 0001–0012). Candidate suspects the
seed will reveal: `stock_balances(product_id, location_id)`,
`stock_movements(product_id, type, created_at)`, `sale_items(sale_id)`,
`audit_logs(user_id, created_at)`. N+1 check: product/sale list endpoints must
resolve images/child rows with `IN (...)` or COUNT/SUM aggregation, not a
per-row query.

Create the migration statement inline in this plan only after EXPLAIN evidence
— the default is **no new index**.

**Verification:** `cargo test` still green with the new migration applied to a
fresh DB; review doc records plan before/after for each hot query.

### Step 4: Security — command authorization + path defense gaps

**Files:**
- `src-tauri/src/infrastructure/paths.rs` — extend `ensure_member` usage
- `src-tauri/src/infrastructure/images.rs` (or wherever image paths resolve) —
  enforce `ensure_member` on image file reads/imports
- `src-tauri/src/infrastructure/csv_export.rs` — keep `sanitize_cell`, add
  `ensure_member` on the export target path

**Actions:**
- Audit every command (list above, 100+) confirms each calls
  `authed(...)` + `principal.require(...)` or is a non-sensitive probe
  (`proof_app_info`, `first_run_status`). Record the audit grid in
  `docs/phase-13-security-review.md`.
- Close the `ensure_member` gap: image import/read (`product_image_add`,
  `proof_import_image`), report/CSV export targets (`report_export`,
  `open_file`), and restore already covered (restore.rs) get path membership
  checks so `../` escapes the approved directory.
- Confirm capabilities stay minimal: `capabilities/default.json` is still only
  `core:default` + `dialog:default` on window `main`; blocked `fs`/`shell`/
  `http` plugin perms remain absent.

**Verification:** grep confirms every command is behind `authed`; new path
tests in Step 6.

### Step 5: Security — adversarial test suite

**File:** `src-tauri/tests/security.rs` — **new**

| Test | What it proves |
|---|---|
| RBAC matrix | each of owner / manager / accountant / storekeeper / salesperson × every command returns `UNAUTHORIZED` where its permission is absent |
| Path traversal | product image add/remove, report export, open_file, restore reject `..\..\`, absolute escapes, and `%2e%2e`/unicode lookalikes |
| Invalid files | corrupt JPEG, oversized image, non-SQLite "backup", truncated audit file → typed error, no partial DB/image state |
| SQL injection | search/sort/name payloads `'; DROP TABLE ...` and `" OR 1=1 --` never alter schema or widen results (asserts exact-match only) |
| Locked session | after `auth_lock`, every command in a sample of 10 returns `SESSION_LOCKED` until `auth_unlock` |
| Backup tampering | modify one byte of a backup → `perform_pending_restore`/`restore_backup` rejects and live DB untouched |
| CSV injection | cell values `=cmd`, `+123`, `-1;drop`, `@import` export as `'=cmd` … (existing `sanitize_cell`) |
| Oversized inputs | name/length limits respected (max lengths across entity DTOs return `VALIDATION`) |

**Verification:** `cargo test --test security` all green.

### Step 6: Security — dependency audits + secrets redaction

**Files:**
- `docs/phase-13-security-review.md` — **new** (also records Step 4 grid)
- Possible `Cargo.toml`/`package.json` changes only to fix found issues

**Actions:**
- `cargo install cargo-audit` if absent; run `cargo audit` and capture output.
- `npm audit` (and `npm audit --omit=prod` for dev deps); capture output.
- Every finding: severity, affected crate/package, decision (upgrade / pin /
  accepted with justification), recorded in the review doc. Must-have gate:
  no vulnerable item in the shipped dependency tree without an evidence-based
  exception.
- Secrets redaction: verify the existing `redact()` chain
  (`logging.rs redact` → `audit.rs` before/after JSON → DTO
  `redacted_json`) covers `password`, `password_hash`, `token`, `secret`,
  `salt`, `key`; add a test proving an audit row written with a
  `{ "password_hash": "..." }` payload contains `[REDACTED]`, and that
  `UserDto` (already hash-free) stays hash-free. Confirm TLS/log settings never
  log a password by inspection.

**Verification:** audit logs appended to review doc; redaction test passes;
no high-priority open finding.

### Step 7: Reliability — kill/idempotency & transaction invariants

**Files:**
- `src-tauri/tests/reliability.rs` — **new**

**Tests:**
- **Kill/mid-commit recovery**: for `sale_confirm`, `purchase_post`,
  `stock_adjust`, image import, `backup_create`, and migration — execute in a
  separate task, abort it mid-flight (`tokio::select!` timeout / future drop),
  then re-run the flow and assert no duplicate document_number, no double stock
  movement, and balances reconcile (`receivables`, `stock_balances`, cash
  accounts all tie out).
- **Backend-committed/frontend-failed idempotency**: simulate a committed
  `sale_confirm` whose response the UI "lost" by re-invoking the same
  correlation_id; assert the second call is harmless (idempotent via
  document_sequences / unique keys).
- **Transaction invariants on seeded data**: stock never < 0 at any point,
  Σ invoice items ± tax/sets = invoice total, Σ movements per product =
  stock_balance, Σ customer ledger = receivables, gross profit report
  reconciles against cash + dues + debtors.
- **SQLite integrity**: `PRAGMA integrity_check` on the seeded DB returns
  `ok`; `integrity_check(pool)` reports 0 FK violations; a deliberate FK break
  is caught.

**Verification:** `cargo test --test reliability` all green.

### Step 8: Reliability — low-disk and database-busy recovery

**Files:**
- `src-tauri/src/infrastructure/db.rs` — ensure `busy_timeout` is applied on
  pool connect (check + set `PRAGMA busy_timeout = 3000`), and map
  `SQLITE_BUSY`/`SQLITE_LOCKED` to a `DATABASE`/`CONFLICT` AppError with a
  clear message surfaced by the frontend command error path
- `src-tauri/tests/reliability.rs` — extend

**Tests:**
- **Busy**: two concurrent `sale_confirm` writers against the same pool both
  complete or one gets a clean retryable error (no deadlock, no suppressed
  data); concurrent backup while writing does not corrupt the backup.
- **Low-disk**: seed a `tmpfs`/small-claim scenario using `fs2`/`set_file_size`
  trick (or document manual run) ensuring backup create/restore and image
  import return a typed `IO_ERROR` instead of panicking, and the frontend shows
  the error (unit-level: AppError→Dto mapping).

Manual protocol notes captured in `docs/phase-13-kill-testing.md` for the
GUI-level checks that can't be automated fully (Step 9).

**Verification:** reliability tests green; busy_timeout verified via a probe.

### Step 9: UX & accessibility — manual test matrix

**Files:**
- `docs/phase-13-ux-a11y.md` — **new** checklist + results (screen-by-screen)

**Actions:**
- Screen matrix (login, dashboard, products, sale, invoice, purchase,
  inventory, fulfilment, customers, reports, settings, maintenance) at
  1366×768 and 1920×1080 and at 125% / 150% Windows scaling; record overflow,
  truncation, unclickable elements, contrast < 4.5:1.
- Keyboard-only walkthrough of every major workflow (tab order, focus
  rings/`focus-visible`, Escape closes dialogs, Enter submits, no mouse-only
  traps).
- Reduced-motion / labels / error-state review against existing dialog, toast,
  table, and form components.
- Role usability sessions: owner, salesperson, accountant, storekeeper each
  complete their daily workflow; findings logged with priority.
- `npm run lint` (eslint-config-next includes jsx-a11y) runs clean; fix any
  flagged a11y issues.

This step is largely manual and evidence lives in the doc; automatable fixes
(e.g., a missing `aria-label`, focus ring) are applied as small PRs within
Phase 13.

**Verification:** matrix filled out and signed in `docs/phase-13-ux-a11y.md`;
lint clean.

### Step 10: Release-candidate exit — report + gates

**File:** `docs/phase-13-report.md` — **new** (dates, hardware, measured table,
findings, resolutions, exceptions)

**Gates to run (all must pass):**
- `cargo test` (all suites incl. `tests/performance.rs`, `tests/security.rs`,
  `tests/reliability.rs`, existing 12 suites) 
- `cargo clippy --all-targets -- -D warnings`
- `cargo fmt --check`
- `npm run typecheck`
- `npm run lint`
- `npm run build`
- Perf budgets from `perf_measure` recorded (pass or documented exception)
- Security review / kill-testing / UX matrix signed off
- No open Critical or High defect without an approved exception

### Step 11: Commit + push

```bash
git add -A
git -c user.name="Furniture Shop Devs" -c user.email="furnitureshop-dev@localhost" commit -m "Phase 13: hardening, performance, release candidate"
git push
```

Larger review docs (report/security/UX/kill-testing) may arrive as separate
commits before this final one if they get large; the final commit message above
is the release-candidate marker.

## File Summary

| File | Action |
|------|--------|
| `src-tauri/Cargo.toml` | Edit: `[dev-dependencies]` `rand` (+ `fs2` if used for low-disk) |
| `src-tauri/tests/perf_support/mod.rs` | **New** (shared seed + timing helpers) |
| `src-tauri/tests/performance.rs` | **New** (`#[ignore]` seed) |
| `src-tauri/tests/perf_measure.rs` | **New** (`#[ignore]` budgets + report) |
| `src-tauri/tests/security.rs` | **New** (RBAC/traversal/injection/tamper/CSV) |
| `src-tauri/tests/reliability.rs` | **New** (kill/idempotency/invariants/busy/low-disk) |
| `src-tauri/migrations/0013_perf_indexes.sql` | **New only if EXPLAIN justifies an index** |
| `src-tauri/src/infrastructure/paths.rs` | Edit: apply `ensure_member` to image/export paths |
| `src-tauri/src/infrastructure/db.rs` | Edit: `busy_timeout` + busy/locked error mapping |
| `src-tauri/src/infrastructure/images.rs` (path) | Edit: path membership + decode `Limits` |
| `src-tauri/src/infrastructure/csv_export.rs` | Edit: `ensure_member` on export target |
| `docs/phase-13-performance-review.md` | **New** |
| `docs/phase-13-security-review.md` | **New** |
| `docs/phase-13-kill-testing.md` | **New** |
| `docs/phase-13-ux-a11y.md` | **New** |
| `docs/phase-13-report.md` | **New** |

## Permissions

No new permissions and no changes to role or permission seeds. `tests/security.rs`
asserts the existing seeds exactly.

## Out of Scope (Phase 13 deferred)

- Licensing / registration / trial states (deferred again; still required
  before Phase 14 rollout per master plan §18 licensing block)
- Scheduled/automatic backups, rotation & retention, encrypted bundles, cloud
  targets (Phase 12 deferred list)
- Real pilot data import, user training, printer configuration — Phase 14
- Non-essential feature work; freeze until financial/inventory accuracy proven