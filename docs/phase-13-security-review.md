# Phase 13 — Security Review (Step 4 part 1)

Date: 2026-09-11
Scope: command authorization audit, Tauri capability review, dead/unauthorized
IPC removal, path-containment gaps for images/reports. Adversarial test suite =
Step 5 (`tests/security.rs`), dependency audits = Step 6.

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

- `tests/security.rs` adversarial suite — Step 5
- `cargo audit` / `npm audit` dependency findings + fix list — Step 6
- Secrets-redaction verification is largely satisfied by the existing
  `redact()` chain (logging.rs → audit.rs → DTO redacted_json); a targeted test
  lands with Step 5.