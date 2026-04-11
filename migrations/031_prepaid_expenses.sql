-- Migration 031: Add prepaid expense support columns.
--
-- Adds:
--   event_data.prepaid_until TEXT  — ISO date (YYYY-MM-DD) through which a prepaid
--                                    expense is valid. Replaces the never-shipped
--                                    prepaid_period_months column (see below).
--   event.linked_prepaid_event_id  — FK back to the parent prepaid_expense event for
--                                    system-generated child expense rows.
--
-- NOTE: event_data.prepaid_period_months (added in migration 023) is intentionally
-- left in place. ALTER TABLE DROP COLUMN is not used in this codebase (SQLite
-- version support is uncertain at deploy time). The column is functionally dead —
-- it is never written or read by application code going forward.

BEGIN TRANSACTION;

ALTER TABLE event_data ADD COLUMN prepaid_until TEXT DEFAULT NULL;

ALTER TABLE event ADD COLUMN linked_prepaid_event_id INTEGER DEFAULT NULL REFERENCES event(id);

CREATE INDEX IF NOT EXISTS idx_event_linked_prepaid_event_id
    ON event (linked_prepaid_event_id);

COMMIT;
