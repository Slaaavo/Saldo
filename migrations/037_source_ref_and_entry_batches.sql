-- Migration 037: One general source reference, and the batches that recorded documents.
--
-- source_ref is not UNIQUE: the legs of a split group share it, and an eKasa receipt may be
-- imported twice on purpose.
--
-- Generated rows (prepaid shares, depreciation) carry no batch_id. They are regenerated, so
-- undo finds them through linked_prepaid_event_id and linked_asset_id.

ALTER TABLE event RENAME COLUMN ekasa_receipt_ref TO source_ref;
DROP INDEX idx_event_ekasa_receipt_ref;
CREATE INDEX idx_event_source_ref ON event (source_ref) WHERE source_ref IS NOT NULL;

CREATE TABLE entry_batch (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  label          TEXT    NOT NULL,
  document_count INTEGER NOT NULL CHECK (document_count > 0),
  created_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime')),
  undone_at      TEXT    NULL
);

ALTER TABLE event   ADD COLUMN batch_id   INTEGER NULL REFERENCES entry_batch (id);
ALTER TABLE account ADD COLUMN batch_id   INTEGER NULL REFERENCES entry_batch (id);
ALTER TABLE account ADD COLUMN source_ref TEXT    NULL;

CREATE INDEX idx_event_batch_id     ON event   (batch_id)   WHERE batch_id   IS NOT NULL;
CREATE INDEX idx_account_batch_id   ON account (batch_id)   WHERE batch_id   IS NOT NULL;
CREATE INDEX idx_account_source_ref ON account (source_ref) WHERE source_ref IS NOT NULL;

-- Accounts and payment links keep no edit history, so undo compares them with what the batch
-- made, recorded here. Deleting the account or the event in Saldo removes the row with it.

CREATE TABLE entry_batch_asset (
  account_id                 INTEGER PRIMARY KEY REFERENCES account (id) ON DELETE CASCADE,
  name                       TEXT    NOT NULL,
  purchase_price_minor       INTEGER NOT NULL,
  purchase_date              TEXT    NOT NULL,
  depreciation_period_months INTEGER NOT NULL
);

CREATE TABLE entry_batch_link (
  batch_id          INTEGER NOT NULL REFERENCES entry_batch (id),
  taxable_event_id  INTEGER NOT NULL REFERENCES event (id) ON DELETE CASCADE,
  cashflow_event_id INTEGER NOT NULL REFERENCES event (id) ON DELETE CASCADE,
  PRIMARY KEY (taxable_event_id, cashflow_event_id)
);

CREATE INDEX idx_entry_batch_link_batch_id ON entry_batch_link (batch_id);
