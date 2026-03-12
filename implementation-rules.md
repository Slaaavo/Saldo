# Implementation Rules

Date: 2026-03-08

Purpose: capture general architecture, conventions, and technical guidance for the personal finance desktop app using React + TypeScript + Vite + Tauri + Rust + SQLite and Playwright for testing. For MVP-specific scope, constraints, and phase details, see `mvp-plan.md`.

## 1. Chosen Tech Stack
- UI: React + TypeScript + Vite
- Desktop shell: Tauri (Rust backend)
- Language for native/backend: Rust (small, focused modules)
- Local storage: SQLite
- End-to-end tests: Playwright Test
- Unit tests: Vitest (for TS) and Rust unit tests
- Notifications: sonner (toast notifications)
- Formatting/lint: Prettier, ESLint (TS rules), rustfmt, clippy
- Bundling/CI: Vite build for UI; Tauri build for native bundles

## 2. Project Layout
- /src-tauri/  — Rust + SQLite access + Tauri commands
- /src/           — React + TS UI (Vite)
  - /src/app/            — Application shell (App, AppModals, useModalManager, useModalActions)
  - /src/features/       — Domain feature modules, each containing views, hooks, components, and utils
    - /src/features/dashboard/    — Main dashboard view, account cards, ledger, metrics
    - /src/features/accounts/     — Account creation and rename modals
    - /src/features/buckets/      — Bucket allocation hook, editor, and tooltip display
    - /src/features/assets/       — Asset/units page, asset modals, unit pricing utils
    - /src/features/transactions/ — Balance update modals (create, edit, bulk)
    - /src/features/currency/     — FX rates page, currency select, rate utils
    - /src/features/settings/     — Settings page, theme/DB/demo hooks, language selector
  - /src/shared/api/     — Tauri IPC command wrappers
  - /src/shared/types/   — TypeScript type definitions (Account, Event, ModalState, etc.)
  - /src/shared/ui/      — Reusable UI components: shadcn/ui primitives + NumberValue, CurrencyInput, ConfirmDialog, ReorderModal
  - /src/shared/layout/  — App layout components (Header, Sidebar, SaldoLogo)
  - /src/shared/config/  — Configuration constants (numberFormat, pinned currencies)
  - /src/shared/utils/   — Utility functions (format, errors)
  - /src/shared/lib/     — Third-party integration helpers (cn utility for Tailwind class merging)
  - /src/i18n/       — Internationalization (en, sk locales)
  - /src/styles/     — Global CSS styles
  - /src/assets/     — Static assets (logos, images)
- /migrations/ — SQL migration files (one file per migration)
- /tests/     — Playwright tests
- package.json, Cargo.toml, vite.config.ts, tauri.conf.json

## 3. Data Model (core tables)
- `currency`   (id INTEGER PK, code TEXT UNIQUE, name TEXT, minor_units INTEGER)
- `account`    (id INTEGER PK, name TEXT, created_at TEXT, currency_id INTEGER REFERENCES currency(id))
- `event`      (id INTEGER PK, account_id INTEGER REFERENCES account(id), event_type TEXT, created_at TEXT, deleted_at TEXT, latest_data_id INTEGER REFERENCES event_data(id))
- `event_data` (id INTEGER PK, event_id INTEGER REFERENCES event(id), amount_minor INTEGER, event_date TEXT, note TEXT, created_at TEXT)

Notes:
- **`event` vs `event_data` split:** `event` is the identity/lifecycle record (who, what type, when created, soft-delete). `event_data` holds the mutable payload (amount, date, note). Every edit inserts a new `event_data` row; the **latest** `event_data` row (by `created_at DESC`) is the current version. The reference is also kept in `latest_data_id`. This gives a built-in edit-history log at no extra cost.
- `amount_minor` is integer minor units (cents). Use 64-bit integers in Rust and JS where necessary.
- `currency` exists as a first-class entity so accounts are tied to currencies without embedding codes, enabling future multi-currency support.
- `event_date` stores an ISO 8601 datetime string (`YYYY-MM-DDTHH:MM:SS`), not just a date. The schema stores full datetime to allow a future setting to show time.
- `deleted_at` (nullable) supports soft-delete. When set to a datetime string, the event is considered deleted. Soft-delete exists for audit log / edit history support.
- All datetime fields (`event_date`, `created_at`, `deleted_at`) store ISO 8601 datetime strings in local time (`YYYY-MM-DDTHH:MM:SS`).

## 4. Monetary arithmetic rules
- Store and compute in integer minor units. Use 64-bit integers (`i64` / `BigInt` if required in JS).
- Display formatting divides by the currency's minor unit factor and always shows the appropriate number of decimal places.
- **Displaying amounts in the UI:** Always use the `<NumberValue>` component (`src/shared/ui/NumberValue.tsx`). Never format amounts manually or call `formatAmount` directly in JSX. The component accepts `value` (minor units), an optional `minorUnits` prop (defaults to 2, derived from the currency model), and an optional `config` override for display preferences.
- Display preferences (currency symbol, position, thousands/decimal separators) are defined in `src/shared/config/numberFormat.ts` as a `NumberFormatConfig` object. The default config uses `€` on the right, space as thousands separator, and dot as decimal separator. This config is designed to be swappable from a future settings UI.
- **Converting between minor units and display values:** Use the utility functions in `src/shared/utils/format.ts`: `toMinorUnits(decimalStr, minorUnits)` to convert a decimal string to integer minor units, `fromMinorUnits(amountMinor, minorUnits)` to convert back to a decimal string, and `getMinorUnitsStep(minorUnits)` for HTML input `step` attributes. Never use raw `Math.pow(10, minorUnits)` arithmetic inline.
- **Currency amount inputs:** Use the `<CurrencyInput>` component (`src/shared/ui/CurrencyInput.tsx`) for all form inputs that accept monetary amounts. It wraps `<Input>` with an optional currency code suffix overlay. Never recreate the currency suffix pattern inline.

## 5. Snapshot algorithm (per selected date)
1. For each account, find the current data of the last non-deleted event: join `event` (with `deleted_at IS NULL`) to its latest `event_data` row (by `event_data.created_at DESC`), filter `event_data.event_date <= selected_datetime`, order by `event_data.event_date DESC, event.created_at DESC`, take the first. When the UI passes just a date, interpret it as end-of-day (`YYYY-MM-DDT23:59:59`) for snapshot purposes.
2. If found, `account_balance_minor = event_data.amount_minor` (events are authoritative balance snapshots).
3. If no event exists for the account (or all events are soft-deleted / after the selected date), the account balance is 0.

All snapshot and list queries must filter out soft-deleted events: `WHERE event.deleted_at IS NULL`.

When listing or displaying an event, always use the latest `event_data` row for that event. Earlier `event_data` rows are the edit history and are preserved in the database.

API exposed from Rust (Tauri commands):
- `create_balance_update(account_id, amount_minor, event_date, note) -> event_id` — creates `event` + first `event_data` row
- `get_accounts_snapshot(date_iso) -> [{account_id, name, balance_minor}]`
- `list_events(filter) -> [events]` — returns events with their current (latest) `event_data`
- `create_account(name, currency_id, initial_balance_minor?) -> account_id` — creates account; if `initial_balance_minor` is provided, also creates a `balance_update` event
- `update_account(account_id, name) -> void` — rename account
- `delete_account(account_id) -> void` — delete account (see `mvp-plan.md` for phase-specific deletion rules)
- `update_event(event_id, amount_minor, event_date, note) -> void` — inserts a new `event_data` row (previous row becomes history)
- `delete_event(event_id) -> void` — soft-delete (sets `event.deleted_at`)

## 6. Multi-currency / Exchange-layer (future)
- When adding multi-currency, follow these principles: store original values and apply read-time conversion using time-indexed rates; store rates as high-precision scaled integers; surface provenance.

## 7. SQLite & migrations
- Use a simple SQL-based migration folder (`/migrations/NN_description.sql`). Apply migrations from Rust at startup if needed.
- Use WAL journaling for better concurrency and performance: `PRAGMA journal_mode = WAL;` and tune `page_size` if required.

## 8. Rust implementation guidelines
- Keep Rust surface area small: implement DB access, migrations, snapshot/aggregation logic, and a minimal command surface for the UI.
- Use `rusqlite` or `sqlx` (choose `sqlx` for async, `rusqlite` for sync simplicity). Prefer sync `rusqlite` if Rust modules remain small and synchronous.
- Use `i64` for `amount_minor`. Validate inputs at the boundary (Tauri command) and return typed errors.
- Use `serde` for serializing command responses. Add unit tests for snapshot logic.
- **Database access in commands:** Use `state.conn()?` (the `AppState::conn()` helper) to acquire a database connection in Tauri command handlers. Never call `state.db.lock().map_err(...)` directly.
- **SQL placement:** All SQL queries must live in `repository.rs` (or repository module files). Command handlers in `commands.rs` must not contain inline SQL — they validate inputs, call repository functions, and return results.

## 9. TypeScript / React guidelines
- Keep business-critical calculations in Rust; React reads converted values via the command API.
- Use strict TypeScript (`strict: true`). Define interfaces for `Account`, `Event`, `EventData`, `SnapshotRow`.
- **Application state:** Business logic and data management is encapsulated in custom hooks. `useFinanceData` (in `src/features/dashboard/useFinanceData`) owns snapshot/events state and all mutation handlers. `useModalManager` (in `src/app/useModalManager`) owns modal state via a `ModalState` discriminated union type (defined in `src/shared/types/`). `App.tsx` is a thin composition root that wires hooks to views.
- **Feature vs Shared separation:** Page-level views live inside their feature folder (e.g. `src/features/dashboard/DashboardView.tsx`). Reusable UI components live in `src/shared/ui/`. Feature-specific components live in `src/features/<domain>/`. Components should not import across unrelated features; use `src/shared/` for cross-cutting concerns.
- **Error handling (UI):** Use `toast.error()` from `sonner` for user-facing error messages. Never use `window.alert()`. The `<Toaster>` component is mounted in `App.tsx` with theme-aware configuration.
- **Shared constants:** App-wide constants (e.g., `PINNED_CURRENCY_CODES`) live in `src/shared/config/constants.ts`. Do not duplicate magic values across components.

## 10. Testing
- Unit tests:
  - Rust: snapshot logic, DB edge cases, migrations.
  - TS: UI utilities and components with Vitest.
- E2E: Playwright tests for critical user flows.

Playwright tips:
- Run headful during development for debugging; use traces for CI failures.
- Test both UI flows and IPC commands (if Tauri exposes test endpoints, invoke them or run CLI-backed tests).

## 11. CI & quality
- Pre-commit: run `prettier`, `eslint --fix`, `cargo fmt`, `cargo clippy`.
- CI steps: install Rust + Node, run linters, run unit tests (TS + Rust), run a headless Playwright suite.

## 12. Packaging & distribution
- Use Tauri bundler to produce platform-native installers (Windows exe/msi, macOS dmg, Linux AppImage). Keep build artifacts small by minimizing Rust binary size (`cargo build --release` + strip).

## 13. Security & privacy
- Minimize permissions requested by the native app. Keep everything local by default.
- When DB encryption is required, adopt SQLCipher and add secure key management.

## 14. Developer commands (examples)
```
# Install JS deps
pnpm install

# Run UI dev server
pnpm dev

# Run Tauri dev (UI + native)
pnpm tauri dev

# Run tests
pnpm test    # plays TS unit tests
cargo test   # runs Rust unit tests
pnpm playwright test  # runs Playwright E2E
```

---
End of implementation rules.
