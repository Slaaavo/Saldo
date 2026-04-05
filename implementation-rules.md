# Implementation Rules

Architecture, conventions, and technical guidance for the personal finance desktop app (React + TypeScript + Vite + Tauri + Rust + SQLite, Playwright for testing).

## 1. Chosen Tech Stack
- UI: React + TypeScript + Vite
- Desktop shell: Tauri (Rust backend)
- Language for native/backend: Rust (small, focused modules)
- Local storage: SQLite
- Routing: `@tanstack/react-router` (memory history — no real browser URLs)
- Server state / async data: `@tanstack/react-query`
- End-to-end tests: Playwright Test
- Unit tests: Vitest (for TS) and Rust unit tests
- Notifications: sonner (toast notifications)
- Formatting/lint: Prettier, ESLint (TS rules), rustfmt, clippy
- Bundling/CI: Vite build for UI; Tauri build for native bundles

## 2. Project Layout

Follow this high-level structure. Domain logic is organized by feature on both sides.

- `/src-tauri/src/features/<domain>/` — Rust backend: each domain module has `commands.rs`, `repository.rs`, and optionally `models.rs`
- `/src-tauri/src/shared/` — Shared Rust helpers (`with_savepoint`, `local_now`, `convert_balance`, etc.)
- `/src-tauri/src/` (root) — Infrastructure: `error.rs`, `db.rs`, `config.rs`, `demo.rs`, `migrations.rs`, `oxr.rs`, `lib.rs`
- `/src/app/` — React application shell (App, AppModals, modal management hooks)
- `/src/features/<domain>/` — React feature modules: views, hooks, components, utils
- `/src/shared/` — Cross-cutting: `api/` (IPC wrappers), `types/`, `ui/` (shadcn/ui + custom), `layout/`, `config/`, `utils/`, `lib/`
- `/src/i18n/` — Internationalization (en, sk)
- `/migrations/` — SQL migration files
- `/tests/` — Playwright E2E tests

## 3. Data Model (core tables)

This section covers the foundational tables. Additional tables (buckets, assets, FX rates, settings, cashflow, CSV profiles, etc.) are defined in `/migrations/` and `/schema/`.

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
- **Currency amount inputs:** Use the `<CurrencyInput>` component (`src/shared/ui/CurrencyInput.tsx`) for all form inputs that accept monetary amounts. It wraps `<Input>` with an optional currency code suffix overlay.
- **Multi-currency (future):** Store original values and apply read-time conversion using time-indexed rates; store rates as high-precision scaled integers; surface provenance.

## 5. Snapshot algorithm (per selected date)
1. For each account, find the current data of the last non-deleted event: join `event` (with `deleted_at IS NULL`) to its latest `event_data` row (by `event_data.created_at DESC`), filter `event_data.event_date <= selected_datetime`, order by `event_data.event_date DESC, event.created_at DESC`, take the first. When the UI passes just a date, interpret it as end-of-day (`YYYY-MM-DDT23:59:59`) for snapshot purposes.
2. If found, `account_balance_minor = event_data.amount_minor` (events are authoritative balance snapshots).
3. If no event exists for the account (or all events are soft-deleted / after the selected date), the account balance is 0.

All snapshot and list queries must filter out soft-deleted events: `WHERE event.deleted_at IS NULL`.

When listing or displaying an event, always use the latest `event_data` row for that event. Earlier `event_data` rows are the edit history and are preserved in the database.

## 6. SQLite & migrations
- Use a simple SQL-based migration folder (`/migrations/NNN_description.sql`). Migrations are **auto-registered** at compile time — `src-tauri/build.rs` scans the `migrations/` directory, collects all `.sql` files, sorts them lexicographically, and generates the `MIGRATIONS` constant via `include_str!`. To add a new migration, simply create a new `.sql` file in `migrations/` with a zero-padded numeric prefix (e.g., `014_my_change.sql`). No Rust source edits are needed.
- Migrations are applied from Rust at startup (`migrations::run_pending`) and tracked in the `_migrations` table to avoid re-execution.
- Use WAL journaling for better concurrency and performance: `PRAGMA journal_mode = WAL;` and tune `page_size` if required.

## 7. Rust implementation guidelines
- Keep Rust surface area small: implement DB access, migrations, snapshot/aggregation logic, and a minimal command surface for the UI.
- Use `rusqlite` or `sqlx` (choose `sqlx` for async, `rusqlite` for sync simplicity). Prefer sync `rusqlite` if Rust modules remain small and synchronous.
- Use `i64` for `amount_minor`. Validate inputs at the boundary (Tauri command) and return typed errors.
- Use `serde` for serializing command responses. Add unit tests for snapshot logic.
- **Database access in commands:** Use `state.conn()?` (the `AppState::conn()` helper) to acquire a database connection in Tauri command handlers. Never call `state.db.lock().map_err(...)` directly.
- **SQL placement:** All SQL queries must live in `repository.rs` (or repository module files). Command handlers in `commands.rs` must not contain inline SQL — they validate inputs, call repository functions, and return results.
- **Core Tauri commands:** `create_balance_update`, `get_accounts_snapshot`, `list_events`, `create_account`, `update_account`, `delete_account`, `update_event` (inserts new `event_data` row), `delete_event` (soft-delete). See `src-tauri/src/features/` for full signatures.
- **Repository function params convention:** Functions with 2+ meaningful parameters use a `pub struct XxxParams` instead of individual arguments. Declare the struct in the same `repository.rs` file, just before the first `pub fn`. Use owned types (`String`, `Option<String>`, `Vec<...>`) — no lifetime parameters. Derive `Default` when the struct has optional fields so test call sites can use `..Default::default()` for `None` fields. Naming: `CreateXxxParams`, `UpdateXxxParams`, etc. Existing examples: `CreateAccountParams`, `UpdateAccountParams`, `UpdateSortOrderParams` in `accounts/repository.rs`; `ListEventsQuery` in `transactions/repository/mod.rs` (query-filter flavour of the same pattern). In `commands.rs`, the Tauri command input structs (e.g. `CreateAccountInput`) stay separate with their `#[derive(Deserialize)]`; the command handler constructs the `XxxParams` from the input, converting `&str` → `.to_owned()` and `Option<String>` → `.clone()` as needed.

## 8. TypeScript / React guidelines
- Keep business-critical calculations in Rust; React reads converted values via the command API.
- Use strict TypeScript (`strict: true`). Define interfaces for `Account`, `Event`, `EventData`, `SnapshotRow`.
- **Application state:** Server/async state is managed by `@tanstack/react-query`. Navigation is managed by `@tanstack/react-router` (memory history). Cross-cutting UI state uses React context: `ModalContext` (modal open/close), `DemoContext` (demo mode), `SelectedDateContext` (date picker). `App.tsx` is the root route shell — a thin layout that renders `<Outlet />`. Each page component is zero-prop and self-fetching. `useModalManager` (in `src/app/useModalManager`) owns modal state via a `ModalState` discriminated union type (defined in `src/shared/types/`). The old `useFinanceData` hook has been deleted.
- **Feature vs Shared separation:** Page-level views live inside their feature folder (e.g. `src/features/dashboard/DashboardView.tsx`). Reusable UI components live in `src/shared/ui/`. Feature-specific components live in `src/features/<domain>/`. Components should not import across unrelated features; use `src/shared/` for cross-cutting concerns.
- **Async calls in `useEffect`:** For API/IPC calls triggered from React effects, prefer the async/await pattern with an inner async function inside `useEffect` (e.g., define `const load = async () => { ... }` and then call `load()`). Avoid `.then()`/`.catch()` chaining in effects unless there is a specific reason to use promise chaining.
- **Component folder structure:** When a component grows complex enough to warrant sub-components extracted to their own files, convert it from a flat file (`MyComponent.tsx`) into a folder (`MyComponent/`). The folder must contain:
  - `MyComponent.tsx` — the actual component file with the default export (named the same as the folder)
  - `index.ts` — a pure barrel file with a single re-export: `export { default } from './MyComponent'`. No logic, no JSX.
  - Sub-component files (e.g. `IbanActionCell.tsx`, `PartnerCell.tsx`) — named exports, co-located as private implementation details
  
  This structure preserves TypeScript's directory import resolution (`'./csv-import/MyComponent'` resolves to `MyComponent/index.ts`) so callers are never affected by the refactor. `index.ts` must remain a pure barrel — it should never contain logic or component definitions.
- **Error handling (UI):** Use `toast.error()` from `sonner` for user-facing error messages. Never use `window.alert()`. The `<Toaster>` component is mounted in `App.tsx` with theme-aware configuration.
- **React context file split (Fast Refresh):** Vite Fast Refresh requires each file to export only components or only non-components. For context modules, put `createContext()` and `useXxx()` in `useXxx.ts` (no JSX); put only the `XxxProvider` component in `XxxContext.tsx`. The provider imports the context object from the hook file (never the other way around). Do not export the context object or non-component values from `XxxContext.tsx`. `buttonVariants` and similar non-component values should not be exported from component files unless consumed externally.
- **Shared constants:** App-wide constants (e.g., `PINNED_CURRENCY_CODES`) live in `src/shared/config/constants.ts`. Do not duplicate magic values across components.

## 9. Testing
- Unit tests:
  - Rust: snapshot logic, DB edge cases, migrations.
  - TS: UI utilities and components with Vitest.
- E2E: Playwright tests for critical user flows.

Playwright tips:
- Run headful during development for debugging; use traces for CI failures.
- Test both UI flows and IPC commands (if Tauri exposes test endpoints, invoke them or run CLI-backed tests).

Vitest patterns for TanStack Query:
- Components that call `useQueryClient()` or any query hook need a `QueryClientProvider`. Create a fresh `QueryClient` per test:
  ```ts
  function makeWrapper() {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
    return ({ children }: { children: React.ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
  }
  ```
  Pass as `{ wrapper: makeWrapper() }` to `render()` or `renderHook()`.

Vitest patterns for TanStack Router (in `vi.mock` factories):
- `vi.mock` factories that import TanStack Router hooks **must be async** and use `await import(...)`, NOT `require()`. Using `require()` loads the CJS build into a separate module registry, giving the mock a null router context at runtime.
  ```ts
  // CORRECT
  vi.mock('../shared/layout/Sidebar', async () => {
    const { useNavigate } = await import('@tanstack/react-router')
    const SidebarMock = (...) => { ... }
    return { default: SidebarMock }
  })
  // WRONG — causes 'Cannot read properties of null (reading isServer)'
  vi.mock('../shared/layout/Sidebar', () => ({
    default: () => { const { useNavigate } = require('@tanstack/react-router'); ... }
  }))
  ```
- Mock component functions inside `vi.mock` must be **named** (assigned to a `const`, not anonymous `() =>`), otherwise ESLint's `react-hooks/rules-of-hooks` will flag hook calls inside them as errors. Arrow functions assigned to `const` satisfy this requirement.
- App-level tests use `createTestRouter()` + `renderApp()` helpers that wrap in both `QueryClientProvider` and `RouterProvider` with a fresh router instance per test. See `src/app/App.test.tsx` for the reference pattern.

## 10. CI & quality
- Pre-commit: run `prettier`, `eslint --fix`, `cargo fmt`, `cargo clippy`.
- CI steps: install Rust + Node, run linters, run unit tests (TS + Rust), run a headless Playwright suite.

## 11. Packaging & distribution
- Use Tauri bundler to produce platform-native installers (Windows exe/msi, macOS dmg, Linux AppImage). Keep build artifacts small by minimizing Rust binary size (`cargo build --release` + strip).

## 12. Security & privacy
- Minimize permissions requested by the native app. Keep everything local by default.
- When DB encryption is required, adopt SQLCipher and add secure key management.

---
End of implementation rules.

## 13. TanStack Router & Query patterns

- **Memory history:** `createMemoryHistory({ initialEntries: ['/dashboard'] })`. There are no real browser URLs — this is a desktop app.
- **Route components must be zero-prop.** All data and callbacks must come from query hooks or contexts. If a route component has required props, they will be silently `undefined` at runtime (no TypeScript error). See `memory.md` for the SettingsPage bug story.
- **Shared query hooks** live in `src/shared/hooks/`. Current hooks: `useSnapshotQuery(date)`, `useDashboardEventsQuery(date)`, `useConsolidationCurrencyQuery()`, `useBulkUpdateExclusionsQuery()`.
- **Query key conventions:**
  - `['snapshot', date]`
  - `['events', 'dashboard', date]`
  - `['events', 'ledger', fromDate, toDate, accountIds, typeFilter]`
  - `['consolidation-currency']`
  - `['bulk-update-exclusions']`
  - `['fx-rates']`
- **QueryClient config:** `staleTime: Infinity`, `refetchOnWindowFocus: false`, `retry: 0`. The singleton is exported from `src/app/queryClient.ts`. A global `QueryCache.onError` handler toasts all query failures via `extractErrorMessage()`. Individual consumers do not need their own error handling unless they want custom behavior beyond the default toast.
- **Post-mutation refresh:** Call `queryClient.invalidateQueries({ queryKey: [...] })`. Never call fetch functions manually to refresh. Pass the minimum necessary key prefix (e.g., `['events']` invalidates all event queries).
- **Contexts in AppShell:** `ModalProvider` → `SelectedDateProvider` → `DemoProvider` are nested inside `App.tsx`. Any component in the tree can call `useModal()`, `useSelectedDate()`, or `useDemo()`.
- **Navigation from code:** Use `const navigate = useNavigate()` from `@tanstack/react-router`, then `navigate({ to: '/dashboard' })`.
- **Conditional UI based on route:** Use `useRouterState({ select: s => s.location.pathname })` to read the current path inside a component (e.g., Header shows date picker only on `/dashboard`).
