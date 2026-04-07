-- Migration 023: Add taxable event metadata columns to event_data table.
-- Used for revenue and expense event types (100 bps = 1%, 2300 bps = 23%).
-- All nullable; existing rows are unaffected.

BEGIN TRANSACTION;

ALTER TABLE event_data ADD COLUMN vat_rate_bps INTEGER DEFAULT NULL;
ALTER TABLE event_data ADD COLUMN vat_deductible_pct_bps INTEGER DEFAULT NULL;
ALTER TABLE event_data ADD COLUMN expense_deductible_pct_bps INTEGER DEFAULT NULL;
ALTER TABLE event_data ADD COLUMN prepaid_period_months INTEGER DEFAULT NULL;

COMMIT;
