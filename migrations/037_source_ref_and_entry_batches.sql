-- Migration 037: One general source reference, and the batches that recorded documents.
--
-- event.ekasa_receipt_ref becomes event.source_ref. It holds a canonical, prefixed reference
-- built only by SourceRef in Rust: 'online:<id>' / 'offline:<code>' for eKasa receipts (values
-- unchanged) or 'invoice:<ISSUER>/<NUMBER>'. Every leg of a split group carries the same
-- reference, and the app may import an eKasa receipt twice on purpose, so it stays NOT UNIQUE.
--
-- An asset is an account, not an event, so account gets its own source_ref.
--
-- batch_id marks the rows a batch created: entries, split legs, asset accounts and their
-- initial balance update. Generated rows (prepaid shares, depreciation) are found through
-- linked_prepaid_event_id and linked_asset_id instead, so regenerating them never loses them.

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
