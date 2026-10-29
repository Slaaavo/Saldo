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
pnpm tauri dev          # full app (Vite on :1420 + Rust backend) on the REAL saldo.db
pnpm tauri dev --config src-tauri/tauri.dev.conf.json   # same, but as com.teamsumbal.saldo.dev with its own saldo.db
pnpm dev                # frontend only — IPC calls fail without the Tauri runtime
pnpm tauri build        # native bundle
pnpm build              # tsc + vite build

pnpm test               # Vitest (jsdom); vite.config.ts excludes tests/** (Playwright) and .claude/** (agent worktrees)
pnpm test src/features/currency/fxRate.test.ts   # single file
pnpm test -t "name"     # filter by test name
pnpm lint               # eslint src/
pnpm format             # prettier --write src/
pnpm run ci             # format:check + lint + vitest + cargo fmt --check + clippy -D warnings + cargo test
                        # (`run` is required: bare `pnpm ci` is pnpm's built-in clean install and wipes node_modules)

cd src-tauri && cargo test                  # Rust unit tests
cd src-tauri && cargo test snapshot         # filter by test name/path substring
cd src-tauri && cargo fmt && cargo clippy -- -D warnings

pnpm test:e2e           # Playwright; requires `pnpm tauri dev` already running at http://localhost:1420
```

- Husky pre-commit runs `lint-staged` (eslint --fix + prettier on staged `src/` files) and then `cargo fmt` + `cargo clippy -- -D warnings`, so clippy warnings block commits.
- E2E specs in `tests/` run against the live dev app and accumulate data across runs (each test uses a unique name prefix).
- Plain `pnpm tauri dev` uses the installed app's identifier (`com.teamsumbal.saldo`), so it opens the same `saldo.db` (or the `db-location.json` folder) and runs pending migrations on it. `src-tauri/tauri.dev.conf.json` swaps in `com.teamsumbal.saldo.dev`, whose app-data folder holds a separate `saldo.db`.
- To prove a change in the running app, use the `verify-saldo` skill (`.claude/skills/verify-saldo/`). It launches the `.dev` identifier inside a `Saldo Dev.app` wrapper, which computer use can target (the plain debug binary has no bundle id), and drives it with computer use.
- `saldo mcp` (the same binary with the argument `mcp`) serves MCP tools on stdio instead of opening the window. It opens the `saldo.db` of the identifier the binary was built with, so a plain `cargo build` binary started with `mcp` opens the real database. Verify it only through `verify-saldo`'s `mcp.sh`, which uses the dev bundle. The `record-invoices` skill (`.claude/skills/record-invoices/`) is how Claude uses it.
- PDFium (eKasa receipt PDF import) is loaded from a platform library (`pdfium.dll` / `libpdfium.dylib`) placed in `src-tauri/`. It is gitignored and downloaded separately; the platform Tauri configs bundle it as a resource.

## Architecture

### Request path

React page → query/mutation hook → `src/shared/api/index.ts` (thin `invoke('snake_case_command', { input: {...camelCase} })` wrappers) → Tauri command in `src-tauri/src/features/<domain>/commands.rs` → SQL in `repository.rs` (or `repository/*.rs`) → SQLite. Business-critical calculations (snapshots, FX conversion, headline totals, depreciation, tax) live in Rust; the UI renders what commands return.

### Backend (`src-tauri/src/`)

- `features/<domain>/` has `commands.rs` (validate input, build `XxxParams`, call repository), `repository.rs` (all SQL), `models.rs` (serde types), and sometimes pure-logic modules (`tax_models/calculation.rs`, `ekasa/qr_decoder.rs`). `features/mod.rs` glob re-exports every module.
- **Adding a command:** write it in the feature's `commands.rs`, then register it in the `generate_handler![...]` list in `lib.rs`, add a wrapper in `src/shared/api/index.ts`, and update `api-map.md`.
- `AppState` (`lib.rs`) holds a single `Mutex<rusqlite::Connection>`; commands get it via `state.conn()?`. Async commands that do HTTP (OXR FX rates in `oxr.rs`) must lock → read → drop and never hold the guard across `.await` (see `smart_fetch_fx_rates`).
- Errors are `AppError { code, message }` (`error.rs`), serialized to the frontend, where `extractErrorMessage()` turns them into toasts.
- Multi-step writes use savepoints (`with_savepoint` / `with_savepoint_app` in `shared/helpers.rs`). The outermost scope begins `BEGIN IMMEDIATE` and nested scopes are savepoints, and every connection waits up to 5 s for a lock (`db.rs`). Both exist because `saldo mcp` can write to the same file while the app runs.
- **`saldo mcp`:** `main.rs` branches on the `mcp` argument before Tauri starts. `mcp/protocol.rs` is a synchronous JSON-RPC loop, `mcp/tools.rs` is the one tool table (schemas derived from the serde inputs, no SQL), and `mcp/open.rs` opens the DB for each call and refuses instead of migrating, creating a file or falling back. The write tools call `features/entry_batches` (`record_batch`, `undo_batch`, rules in `derive.rs`), which writes through the same `_inner` functions the Tauri commands call. A command is `state.conn()?` plus one call to its `_inner`.
- **Demo mode** swaps `AppState.db` for a seeded in-memory DB (`demo.rs`) and stashes the real connection in `persistent_db`. Any code path touching the DB works in both modes.
- **Swapping the DB:** every swap of `AppState.db` (demo enter/exit, DB location changes) goes through `AppState::swap_conn` / `replace_conn`, which bump a DB generation. Code that awaits between reading and writing the DB (the OXR fetch) compares the generation before it writes and drops results meant for the other DB. After every successful swap the frontend reloads the window (`reloadAfterDbSwap.ts`), because every cached query and every setting held in component state (the person filter, the theme) belongs to the other DB.
- **DB location:** `saldo.db` lives in the OS app-data dir unless `db-location.json` there points to a custom folder (`config.rs` `resolve_db_location`). A custom folder without a `saldo.db` falls back to the default in the app and sets `fallback_warning`, so a missing file never becomes a new empty DB. `saldo mcp` refuses in that case instead.
- **Startup:** run migrations, generate due depreciation events, then fetch missing FX rates in the background (only when an `oxr_app_id` setting exists).

### Migrations

`build.rs` scans `../migrations/*.sql`, sorts them, and generates a `MIGRATIONS` const via `include_str!`. Applied state is tracked in `_migrations`. The runner (`migrations.rs`) applies each file with foreign keys off inside one transaction, runs `PRAGMA foreign_key_check`, and records the file in the same transaction, so a migration lands whole or not at all and fails on any orphan row it leaves. To add one, create `migrations/NNN_description.sql` (next number) with no Rust registration needed and no `BEGIN`/`COMMIT` or `PRAGMA foreign_keys` lines, **and** bump the hardcoded migration count asserted in `migrations_apply_cleanly` / `migrations_are_idempotent` in `src-tauri/src/db.rs`. `schema/ddl.sql` is a human-readable reference only, not applied.

### Data model essentials

- `event` is identity/lifecycle (type, account, soft-delete via `deleted_at`). `event_data` is the versioned payload: every edit inserts a new row, and `event.latest_data_id` points at the current one. Always read through `latest_data_id` and filter `deleted_at IS NULL`.
- Event types include `balance_update`, `cashflow`, `transfer` (linked pair across own accounts), and taxable `revenue` / `expense` / `prepaid_expense`. Split groups tie multiple legs together, and bucket links allocate balances to capital buckets.
- **Snapshot balance** for an account at moment D (`transactions/repository/snapshot.rs`) = the latest `balance_update` on or before D, plus the sum of `cashflow`/`transfer` amounts after that balance update and up to D, converted to the consolidation currency with the latest stored FX rate on or before D. `get_accounts_snapshot` takes `asOf`: `live` has no cutoff, so rows and rates dated after today count, while `at` takes a date (read as end of day, `T23:59:59`) or a datetime. An amount whose currency has no rate gets no value and stays out of every total. A rate older than 7 days (31 for custom units) is marked stale, aged from today when live and from the picked day otherwise (`currency/rate_age.rs`).
- **Headline metrics** (Net Worth, Liquid, Total Balance, Left to Spend), their breakdown lines and the rate warnings come back with the same snapshot, built by the pure `transactions/metrics.rs`. Each metric's total is the sum of its counted lines, and every row also appears in it as counted or not counted with a reason.
- **Tax models are a yearly forecast.** A model stores the month its actual figures cover, a monthly payroll amount, values the user typed per line, profit left from earlier years and minimum tax bands. Each of the five lines (revenue, payroll, depreciation, deductible and non-deductible costs) takes the typed value when there is one. Otherwise it follows its rule in `LineSpec::of` (`tax_models/forecast.rs`), which uses the person's revenue and expense entries, the asset's depreciation schedule, the average of the actual months, or the payroll amount. Depreciation follows its schedule, and a prepaid expense's yearly share counts once in the month it is dated, never averaged. A typed year-so-far is taken from the accountant's books, so only prepaid shares dated after it are added to its average. An asset written off at once is a plain expense. A company's bracket and minimum tax are picked by revenue, a physical person's bracket by the tax base, and a bracket's upper bound belongs to that bracket.
- **Where an entry came from:** `event.source_ref` (and `account.source_ref` for an asset) holds one prefixed reference built only by `SourceRef`: `online:` and `offline:` for eKasa receipts, `invoice:<ISSUER>/<NUMBER>` for documents recorded through `saldo mcp`. It is not unique, because split legs share it and an eKasa receipt may be imported twice on purpose. `find_recorded_source` decides "already recorded" from live rows. `entry_batch` records each MCP batch, and `batch_id` marks its entries, split legs, asset accounts and their starting balance. Prepaid shares and depreciation are found through their links.
- Money is integer minor units (`i64`). Percentages are integer basis points (10000 = 100%). Datetimes are local ISO strings `YYYY-MM-DDTHH:MM:SS`.

### Frontend (`src/`)

- `app/router.ts`: TanStack Router with **memory history** (no URLs; `/` redirects to `/dashboard`). Route components are zero-prop and self-fetching, because props on a route component are silently `undefined`.
- `app/App.tsx` is the shell (Sidebar/Header/`<Outlet />`/Toaster) wrapping the providers: `ModalProvider`, `TimeMachineProvider` (the dashboard's `asOf`, live until a date is picked), `SelectedPersonProvider`, `DemoProvider`, etc. Each context is split into `useXxx.ts` (context + hook) and `XxxContext.tsx` (provider only) for Vite Fast Refresh.
- **Modals** are driven by a single `ModalState` discriminated union (`shared/types`). Pages call `useModal().setModalState({ type: ... })`, `app/AppModals.tsx` renders the matching modal, and `app/useModalActions.ts` holds the shared mutation handlers.
- **Server state:** TanStack Query singleton in `app/queryClient.ts` (`staleTime: Infinity`, no refetch on focus, global error toast). Refresh after mutations only via `queryClient.invalidateQueries` with key prefixes (`['snapshot']`, `['events']`, `['persons']`, ...). Shared query hooks are in `shared/hooks/`. The one exception is `app/useRefreshOnExternalWrites.ts`: when the window regains focus and `PRAGMA data_version` moved (another process such as `saldo mcp` wrote), it invalidates every query.
- `features/<domain>/` holds pages, modals and hooks per domain. Do not import across unrelated features; promote shared pieces to `shared/` (`ui/` holds the shadcn/Radix primitives and app-specific inputs).
- i18n: i18next with YAML locales `src/i18n/locales/en.yml` and `sk.yml`. Add every new key to both.
- Use the `@/` alias for `src/`.

### Conventions most often relevant (details in `implementation-rules.md`)

- Display money only through `<NumberValue>`. Convert with `toMinorUnits` / `fromMinorUnits`, and use `<CurrencyInput>`, `<PercentageInput>` and `<DatePicker>` for those input kinds.
- Rust: no SQL in `commands.rs`. Repository functions with 2+ params take a `pub struct XxxParams` (owned fields, `Default` where useful). Use `div_round` for proportional money splits and floor division for tax rates.
- TS: components and hooks are `const` arrow functions (ESLint-enforced), with no explicit JSX return types. A `.tsx` file exports only components. A component that grows sub-components becomes a folder `Foo/` with `Foo.tsx` plus a pure barrel `index.ts`, and the flat `Foo.tsx` is deleted.
- Radix `<Select>` item values must never be `""`. Use sentinels like `"none"` / `"all"`.
- Vitest: wrap components and hooks using queries in a fresh `QueryClientProvider` per test. `vi.mock` factories that need TanStack Router must be `async` and use `await import(...)`. See `src/app/App.test.tsx` for the app-level `createTestRouter()` / `renderApp()` pattern.
