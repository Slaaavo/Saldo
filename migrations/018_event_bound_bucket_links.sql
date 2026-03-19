BEGIN TRANSACTION;

-- Drop old tables (data loss is intentional per PRD NG-4)
DROP TABLE IF EXISTS bucket_relative_link;
DROP TABLE IF EXISTS bucket_allocation;

-- New event-bound link table
CREATE TABLE bucket_event_link (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id          INTEGER NOT NULL REFERENCES event(id) ON DELETE CASCADE,
    source_account_id INTEGER NOT NULL REFERENCES account(id) ON DELETE RESTRICT,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
    UNIQUE (event_id, source_account_id)
);

CREATE INDEX idx_bucket_event_link_event  ON bucket_event_link (event_id);
CREATE INDEX idx_bucket_event_link_source ON bucket_event_link (source_account_id);

COMMIT;
