# MVP Plan

Date: 2026-03-02

Purpose: capture MVP-specific scope, constraints, and implementation details for the first phase of the personal finance desktop app. For general architecture and conventions, see `implementation-rules.md`. For goals and success criteria, see `goals-and-scope.md`.

## 1. MVP Scope Summary

- Single currency: EUR only. The `currency` table exists for future extensibility, but all seeded accounts and events use EUR. No exchange rates or multi-currency aggregation.
- Single event type: `balance_update` (user-created). More types may be added later.
- Single user, single device, offline-first.
- No DB encryption. If later required, adopt SQLCipher and add secure key management.

## 2. Data Model — MVP Notes

These notes supplement the general data model defined in `implementation-rules.md`:

- All seeded accounts and events should use EUR; no exchange rates in MVP.
- Known `event_type` values for MVP: `'balance_update'` only.
- Account creation optionally creates an initial `balance_update` event (if the user provides an initial balance). An account with no events has a balance of 0.
- All `balance_update` events are editable and deletable (soft-delete) by the user.
- The MVP UI does not expose undo or edit history, but the `deleted_at` column and `event_data` history rows are present from the start for future use.
- Datetime fields store ISO 8601 strings in local time (`YYYY-MM-DDTHH:MM:SS`). The app is single-user, single-device, so timezone conversion is not needed for MVP. The UI defaults to showing only the date portion; a future setting may enable time display.

## 3. Monetary Arithmetic — MVP Rules

- There is no currency conversion in MVP; all arithmetic is within EUR.
- With single-currency integer cents displayed as EUR, no rounding is needed in calculations. Display formatting divides by 100 and always shows exactly 2 decimal places.

## 4. Snapshot & Event Display — MVP Behavior

- When listing or displaying an event, always use the latest `event_data` row. Earlier `event_data` rows are edit history and are not shown in the MVP UI, but are preserved in the database.

## 5. API — MVP-Specific Behavior

The following MVP-specific behaviors apply to the Tauri commands listed in `implementation-rules.md`:

- `delete_account`: prevent deletion if the account has any non-deleted events; otherwise delete the account.
- `create_account`: if `initial_balance_minor` is provided, also creates a `balance_update` event.

## 6. Seed Data

- Seed a default `currency` row for EUR on first migration.
- Seed a default account (e.g., 'Main Account') linked to EUR currency on first migration (no initial event; balance will be 0 until the user creates one).

## 7. UI — MVP Display

- UI displays `balance_minor` formatted as EUR (divided by 100, 2 decimal places) with the EUR symbol.
- Multi-currency display fields are deferred to future work.
- Date picker defaults to today; only the date portion of datetime fields is shown.

## 8. Testing — MVP Scenarios

- E2E (Playwright): cover creating a `balance_update`, snapshot date changes, total aggregation, and ledger listing.
- Rust unit tests: snapshot logic, DB edge cases, migrations.
- TS unit tests: UI utilities and components with Vitest.

## 9. Next Steps

- Scaffold the repo with the project layout and add initial migrations (create `currency`, `account`, `event`, `event_data` tables and seed EUR + default account).
- Implement `get_accounts_snapshot` in Rust with unit tests.

---
End of MVP plan.
