-- Migration 005: create fx_rate and app_setting tables; seed consolidation currency
BEGIN TRANSACTION;

CREATE TABLE fx_rate (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  date             TEXT NOT NULL,
  from_currency_id INTEGER NOT NULL,
  to_currency_id   INTEGER NOT NULL,
  rate_mantissa    INTEGER NOT NULL,
  rate_exponent    INTEGER NOT NULL,
  is_manual        INTEGER NOT NULL DEFAULT 0,
  fetched_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
  FOREIGN KEY (from_currency_id) REFERENCES currency (id) ON DELETE RESTRICT,
  FOREIGN KEY (to_currency_id)   REFERENCES currency (id) ON DELETE RESTRICT,
  UNIQUE (date, from_currency_id, to_currency_id)
);

CREATE INDEX idx_fxrate_date_from_to ON fx_rate (date DESC, from_currency_id, to_currency_id);

CREATE TABLE app_setting (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

INSERT INTO app_setting (key, value) VALUES ('consolidation_currency_code', 'EUR');

COMMIT;
