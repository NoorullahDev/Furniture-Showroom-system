# Development Guide

Version requirements, local checks, and workflow conventions.

## Version requirements

| Tool | Minimum | Notes |
| --- | --- | --- |
| Node.js | 24 | Enforced in `package.json` `engines` |
| npm | 11 | Enforced in `package.json` `engines` |
| Rust (stable) | 1.98 | CI uses `rustup` stable; build requires `rustc >= 1.77` (`Cargo.toml` `rust-version`) |
| Tauri CLI | 2 | Provided as `@tauri-apps/cli` dev dependency (`npm run tauri`) |
| WebView2 | latest | Runtime on Windows; installer should verify presence |

## Local checks

```powershell
# Frontend — type, lint, static export
npm run typecheck
npm run lint
npm run build

# Rust — format, lint, tests
cd src-tauri
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test

# Full desktop app (debug)
cd ..
npm run tauri dev

# Release installer
npm run tauri build
```

## Environment

There are no required secrets for local development. Optional variables:

- `RUST_LOG` / `FURNITURE_SHOP_LOG_VERBOSE` — controls log filtering and
  mirrors logs to stderr in development. Default filtering is defined in
  `src-tauri/src/infrastructure/logging.rs`.
- Log files roll daily under the app-data `logs/` directory
  (`furniture-shop.log`).

## Secrets never touch the repository

- Never commit `.env` files, signing keys, passwords, license private keys, or
  customer data. The license private key is kept out of the repository by
  design (see `docs/30`).
- The top-level `.env.*` is ignored except the `.env.example` template, which
  must contain placeholders only.

## Commit and review workflow

- `main` is protected: no direct pushes. Changes arrive through pull requests
  reviewed by at least one person.
- Every PR must pass the CI pipeline (frontend checks, Rust fmt/clippy/tests,
  Windows release build smoke test, dependency audit) before merge.
- Keep Phase boundaries: migrate schema via new numbered files in
  `src-tauri/migrations/`; never edit an already-released migration.
- Lockfiles (`package-lock.json`, `src-tauri/Cargo.lock`) are committed and
  must be updated together with dependency changes.