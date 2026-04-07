-- Migration 022: Add asset depreciation metadata columns to account table.
-- Only meaningful for asset accounts (account_type = 'asset').
-- All nullable; existing rows are unaffected.

BEGIN TRANSACTION;

ALTER TABLE account ADD COLUMN purchase_price_minor INTEGER DEFAULT NULL;
ALTER TABLE account ADD COLUMN purchase_date TEXT DEFAULT NULL;
ALTER TABLE account ADD COLUMN depreciation_period_months INTEGER DEFAULT NULL;

COMMIT;
