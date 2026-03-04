# Implementation Rules

Date: 2026-03-02

Purpose: capture concrete implementation choices, conventions, and minimal technical guidance for the MVP using React + TypeScript + Vite + Tauri + Rust + SQLite and Playwright for testing.

## 1. Chosen Tech Stack (MVP)
- UI: React + TypeScript + Vite
- Desktop shell: Tauri (Rust backend)
- Language for native/backend: Rust (small, focused modules)
- Local storage: SQLite (no encryption for MVP)
- End-to-end tests: Playwright Test
- Unit tests: Vitest (for TS) and Rust unit tests
- Formatting/lint: Prettier, ESLint (TS rules), rustfmt, clippy
- Bundling/CI: Vite build for UI; Tauri build for native bundles

## 2. Project Layout (suggested)
- /src-tauri/  — Rust + SQLite access + Tauri commands
- /src/       — React + TS UI (Vite)
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
- MVP: single currency (EUR) only. `currency` exists for future extensibility but all seeded accounts and events should use EUR; no exchange rates in MVP.
- `event_date` stores an ISO 8601 datetime string (`YYYY-MM-DDTHH:MM:SS`), not just a date. In the UI, only the date portion is shown by default; the schema stores full datetime to allow a future setting to show time.
- `deleted_at` (nullable) supports soft-delete. When set to a datetime string, the event is considered deleted. Soft-delete exists for future audit log / edit history support. The MVP UI does not expose undo or history, but the column is present from the start.
- Known `event_type` values: `'balance_update'` (user-created). More types may be added later.
- Account creation optionally creates an initial `balance_update` event (if the user provides an initial balance). An account with no events has a balance of 0.
- All `balance_update` events are editable and deletable (soft-delete) by the user.
- All datetime fields (`event_date`, `created_at`, `deleted_at`) store ISO 8601 datetime strings in local time (`YYYY-MM-DDTHH:MM:SS`). The app is single-user, single-device, so timezone conversion is not needed for MVP. The UI defaults to showing only the date portion; a future setting may enable time display.

## 4. Monetary arithmetic rules
- Store and compute in integer minor units. Use 64-bit integers (`i64` / `BigInt` if required in JS).
- There is no currency conversion in MVP; all arithmetic is within the single currency (EUR).
- Rounding rule: for MVP with single-currency integer cents displayed as EUR, no rounding is needed in calculations. Display formatting divides by 100 and always shows exactly 2 decimal places. This trivial rule is documented here to close the decision.

## 5. Snapshot algorithm (per selected date)
1. For each account, find the current data of the last non-deleted event: join `event` (with `deleted_at IS NULL`) to its latest `event_data` row (by `event_data.created_at DESC`), filter `event_data.event_date <= selected_datetime`, order by `event_data.event_date DESC, event.created_at DESC`, take the first. When the UI passes just a date, interpret it as end-of-day (`YYYY-MM-DDT23:59:59`) for snapshot purposes.
2. If found, `account_balance_minor = event_data.amount_minor` (events are authoritative balance snapshots).
3. If no event exists for the account (or all events are soft-deleted / after the selected date), the account balance is 0.

All snapshot and list queries must filter out soft-deleted events: `WHERE event.deleted_at IS NULL`.

When listing or displaying an event, always use the latest `event_data` row for that event. Earlier `event_data` rows are the edit history and are not shown in MVP UI, but are preserved in the database.

API exposed from Rust (Tauri commands):
- `create_balance_update(account_id, amount_minor, event_date, note) -> event_id` — creates `event` + first `event_data` row
- `get_accounts_snapshot(date_iso) -> [{account_id, name, balance_minor}]`
- `list_events(filter) -> [events]` — returns events with their current (latest) `event_data`
- `create_account(name, currency_id, initial_balance_minor?) -> account_id` — creates account; if `initial_balance_minor` is provided, also creates a `balance_update` event
- `update_account(account_id, name) -> void` — rename account
- `delete_account(account_id) -> void` — for MVP, prevent deletion if account has any non-deleted events; otherwise delete the account
- `update_event(event_id, amount_minor, event_date, note) -> void` — inserts a new `event_data` row (previous row becomes history)
- `delete_event(event_id) -> void` — soft-delete (sets `event.deleted_at`)

## 6. Multi-currency / Exchange-layer (future)
- MVP intentionally omits exchange rates and multi-currency aggregation. Keep the code and schema simple.
- When adding multi-currency later, follow these principles: store original values and apply read-time conversion using time-indexed rates; store rates as high-precision scaled integers; surface provenance.

## 7. SQLite & migrations
- Use a simple SQL-based migration folder (`/migrations/NN_description.sql`). Apply migrations from Rust at startup if needed.
- Use WAL journaling for better concurrency and performance: `PRAGMA journal_mode = WAL;` and tune `page_size` if required.
- Seed a default `currency` row for EUR on first migration.
- Seed a default account (e.g., 'Main Account') linked to EUR currency on first migration (no initial event; balance will be 0 until the user creates one).

## 8. Rust implementation guidelines
- Keep Rust surface area small: implement DB access, migrations, snapshot/aggregation logic, and a minimal command surface for the UI.
- Use `rusqlite` or `sqlx` (choose `sqlx` for async, `rusqlite` for sync simplicity). Prefer sync `rusqlite` if Rust modules remain small and synchronous.
- Use `i64` for `amount_minor`. Validate inputs at the boundary (Tauri command) and return typed errors.
- Use `serde` for serializing command responses. Add unit tests for snapshot logic.

## 9. TypeScript / React guidelines
- Keep business-critical calculations in Rust; React reads converted values via the command API.
- Use strict TypeScript (`strict: true`). Define interfaces for `Account`, `Event`, `EventData`, `SnapshotRow`.
- UI displays `balance_minor` formatted as EUR (divided by 100, 2 decimal places) with the EUR symbol. Multi-currency display fields are deferred to future work.

## 10. Testing
- Unit tests:
  - Rust: snapshot logic, DB edge cases, migrations.
  - TS: UI utilities and components with Vitest.
- E2E: Playwright tests that cover: creating a balance_update, snapshot date changes, total aggregation, ledger listing.

Playwright tips:
- Run headful during development for debugging; use traces for CI failures.
- Test both UI flows and IPC commands (if Tauri exposes test endpoints, invoke them or run CLI-backed tests).

## 11. CI & quality
- Pre-commit: run `prettier`, `eslint --fix`, `cargo fmt`, `cargo clippy`.
- CI steps: install Rust + Node, run linters, run unit tests (TS + Rust), run a headless Playwright suite.

## 12. Packaging & distribution
- Use Tauri bundler to produce platform-native installers (Windows exe/msi, macOS dmg, Linux AppImage). Keep build artifacts small by minimizing Rust binary size (`cargo build --release` + strip).

## 13. Security & privacy
- No DB encryption in MVP. If later required, adopt SQLCipher and add secure key management.
- Minimize permissions requested by the native app. Keep everything local by default.

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

## 15. Next steps
- Scaffold the repo with the above layout and add initial migrations (create `currency`, `account`, `event`, `event_data` tables and seed EUR + default account).
- Implement `get_accounts_snapshot` in Rust with unit tests.

---
End of implementation rules.
