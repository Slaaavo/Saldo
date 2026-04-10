-- Migration 030: Rename vat_deductible_pct_bps to vat_reclaimable_pct_bps in event_data.
-- Requires SQLite >= 3.25.0 (bundled with rusqlite).
-- The new name reflects that this field controls the percentage of VAT that can be reclaimed,
-- not all of which may be deductible in every jurisdiction.

BEGIN TRANSACTION;

ALTER TABLE event_data RENAME COLUMN vat_deductible_pct_bps TO vat_reclaimable_pct_bps;

COMMIT;
