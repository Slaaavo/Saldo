BEGIN TRANSACTION;

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
