-- Migration 029: Add reclaimed_vat column to event_data table.
-- Nullable INTEGER (0/1 boolean). Only meaningful for expense events.
-- NULL means not applicable (non-expense events and pre-existing events).
-- Pre-existing expense events with NULL are treated as false (VAT not reclaimed).

BEGIN TRANSACTION;

ALTER TABLE event_data ADD COLUMN reclaimed_vat INTEGER DEFAULT NULL;

COMMIT;
