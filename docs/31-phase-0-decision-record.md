# Phase 0 Decision Record

Status: **Approved**
Date: 2026-09-08

This record captures the business decisions confirmed in Phase 0 and their
consequences. It is the authoritative reference for the decisions that shape
Phase 1 onward. If a module document contradicts a decision here, the decision
record wins unless a later approved change supersedes it.

## Decisions

| # | Decision | Chosen value | Why / consequence |
| --- | --- | --- | --- |
| D1 | Launch language | English only | Simpler V1. Architecture stays Urdu-ready: all UI strings in translation dictionaries, Unicode fonts embedded in PDF, UTF-8 storage and naming tested from the start. |
| D2 | Sales tax | Configurable via settings, no hardwired engine | Tax profiles and per-product tax fields exist; a settings-driven global tax profile can apply a rate. No tax logic is hard-coded; defaults are zero. |
| D3 | Stock issue timing | Issue at sale confirmation; scheduled/delivery sales reserve first and issue at dispatch/delivery | Immediate counter sales consume stock immediately. Confirmed credit/order sales reserve and release only on delivery. One documented fulfilment policy chosen during setup; historical documents retain their behavior. |
| D4 | Negative stock | Always blocked | No override path. Manually disabling is not offered in V1. Selling or transferring beyond available stock is rejected with `INSUFFICIENT_STOCK`. |
| D5 | Print/paper format | A4 only (portrait invoices, landscape wide reports) | Thermal receipt format is excluded from V1 and deferred to a configurable future template. |
| D6 | Inventory costing | FIFO (layered cost) | Overrides the earlier weighted-average recommendation in `14-inventory-management.md` and Phase 4/5 text. Purchases create cost layers; consumption consumes the oldest layers first. Algorithm and return re-entry rules to be defined and tested in Phase 4. |
| D7 | Target operating system | Windows 11 only | Testing matrix limited to Windows 11. WebView2 runtime required; the installer should verify/drive WebView2 availability. |
| D8 | Licensing | Optional, offline activation | Trial / active / grace / expired / suspended / invalid states are designed. Verifies a signed payload with an embedded public key; request/response activation files for offline shops. Private signing key never ships. On expiry: read-only + backup/export retained, customer data never destroyed. |
| D9 | App identity | Application brand "Furniture Shop", product-byline "Powered by EagleNest Creations" | Kept separate from customer invoice branding (shop logo/contact on commercial documents). |

## Design direction

Light, calm, professional retail UI. Warm off-white background (`#F7F7F5`),
white surfaces, deep forest/teal primary, warm amber accent used sparingly.
One modern sans-serif family with tabular numerals for money. See
`11-ui-ux-design-system.md` and `32-pos-wireframe.md`.

## Status of Phase 0 technical proofs

| Proof | Status |
| --- | --- |
| Tauri 2 + statically exported Next.js | Done, builds and runs |
| Typed Rust command + typed error DTO | Done |
| SQLite in app-data, WAL, embedded migration | Done, migration runner verified |
| Image import/validate/thumbnail/re-encode | Done (tests pass); proof UI image display pending review |
| Unicode PDF, PKR + Urdu sample, embedded font | Done (currently single page; multi-page templating is Phase 6–11 work) |
| WAL-safe backup + SHA-256 + integrity check | Done (tests pass) |
| Windows installer | Built (NSIS, x64). Clean-VM smoke test is pending owner execution |

## Open Phase 0 actions

1. Owner: run `Furniture Shop_0.1.0_x64-setup.exe` on a clean Windows 11 VM and
   confirm first-run shell, login-style flows not yet present, and basic UI
   render with WebView2.
2. Confirm the POS wireframe (`32-pos-wireframe.md`) before Phase 4 UI work.

## Risk register (active)

| Risk | Effect | Mitigation | Owner |
| --- | --- | --- | --- |
| Business logic duplicated in UI | Incorrect totals and bypasses | Rust-authoritative use cases + integration tests | Tech lead |
| Urdu will not shape in printpdf 0.7 | Broken patient documents if Urdu shipped | Urdu deferred to post-V1; pipeline keeps font + i18n; revisit HarfBuzz or printpdf 0.12 later | Tech lead |
| SQLite used over a network share | Corruption/locking | Local-only database guaranteed; future API for multi-device | Tech lead |
| Ambiguous stock issue timing | Incorrect availability/delivery | D3 policy encoded in state transitions | Product owner |
| Posted records edited directly | Broken audit and balances | Immutable documents + reversals | Tech lead |
| Duplicate clicks / retries | Duplicate invoices/payments | Idempotency keys + unique constraints | Tech lead |
| Backup exists but cannot restore | False safety | Checksums + routine clean-machine restore drills | QA owner |
| FIFO cost layers grow unbounded | Slow valuation queries | Per-product layer table with index by product/date; periodic safe compaction analysis in Phase 4 | Tech lead |
| License expiry blocks customer data | Trust and legal risk | Read-only/export/backup access always retained | Product owner |
| Scope expansion delays release | Unfinished core system | Versioned backlog and milestone change control | Product owner |