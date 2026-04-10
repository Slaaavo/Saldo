-- Migration 028: Add vat_payer column to person table.
-- Indicates whether the person is a VAT payer (1) or not (0).
-- Defaults to 0. Used as the default for the per-event reclaimed_vat flag.

BEGIN TRANSACTION;

ALTER TABLE person ADD COLUMN vat_payer INTEGER NOT NULL DEFAULT 0;

COMMIT;
