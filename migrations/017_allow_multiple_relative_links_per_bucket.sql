-- Remove the UNIQUE (bucket_id) constraint so a bucket can have multiple
-- relative-linked accounts. The UNIQUE (source_account_id) constraint is kept
-- to ensure each account can only be relatively linked to at most one bucket.

CREATE TABLE bucket_relative_link_new (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    bucket_id         INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    source_account_id INTEGER NOT NULL REFERENCES account(id) ON DELETE RESTRICT,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
    UNIQUE (source_account_id)
);

INSERT INTO bucket_relative_link_new SELECT id, bucket_id, source_account_id, created_at FROM bucket_relative_link;

DROP TABLE bucket_relative_link;

ALTER TABLE bucket_relative_link_new RENAME TO bucket_relative_link;

CREATE INDEX idx_bucket_rel_link_bucket ON bucket_relative_link (bucket_id);
CREATE INDEX idx_bucket_rel_link_source ON bucket_relative_link (source_account_id);
