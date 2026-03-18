-- Migration 015: Add split_group table and split_group_id FK on event.
-- event table requires a full rebuild because SQLite does not support ADD CONSTRAINT.
PRAGMA foreign_keys = OFF;
PRAGMA legacy_alter_table = ON;

BEGIN TRANSACTION;

-- Step 1: Create split_group table
CREATE TABLE split_group (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    note       TEXT    NULL,
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now'))
);

-- Step 2: Recreate event table with split_group_id FK column
CREATE TABLE event_new (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id      INTEGER NOT NULL,
    event_type      TEXT    NOT NULL,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
    deleted_at      TEXT    NULL,
    latest_data_id  INTEGER NULL,
    linked_event_id INTEGER NULL,
    split_group_id  INTEGER NULL,
    FOREIGN KEY (account_id)      REFERENCES account(id) ON DELETE RESTRICT,
    FOREIGN KEY (latest_data_id)  REFERENCES event_data(id) DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (linked_event_id) REFERENCES event(id),
    FOREIGN KEY (split_group_id)  REFERENCES split_group(id)
);

INSERT INTO event_new (id, account_id, event_type, created_at, deleted_at, latest_data_id, linked_event_id, split_group_id)
SELECT id, account_id, event_type, created_at, deleted_at, latest_data_id, linked_event_id, NULL
FROM event;

DROP TABLE event;

ALTER TABLE event_new RENAME TO event;

-- Step 3: Recreate existing indexes on event
CREATE INDEX idx_event_account     ON event (account_id);
CREATE INDEX idx_event_deleted_at  ON event (deleted_at);
CREATE INDEX idx_event_latest_data ON event (latest_data_id);
CREATE INDEX idx_event_type        ON event (account_id, event_type, deleted_at);
CREATE INDEX idx_event_linked      ON event (linked_event_id);

-- Step 4: New index for split group lookups
CREATE INDEX idx_event_split_group ON event (split_group_id);

-- Step 5: Recreate trigger (defined on event_data, references event by name)
DROP TRIGGER IF EXISTS trg_eventdata_after_insert;

CREATE TRIGGER trg_eventdata_after_insert
AFTER INSERT ON event_data
BEGIN
    UPDATE event SET latest_data_id = NEW.id WHERE id = NEW.event_id;
END;

COMMIT;

PRAGMA legacy_alter_table = OFF;
PRAGMA foreign_keys = ON;

PRAGMA foreign_key_check;
