BEGIN TRANSACTION;

CREATE TABLE import_profile (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  column_mapping_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE import_profile_rule (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  profile_id INTEGER NOT NULL REFERENCES import_profile(id) ON DELETE CASCADE,
  rule_type TEXT NOT NULL,
  sort_order INTEGER NOT NULL,
  params_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_import_profile_rule_profile_id ON import_profile_rule(profile_id);

ALTER TABLE account ADD COLUMN preferred_profile_id INTEGER REFERENCES import_profile(id) ON DELETE SET NULL;

COMMIT;
