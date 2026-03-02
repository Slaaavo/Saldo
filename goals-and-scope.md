# Project Goals & Scope

Date: 2026-03-02

Purpose: A minimal, offline-first desktop app to record manual balance updates and view account balances by date.

Summary:
- Provide a clear dashboard showing account balances and a total balance snapshot for a selected date (default: today).
- Record a single event type, `balance_update`, which is the only user-editable event initially.
- Support a single default currency (EUR) today while designing the data model to allow multi-currency later.

Scope Statement
- In-scope:
  - Dashboard with date picker (default: today) showing per-account balances and aggregated Total Balance.
  - Ledger of events limited to `balance_update` events.
  - Ability to create a `balance_update` per account with amount, date, and optional note.
  - Single-currency operation initially (EUR), enforced in UI and business logic.
- Out-of-scope:
  - Bank integrations, transaction imports, reconciliation, budgets, recurring transactions, multi-currency totals.

Goals & Success Metrics
- Goal 1: Display accurate per-account balances for a selected date. Metric: unit tests verifying snapshot logic; manual check.
- Goal 2: Allow adding `balance_update` events that change snapshots. Metric: create/read event flows pass acceptance tests.
- Goal 3: Keep UX minimal and fast. Metric: dashboard loads quickly on typical dev machine.

Users & Personas
- Persona: Sole account owner who wants a quick snapshot of balances and a simple way to correct or set balances.

Core UX / UI Overview
- Layout:
  - Top: date picker (defaults to today) and aggregated Total Balance.
  - Main: list of accounts with balance-as-of-selected-date.
  - Bottom: ledger (reverse-chronological) showing `balance_update` events; filter by account/date.
  - Action: per-account `Update Balance` button → modal with `amount`, `date`, `note`.
- Behavior:
  - Changing the date recalculates each account balance from events with date <= selected date.

Functional Requirements
- FR-001: Dashboard displays balances per account as of selected date (default: today).
- FR-002: Dashboard displays aggregated Total Balance as sum of account balances for that date.
- FR-003: User can create a `balance_update` event with fields: `account_id`, `amount_minor`, `date`, `note`.
- FR-004: Ledger lists events and supports filtering by account and date range.
- FR-005: Single-currency enforced (default EUR); UI shows EUR symbol and stored amounts use minor units.

Data & Event Model (high level)
- Entities:
  - `Currency` { id, code (e.g. EUR), name, minor_units }
  - `Account` { id, name, created_at, currency_id }
  - `Event` { id, account_id, event_type ('balance_update'), amount_minor, date (ISO), note, created_at }
- Storage & amounts:
  - Store monetary values as integer minor units (e.g., cents) in `amount_minor`.
  - `Account.currency_id` references `Currency.id` so accounts are tied to currencies without embedding codes.
  - Default data: create a `Currency` row for EUR and default new accounts to that currency.
- Extensibility for multi-currency later:
  - When multi-currency is enabled, introduce `ExchangeRate` and an aggregation layer that converts account balances to a display currency.
  - Keep aggregation logic abstracted so exchange-layer can be inserted without changing UI components.

Technical Constraints & Extensibility
- Storage: local SQLite (or equivalent) with simple migrations; offline-first single-user design.
- Monetary arithmetic: do all calculations in integer minor units; avoid floating point.
- Default currency: EUR; but `Currency` is a first-class entity to enable later multi-currency support.

Assumptions & Risks
- Assumptions:
  - Users will manually set balances; no bank sync is required for MVP.
  - A single user on a single device is expected initially.
- Risks:
  - Users may expect transactional history or automated imports; mitigate with clear onboarding and docs.

Milestones & Next Steps
- M1: Finalize Goals & Scope document (this file).
- M2: Draft DB schema and migrations reflecting `Currency`, `Account`, `Event` (balance_update).
- M3: Create minimal UI mock and scaffold repository with local persistence.
- M4: Implement persistence and `balance_update` flow.

Acceptance Criteria Checklist
- Dashboard shows per-account balances for the selected date.
- Default date is today.
- Total Balance equals sum of displayed account balances.
- User can add a `balance_update` event with date and amount.
- New event appears in the ledger and affects snapshots for dates >= event.date.

Notes
- Currency is modelled as a separate entity (`Currency`) and `Account` references `currency_id`. Default currency is EUR but totals assume same currency until multi-currency aggregation is implemented.
