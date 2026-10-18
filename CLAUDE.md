# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Saldo is an offline-first, single-user desktop app for personal/family finances: React + TypeScript + Vite frontend, Tauri 2 + Rust + SQLite (`rusqlite`, bundled) backend. The core idea is low-maintenance tracking: occasional balance updates are enough, with cash flows, transfers, partners, CSV/eKasa import, buckets, assets and tax models layered on top.

## Project rule docs — read before planning or implementing

- `implementation-rules.md` — the authoritative conventions document (data model semantics, money rules, Rust/TS patterns, testing patterns, feature-specific gotchas). Consult it as a constraint reference for any feature work; do not duplicate it here.
- `memory.md` — local, gitignored dated log of what was built and why. Use as context.
- `api-map.md` — maps each frontend API function to its backend command and its call sites.
- `.agents/skills/remember` — the procedure for persisting new learnings: narrative "what/why" goes to `memory.md` (append at bottom under `## YYYY-MM-DD`), "how" rules go to `implementation-rules.md`; confirm candidates with the user before writing.
- PRDs / business planning docs must not contain implementation details (DDL, structs, file paths, API signatures). Technical/implementation plans should include full technical detail. Plans live in `plans/` (gitignored).
- Edit file contents with the Edit/Write tools, never with shell commands (sed, heredocs, etc.) — a hard constraint from the project's agent instructions.

## Commands

Package manager is **pnpm** (`pnpm-lock.yaml`; Tauri's `beforeDevCommand` runs `pnpm dev`).

```bash
pnpm install
pnpm tauri dev          # full app (Vite on :1420 + Rust backend)
pnpm dev                # frontend only — IPC calls fail without the Tauri runtime
pnpm tauri build        # native bundle
pnpm build              # tsc + vite build

pnpm test               # Vitest (jsdom); tests/** is excluded via vite.config.ts
pnpm test src/features/currency/fxRate.test.ts   # single file
pnpm test -t "name"     # filter by test name
pnpm lint               # eslint src/
pnpm format             # prettier --write src/
pnpm ci                 # format:check + lint + vitest + cargo fmt --check + clippy -D warnings + cargo test

cd src-tauri && cargo test                  # Rust unit tests
cd src-tauri && cargo test snapshot         # filter by test name/path substring
cd src-tauri && cargo fmt && cargo clippy -- -D warnings

pnpm test:e2e           # Playwright; requires `pnpm tauri dev` already running at http://localhost:1420
```

- Husky pre-commit runs `lint-staged` (eslint --fix + prettier on staged `src/` files) and then `cargo fmt` + `cargo clippy -- -D warnings`, so clippy warnings block commits.
- E2E specs in `tests/` run against the live dev app and accumulate data across runs (each test uses a unique name prefix).
- PDFium (eKasa receipt PDF import) is loaded from a platform library (`pdfium.dll` / `libpdfium.dylib`) placed in `src-tauri/`. It is gitignored and downloaded separately; the platform Tauri configs bundle it as a resource.

## Architecture

### Request path

React page → query/mutation hook → `src/shared/api/index.ts` (thin `invoke('snake_case_command', { input: {...camelCase} })` wrappers) → Tauri command in `src-tauri/src/features/<domain>/commands.rs` → SQL in `repository.rs` (or `repository/*.rs`) → SQLite. Business-critical calculations (snapshots, FX conversion, depreciation, tax) live in Rust; the UI renders what commands return.

### Backend (`src-tauri/src/`)

- `features/<domain>/` has `commands.rs` (validate input, build `XxxParams`, call repository), `repository.rs` (all SQL), `models.rs` (serde types), and sometimes pure-logic modules (`tax_models/calculation.rs`, `ekasa/qr_decoder.rs`). `features/mod.rs` glob re-exports every module.
- **Adding a command:** write it in the feature's `commands.rs`, then register it in the `generate_handler![...]` list in `lib.rs`, add a wrapper in `src/shared/api/index.ts`, and update `api-map.md`.
- `AppState` (`lib.rs`) holds a single `Mutex<rusqlite::Connection>`; commands get it via `state.conn()?`. Async commands that do HTTP (OXR FX rates in `oxr.rs`) must lock → read → drop and never hold the guard across `.await` (see `smart_fetch_fx_rates`).
- Errors are `AppError { code, message }` (`error.rs`), serialized to the frontend, where `extractErrorMessage()` turns them into toasts.
- Multi-step writes use savepoints (`with_savepoint` / `with_savepoint_app` in `shared/helpers.rs`).
- **Demo mode** swaps `AppState.db` for a seeded in-memory DB (`demo.rs`) and stashes the real connection in `persistent_db`. Any code path touching the DB works in both modes.
- **DB location:** `saldo.db` lives in the OS app-data dir unless `db-location.json` there points to a custom folder (`config.rs`). A missing custom folder falls back to the default and sets `fallback_warning`.
- **Startup:** run migrations, generate due depreciation events, then fetch missing FX rates in the background (only when an `oxr_app_id` setting exists).

### Migrations

`build.rs` scans `../migrations/*.sql`, sorts them, and generates a `MIGRATIONS` const via `include_str!`. Applied state is tracked in `_migrations`. To add one, create `migrations/NNN_description.sql` (next number) with no Rust registration needed, **and** bump the hardcoded migration count asserted in `migrations_apply_cleanly` / `migrations_are_idempotent` in `src-tauri/src/db.rs`. `schema/ddl.sql` is a human-readable reference only, not applied.

### Data model essentials

- `event` is identity/lifecycle (type, account, soft-delete via `deleted_at`). `event_data` is the versioned payload: every edit inserts a new row, and `event.latest_data_id` points at the current one. Always read through `latest_data_id` and filter `deleted_at IS NULL`.
- Event types include `balance_update`, `cashflow`, `transfer` (linked pair across own accounts), and taxable `revenue` / `expense` / `prepaid_expense`. Split groups tie multiple legs together, and bucket links allocate balances to capital buckets.
- **Snapshot balance** for an account at date D (`transactions/repository/snapshot.rs`) = the latest `balance_update` on or before D, plus the sum of `cashflow`/`transfer` amounts after that balance update and up to D, converted to the consolidation currency via stored FX rates. Dates passed from the UI are end-of-day (`T23:59:59`). (`implementation-rules.md` §5 still describes the older balance-update-only version.)
- Money is integer minor units (`i64`). Percentages are integer basis points (10000 = 100%). Datetimes are local ISO strings `YYYY-MM-DDTHH:MM:SS`.

### Frontend (`src/`)

- `app/router.ts`: TanStack Router with **memory history** (no URLs; `/` redirects to `/dashboard`). Route components are zero-prop and self-fetching, because props on a route component are silently `undefined`.
- `app/App.tsx` is the shell (Sidebar/Header/`<Outlet />`/Toaster) wrapping the providers: `ModalProvider`, `SelectedDateProvider`, `SelectedPersonProvider`, `DemoProvider`, etc. Each context is split into `useXxx.ts` (context + hook) and `XxxContext.tsx` (provider only) for Vite Fast Refresh.
- **Modals** are driven by a single `ModalState` discriminated union (`shared/types`). Pages call `useModal().setModalState({ type: ... })`, `app/AppModals.tsx` renders the matching modal, and `app/useModalActions.ts` holds the shared mutation handlers.
- **Server state:** TanStack Query singleton in `app/queryClient.ts` (`staleTime: Infinity`, no refetch on focus, global error toast). Refresh after mutations only via `queryClient.invalidateQueries` with key prefixes (`['snapshot']`, `['events']`, `['persons']`, ...). Shared query hooks are in `shared/hooks/`.
- `features/<domain>/` holds pages, modals and hooks per domain. Do not import across unrelated features; promote shared pieces to `shared/` (`ui/` holds the shadcn/Radix primitives and app-specific inputs).
- i18n: i18next with YAML locales `src/i18n/locales/en.yml` and `sk.yml`. Add every new key to both.
- Use the `@/` alias for `src/`.

### Conventions most often relevant (details in `implementation-rules.md`)

- Display money only through `<NumberValue>`. Convert with `toMinorUnits` / `fromMinorUnits`, and use `<CurrencyInput>`, `<PercentageInput>` and `<DatePicker>` for those input kinds.
- Rust: no SQL in `commands.rs`. Repository functions with 2+ params take a `pub struct XxxParams` (owned fields, `Default` where useful). Use `div_round` for proportional money splits and floor division for tax rates.
- TS: components and hooks are `const` arrow functions (ESLint-enforced), with no explicit JSX return types. A `.tsx` file exports only components. A component that grows sub-components becomes a folder `Foo/` with `Foo.tsx` plus a pure barrel `index.ts`, and the flat `Foo.tsx` is deleted.
- Radix `<Select>` item values must never be `""`. Use sentinels like `"none"` / `"all"`.
- Vitest: wrap components and hooks using queries in a fresh `QueryClientProvider` per test. `vi.mock` factories that need TanStack Router must be `async` and use `await import(...)`. See `src/app/App.test.tsx` for the app-level `createTestRouter()` / `renderApp()` pattern.
