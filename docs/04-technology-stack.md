# Technology Stack

## Selected stack

| Area | Technology | Decision |
| --- | --- | --- |
| Desktop shell | Tauri 2 | Small Windows application with controlled native access |
| Frontend | Next.js + TypeScript | Static client application; no Next.js server runtime inside Tauri |
| UI | React, Tailwind CSS, shadcn/ui or Radix primitives | Accessible, consistent interface |
| Forms | React Hook Form + Zod | Typed client validation |
| Server state | TanStack Query | Command caching, invalidation, loading/error states |
| Tables | TanStack Table | Large sortable/filterable management tables |
| Native backend | Rust | Authoritative business logic and secure OS access |
| Database | SQLite | Reliable embedded offline database |
| Database access | SQLx with compile-time checked queries where practical | Transactions, migrations, typed access |
| Serialization | Serde | DTO serialization between TypeScript and Rust |
| IDs | UUID v7 or ULID | Sortable identifiers safe for future sync |
| Passwords | Argon2id | Local credential hashing |
| Logging | `tracing` + rolling file appender | Structured diagnostics |
| Images | Rust `image` crate | Validation, resizing, thumbnails |
| PDF | A dedicated Rust PDF/report adapter with embedded Unicode font | Repeatable offline exports |
| CSV | Rust `csv` crate | Spreadsheet-compatible exports |
| Testing | Rust tests, Vitest, React Testing Library, Playwright/Webdriver integration | Layered verification |

## Frontend build rules

- Configure Next.js for static export compatible with Tauri.
- Do not use server components that require a running Node.js server.
- Do not put database, money, stock, permission, or ledger logic in API routes.
- Generate TypeScript bindings from Rust types when possible, otherwise share a versioned command contract.

## Database rules

- Enable `PRAGMA foreign_keys = ON` on every connection.
- Prefer WAL journal mode for responsiveness; test backup behavior with WAL.
- Set a busy timeout and serialize long writes where appropriate.
- Store timestamps as UTC ISO-8601 text or integer epochs consistently.
- Store money as signed 64-bit integer minor units, such as paisa.

## Version policy

Pin major versions and lock dependencies. Upgrade Tauri, Rust crates, and npm packages in dedicated pull requests with migration and regression testing. Never accept an automatic major upgrade in a release build.

