# Frontend Architecture

## Application model

The Next.js frontend is a statically exported React application hosted by Tauri. It has no direct database or filesystem access. Every trusted operation passes through a typed Tauri command wrapper.

## Feature organization

Each feature contains:

```text
features/products/
├── api.ts             # Typed command wrappers
├── queries.ts         # TanStack Query hooks and keys
├── schemas.ts         # Zod form validation
├── types.ts           # UI-facing types
├── components/
├── pages/
└── tests/
```

## State management

- Tauri command data: TanStack Query.
- Draft form data: React Hook Form.
- Short-lived UI state: component state.
- Global UI preferences: a small typed store only where needed.
- Do not copy server data into a global store.
- Invalidate narrow query keys after mutations.

## Routes

`/dashboard`, `/sales`, `/sales/new`, `/products`, `/inventory`, `/sets`, `/purchases`, `/customers`, `/suppliers`, `/deliveries`, `/expenses`, `/reports`, `/users`, `/audit`, `/settings`.

## Shared UI patterns

- `PageHeader`: title, description, primary action, optional secondary actions.
- `DataTable`: search, filters, columns, pagination, empty/loading/error state.
- `EntityPicker`: fast text search plus image thumbnail where relevant.
- `MoneyInput`: formatted display backed by an exact minor-unit parser.
- `ConfirmActionDialog`: describes consequences and optional reason.
- `StatusBadge`: controlled status tokens.
- `PrintPreview`: document preview plus printer/PDF options.

## Command wrapper

One wrapper maps Tauri command errors into typed UI errors and logs a correlation ID. Components never call raw `invoke` directly. Mutations disable duplicate submission, show pending state, and present retry only when the operation is safe to repeat.

## Forms

- Validate immediately for format and required fields.
- Let Rust revalidate all rules and permissions.
- Preserve drafts after validation errors.
- Warn before leaving a dirty form.
- Focus the first invalid control and display field-specific messages.

## Routing and permissions

Navigation hides unavailable modules for clarity, but backend permission checks remain mandatory. Direct navigation to a forbidden route shows a permission page, not a broken screen.

## Error and offline UX

The app is locally offline by design; do not display unnecessary connectivity warnings. Use helpful states for database busy, insufficient stock, missing printer, failed export, or invalid backup. Global unexpected errors show a recovery view with a reference code.

