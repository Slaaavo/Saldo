CREATE TABLE bucket_relative_link (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    bucket_id         INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    source_account_id INTEGER NOT NULL REFERENCES account(id) ON DELETE RESTRICT,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
    UNIQUE (bucket_id),
    UNIQUE (source_account_id)
);

CREATE INDEX idx_bucket_rel_link_bucket ON bucket_relative_link (bucket_id);
CREATE INDEX idx_bucket_rel_link_source ON bucket_relative_link (source_account_id);
