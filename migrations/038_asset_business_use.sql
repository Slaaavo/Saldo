-- Migration 038: The purchase split and dated business-use share of an asset, and the
-- switch that marks an expense as also used privately.
--
-- purchase_price_minor keeps its meaning, the amount that depreciates. The three purchase
-- columns are NULL together when the user gave no VAT split. A NULL business_use_pct_bps
-- reads as 100 %, so existing assets keep their numbers.

ALTER TABLE account ADD COLUMN purchase_gross_minor INTEGER NULL;
ALTER TABLE account ADD COLUMN purchase_vat_rate_bps INTEGER NULL
  CHECK (purchase_vat_rate_bps BETWEEN 0 AND 10000);
ALTER TABLE account ADD COLUMN purchase_vat_reclaimable_pct_bps INTEGER NULL
  CHECK (
    purchase_vat_reclaimable_pct_bps BETWEEN 0 AND 10000
    AND (purchase_gross_minor IS NULL) = (purchase_vat_rate_bps IS NULL)
    AND (purchase_gross_minor IS NULL) = (purchase_vat_reclaimable_pct_bps IS NULL)
  );
ALTER TABLE account ADD COLUMN business_use_pct_bps INTEGER NULL
  CHECK (business_use_pct_bps BETWEEN 0 AND 10000);

CREATE TABLE asset_business_use_change (
  account_id           INTEGER NOT NULL REFERENCES account (id) ON DELETE CASCADE,
  from_month           TEXT    NOT NULL CHECK (from_month GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'),
  business_use_pct_bps INTEGER NOT NULL CHECK (business_use_pct_bps BETWEEN 0 AND 10000),
  PRIMARY KEY (account_id, from_month)
);

-- Which system entry of an asset this event is. A month can hold both kinds, so the
-- generator's "already exists" check must read this column.

ALTER TABLE event ADD COLUMN asset_entry TEXT NULL
  CHECK (asset_entry IN ('depreciation', 'lost_vat'));

UPDATE event
SET asset_entry = 'depreciation'
WHERE linked_asset_id IS NOT NULL
  AND is_system_generated = 1;

ALTER TABLE event_data ADD COLUMN private_use INTEGER NOT NULL DEFAULT 0
  CHECK (private_use IN (0, 1));

ALTER TABLE ekasa_import_profile_rule ADD COLUMN private_use INTEGER NOT NULL DEFAULT 0
  CHECK (private_use IN (0, 1));

ALTER TABLE entry_batch_asset ADD COLUMN purchase_gross_minor             INTEGER NULL;
ALTER TABLE entry_batch_asset ADD COLUMN purchase_vat_rate_bps            INTEGER NULL;
ALTER TABLE entry_batch_asset ADD COLUMN purchase_vat_reclaimable_pct_bps INTEGER NULL;
ALTER TABLE entry_batch_asset ADD COLUMN business_use_pct_bps             INTEGER NULL;
