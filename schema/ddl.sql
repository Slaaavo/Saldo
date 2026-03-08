-- saldo: canonical schema (MVP)
-- Date: 2026-03-02
-- Notes:
--  - latest_data_id is nullable to allow simple insert flow (insert event -> insert event_data -> trigger updates latest_data_id)
--  - note is nullable
--  - do not treat this file as an applied migration; apply PRAGMAs on DB connection instead

-- Recommended PRAGMAs (apply in your DB opener; shown here as comments)
-- PRAGMA foreign_keys = ON;
-- PRAGMA journal_mode = WAL;
-- PRAGMA synchronous = NORMAL;
-- PRAGMA page_size = 4096;

BEGIN TRANSACTION;

-- currency
CREATE TABLE currency (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  code TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  minor_units INTEGER NOT NULL DEFAULT 2,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now'))
);

-- account
CREATE TABLE account (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  currency_id INTEGER NOT NULL,
  account_type TEXT NOT NULL DEFAULT 'account',
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
  FOREIGN KEY (currency_id) REFERENCES currency (id) ON DELETE RESTRICT,
  CHECK (account_type IN ('account', 'bucket'))
);

-- event (lifecycle record). latest_data_id is nullable to allow simple insert flow:
-- 1) insert event (no latest_data_id), 2) insert event_data, 3) update event.latest_data_id via trigger.
CREATE TABLE event (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  account_id INTEGER NOT NULL,
  event_type TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
  deleted_at TEXT NULL,
  latest_data_id INTEGER NULL,
  FOREIGN KEY (account_id) REFERENCES account (id) ON DELETE RESTRICT,
  FOREIGN KEY (latest_data_id) REFERENCES event_data (id) DEFERRABLE INITIALLY DEFERRED
);

-- event_data (append-only payload). note is nullable.
CREATE TABLE event_data (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id INTEGER NOT NULL,
  amount_minor INTEGER NOT NULL,
  event_date TEXT NOT NULL,
  note TEXT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
  FOREIGN KEY (event_id) REFERENCES event (id) ON DELETE CASCADE
);

-- Indexes
CREATE INDEX idx_account_currency ON account (currency_id);
CREATE INDEX idx_event_account ON event (account_id);
CREATE INDEX idx_event_deleted_at ON event (deleted_at);
CREATE INDEX idx_event_latest_data ON event (latest_data_id);

CREATE INDEX idx_eventdata_event_id ON event_data (event_id);
-- Composite index to optimize snapshot queries that filter by event_date then pick latest created_at:
CREATE INDEX idx_eventdata_eventdate_eventid_createdat ON event_data (event_date, event_id, created_at DESC);
-- Note: intentionally not creating a standalone index on event_data(created_at)

-- Trigger: when a new event_data row is inserted, set event.latest_data_id to the new row's id.
CREATE TRIGGER trg_eventdata_after_insert
AFTER INSERT ON event_data
BEGIN
  UPDATE event SET latest_data_id = NEW.id WHERE id = NEW.event_id;
END;

-- bucket_allocation: links a source account to a bucket with a fixed amount at a point in time
CREATE TABLE bucket_allocation (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  bucket_id         INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  source_account_id INTEGER NOT NULL REFERENCES account(id) ON DELETE RESTRICT,
  amount_minor      INTEGER NOT NULL,
  effective_date    TEXT NOT NULL,
  created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now'))
);

CREATE INDEX idx_bucket_alloc_bucket ON bucket_allocation (bucket_id);
CREATE INDEX idx_bucket_alloc_source ON bucket_allocation (source_account_id);
CREATE INDEX idx_bucket_alloc_effective ON bucket_allocation (bucket_id, source_account_id, effective_date DESC);

COMMIT;
