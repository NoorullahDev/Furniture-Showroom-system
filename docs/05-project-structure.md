# Project Structure

```text
furniture-shop/
├── src/                         # Next.js UI
│   ├── app/
│   │   ├── dashboard/
│   │   ├── sales/
│   │   ├── products/
│   │   ├── inventory/
│   │   ├── purchases/
│   │   ├── customers/
│   │   ├── suppliers/
│   │   ├── deliveries/
│   │   ├── expenses/
│   │   ├── reports/
│   │   └── settings/
│   ├── components/
│   ├── features/
│   ├── lib/tauri/
│   ├── schemas/
│   ├── styles/
│   └── types/
├── src-tauri/
│   ├── migrations/
│   ├── capabilities/
│   ├── src/
│   │   ├── commands/
│   │   ├── application/
│   │   ├── domain/
│   │   ├── repositories/
│   │   ├── infrastructure/
│   │   ├── dto/
│   │   ├── error.rs
│   │   ├── state.rs
│   │   └── lib.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── tests/
│   ├── fixtures/
│   ├── integration/
│   └── e2e/
├── scripts/
├── docs/
└── package.json
```

## Frontend organization

Each `features/<module>` folder contains UI components, query hooks, form schemas, view models, and tests for that module. Shared visual primitives stay in `components`; module-specific tables and dialogs do not.

## Rust organization

- `commands`: thin IPC adapters only.
- `application`: transactional use cases.
- `domain`: entities, value objects, policies, and calculations.
- `repositories`: traits plus SQLite implementations.
- `infrastructure`: database pool, files, PDF, backup, logging, clock.
- `dto`: stable request/response types.

## Naming conventions

- Rust files, commands, and database columns: `snake_case`.
- React components and exported types: `PascalCase`.
- TypeScript variables/functions: `camelCase`.
- Commands: `<verb>_<noun>`, for example `create_product`, `complete_sale`.
- Tables: plural `snake_case`; foreign keys: `<entity>_id`.
- Migrations: timestamp or monotonic prefix plus description.

## Dependency direction

Domain code imports no Tauri, SQLx, or UI library. Application code depends on domain and repository interfaces. Infrastructure implements interfaces. Commands depend on application services. The UI only depends on IPC contracts.

