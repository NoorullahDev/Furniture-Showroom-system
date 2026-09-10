# Phase 13 — Security Review (Step 4 part 1)

Date: 2026-09-11
Scope: command authorization audit, Tauri capability review, dead/unauthorized
IPC removal, path-containment gaps for images/reports. Adversarial test suite =
Step 5 (`tests/security.rs`), dependency audits = Step 6.

> **Step 5 status: complete.** The adversarial suite is live and green
> (18 tests, ~2.3s) and surfaced the `report_export` permission bug closed in
> §7 below.

## 1. Command authorization grid

Every registered Tauri command was checked to confirm it resolves a session and
(the mutation/sensitive reads) enforce a permission. Two-layer pattern:

- `commands/<area>.rs` → `authenticated()` (session + unlocked) or `authed()`
  (session + permission).
- `application/<area>.rs` → `principal.require("<perm>")` as a second, UI-independent
  enforcement on the same principal the command resolved **before** any write
  through `write_coordinator`.

Intentionally unauthenticated (pre-login bootstrap only, low-information):

| Command | Rationale |
|---|---|
| `first_run_status` | Returns only booleans (`required`/`complete`/`has_users`) before any user exists |
| `first_run_complete` | Creates the owner; idempotent — refuses to run once any user exists (`CONFLICT`) |

All other commands are authenticated and permission-enforced. Spot-audited
read/write pairs both demand a permission at the command layer or the
application layer (never neither). 100+ command/HO flows enumerated; no
command was found that branches on data to skip authorization.

## 2. Vuln: removed unauthenticated `proof_*` Phase-0 commands

**Severity: High.** The six `commands::proof::*` IPC commands were registered
with **no session or permission check**:

- `proof_import_image(path)` — read **any** file the caller named via
  `fs::read` and copied it into the images dir (arbitrary file disclosure of a
  path supplied by an unauthenticated webview caller).
- `proof_app_info` — leaked `data_dir` / `db_path`.
- `proof_create_backup` / `proof_list_backups` / `proof_generate_pdf` /
  `proof_open_path` — unauthenticated backup/PDF/file-open side effects.

None were used by any shipping screen (Phase-0 dashboard leftovers only), so
they were **removed**, not patched:

- `src-tauri/src/commands/proof.rs`, `src-tauri/src/application/proof.rs` deleted.
- Six entries removed from `src-tauri/src/lib.rs` `invoke_handler`.
- `pub mod proof;` removed from `commands/mod.rs` and `application/mod.rs`.
- Dead DTOs removed: `AppInfoDto`, `ImageImportResultDto` (dto/mod.rs).
- Frontend dead wrappers/types removed from `src/lib/tauri/api.ts` and the
  two screens that used `proofOpenPath` were migrated to the authenticated
  `open_file` command (see §3).

## 3. Path containment — reports/images

Reviewed every code path that maps a string to a file:

| Path | Guard | Status |
|---|---|---|
| Standalone image read (`read_product_image`) | canonicalize + `starts_with(images_dir)` + size cap (products.rs:1128) | OK |
| Report export target | server-generated name joined under `reports_dir` (never caller text) | OK |
| Restore source | `FilePaths::ensure_member(backups_dir)` (restore.rs) | OK |
| `open_file` (formerly `proof_open_path`) | **was missing `ensure_member`** | **fixed** |
| Image import (`product_image_add`) | source is a real OS file; output name is server-generated UUID; `MaxSourcePixels` + decode-limit guards (image_pipeline.rs) | OK |

`open_file` (src-tauri/src/commands/reports.rs) previously opened any path for
a session holding `reports.view`. Hardened:

- now gated by `authenticated()` (any active session, matching its use as the
  "open a generated document" helper on sales/fulfilment/reports screens) and
- requires `ensure_member(reports_dir, target)` so escapes and symlink hops are
  rejected, closing the contain-check gap that `proof_open_path` had kept.

Frontend call sites migrated from `proofOpenPath(pdf.reportPath)` to
`openFile(session, pdf.reportPath)`: sales-page.tsx (invoice, receipt),
fulfilment-page.tsx (delivery note, credit note), reports-page.tsx (already on
`openFile`).

## 4. Tauri capabilities

`src-tauri/capabilities/default.json` grants only `core:default` +
`dialog:default`, scoped to the single `main` window. No `fs`, `shell`, `http`,
or `process` plugin permissions are granted, so the surface exposed beyond our
own commands is minimal. No change required.

## 5. Verification

- `cargo check --all-targets` — clean
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo test` — 128 tests pass (all existing suites)
- `cargo fmt --check` — clean
- `npm run typecheck` / `npm run build` — clean

## 6. Remaining (later steps)

- ~~`tests/security.rs` adversarial suite~~ — **Step 5 (complete)**, see §8
- `cargo audit` / `npm audit` dependency findings + fix list — Step 6
- Secrets-redaction verification is largely satisfied by the existing
  `redact()` chain (logging.rs → audit.rs → DTO redacted_json); a targeted test
  lands with Step 5.

## 7. Vuln: `report_export` requested a permission that is never granted

**Severity: Medium (functional lockout + latent drift).** `commands/reports.rs`
`report_export` run through `authed(..., "reports.view")`, but no migration
seeds a `reports.view` permission — the registry grants `report.export`
(owner, manager, accountant). Every export therefore returned `UNAUTHORIZED`
for every role; the application layer `export_report` performs no authorization
of its own, so the command layer was the only gate and it was mis-wired.

**Fixed:** the command now requires the seeded `report.export`
(commands/reports.rs). `authed` (vs `authenticated`) import is kept because
`open_file` uses the weaker session-only gate. Guarded by the RBAC matrix test
and by `report_export_permission_is_the_seeded_code`, which asserts
`report.export` is held by owner/manager/accountant and that `reports.view`
exists nowhere in the registry.

## 8. Adversarial suite `tests/security.rs` (Step 5)

18 tests covering the Step-5 attack list. Summary by area:

| Area | Tests |
|---|---|
| RBAC matrix = seed policy | `rbac_matrix_matches_seed_policy` (owner = all permissions; manager/salesperson/accountant/storekeeper exact grant sets) |
| Permission-drift guard | `report_export_permission_is_the_seeded_code` (fixes §7), `sensitive_setting_needs_permission` |
| Path traversal | `path_traversal_read_product_image_rejected` (`..`, absolute, encoded), `path_traversal_delete_backup_rejected`, `path_traversal_restore_marker_rejected`, `ensure_member_rejects_escapes` |
| Invalid files | `corrupt_and_unsupported_image_rejected`, `oversized_image_serve_rejected` (>25MB), `non_sqlite_backup_rejected_and_db_untouched` |
| SQL injection | `sql_injection_global_search_is_safe`, `sql_injection_list_products_and_audit_safe`, `sql_injection_dropped_callback_is_stored_literally` — `DROP TABLE`/`OR 1=1` payloads are bound parameters; schema survives |
| Locked session | `locked_session_blocked_and_unlock_works` (lock → `SESSION_LOCKED`; wrong password → `INVALID_CREDENTIALS`; correct password recovers) |
| Backup tamper | `backup_tamper_rejected_and_db_untouched` (flipped header byte → verify fails → restore rejected, no marker, DB untouched) |
| Audit chain | `audit_hash_chain_detects_tampering` (`verify_chain` pinpoints the row after an edit) |
| CSV injection | `csv_injection_cells_are_neutralized` (`=`/`+`/`-`/tab prefixing via `sanitize_cell`) |
| Oversized inputs | `oversized_inputs_rejected` (category name >120, full name >200, username <3, weak password) |

Notable confirmations while writing the suite:
- The hash chain reports the **next** row after an edit (tail `prev_hash`
  comparison), so a tampered row `id=2` surfaces as `Some(3)`.
- Global-search binding is parameterized end-to-end; the earliest SQLi vectors
  in list/audit filters are also all `?`-bound.

Verification (all clean): `cargo test` (128 previous + 18 new = 146),
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
`npm run typecheck`, `npm run build`.