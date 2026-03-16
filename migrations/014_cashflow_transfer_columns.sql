-- Migration 014: Add linked_event_id to event for transfer linking, and add
-- cashflow-specific columns to event_data (counterpart, bucket, original currency/amount, fx rate).
-- event table requires a full rebuild because SQLite does not support ADD CONSTRAINT.
PRAGMA foreign_keys = OFF;
PRAGMA legacy_alter_table = ON;

BEGIN TRANSACTION;

-- Step 1: Recreate event table with linked_event_id column

CREATE TABLE event_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  account_id INTEGER NOT NULL,
  event_type TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
  deleted_at TEXT NULL,
  latest_data_id INTEGER NULL,
  linked_event_id INTEGER NULL,
  FOREIGN KEY (account_id) REFERENCES account (id) ON DELETE RESTRICT,
  FOREIGN KEY (latest_data_id) REFERENCES event_data (id) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (linked_event_id) REFERENCES event (id)
);

INSERT INTO event_new (id, account_id, event_type, created_at, deleted_at, latest_data_id, linked_event_id)
SELECT id, account_id, event_type, created_at, deleted_at, latest_data_id, NULL
FROM event;

DROP TABLE event;

ALTER TABLE event_new RENAME TO event;

-- Step 2: Recreate existing indexes on event
CREATE INDEX idx_event_account ON event (account_id);
CREATE INDEX idx_event_deleted_at ON event (deleted_at);
CREATE INDEX idx_event_latest_data ON event (latest_data_id);

-- Step 3: Recreate trigger (was defined on event_data but references event by name)
DROP TRIGGER IF EXISTS trg_eventdata_after_insert;

CREATE TRIGGER trg_eventdata_after_insert
AFTER INSERT ON event_data
BEGIN
  UPDATE event SET latest_data_id = NEW.id WHERE id = NEW.event_id;
END;

-- Step 4: Add cashflow columns to event_data
ALTER TABLE event_data ADD COLUMN counterpart_account_id INTEGER;
ALTER TABLE event_data ADD COLUMN bucket_id INTEGER;
ALTER TABLE event_data ADD COLUMN original_currency_id INTEGER;
ALTER TABLE event_data ADD COLUMN original_amount_minor INTEGER;
ALTER TABLE event_data ADD COLUMN fx_rate_mantissa INTEGER;
ALTER TABLE event_data ADD COLUMN fx_rate_exponent INTEGER;

-- Step 5: Performance indexes
CREATE INDEX idx_event_type ON event (account_id, event_type, deleted_at);
CREATE INDEX idx_event_linked ON event (linked_event_id);
CREATE INDEX idx_eventdata_counterpart ON event_data (counterpart_account_id);

COMMIT;

PRAGMA legacy_alter_table = OFF;
PRAGMA foreign_keys = ON;

PRAGMA foreign_key_check;
