BEGIN TRANSACTION;

CREATE TABLE ekasa_import_profile (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  person_id INTEGER NOT NULL REFERENCES person(id) ON DELETE CASCADE,
  default_deductible_pct_bps INTEGER NOT NULL DEFAULT 10000,
  default_vat_reclaimable_pct_bps INTEGER NOT NULL DEFAULT 10000,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (person_id)
);

CREATE TABLE ekasa_import_profile_rule (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  profile_id INTEGER NOT NULL REFERENCES ekasa_import_profile(id) ON DELETE CASCADE,
  sort_order INTEGER NOT NULL,
  name_pattern TEXT NOT NULL,
  deductible_pct_bps INTEGER NOT NULL,
  vat_reclaimable_pct_bps INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ekasa_import_profile_rule_profile_id ON ekasa_import_profile_rule(profile_id);

COMMIT;
