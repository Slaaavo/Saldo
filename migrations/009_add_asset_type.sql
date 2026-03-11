-- Migration 009: rebuild account table to add CHECK constraint supporting 'asset' account_type.
-- SQLite does not support ALTER TABLE ... DROP CONSTRAINT / ADD CONSTRAINT, so we recreate the table.
PRAGMA foreign_keys = OFF;
PRAGMA legacy_alter_table = ON;

BEGIN TRANSACTION;

ALTER TABLE account RENAME TO account_old;

DROP INDEX IF EXISTS idx_account_currency;

CREATE TABLE account (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  currency_id INTEGER NOT NULL,
  account_type TEXT NOT NULL DEFAULT 'account',
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
  FOREIGN KEY (currency_id) REFERENCES currency (id) ON DELETE RESTRICT,
  CHECK (account_type IN ('account', 'bucket', 'asset'))
);

INSERT INTO account (id, name, currency_id, account_type, sort_order, created_at)
SELECT id, name, currency_id, account_type, sort_order, created_at
FROM account_old;

CREATE INDEX idx_account_currency ON account (currency_id);

DROP TABLE account_old;

COMMIT;

PRAGMA legacy_alter_table = OFF;
PRAGMA foreign_keys = ON;

PRAGMA foreign_key_check;
