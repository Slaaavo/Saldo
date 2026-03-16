-- Migration 013: Add iban column to account and expand CHECK constraint to include 'partner'.
-- SQLite does not support ALTER TABLE ... ADD CONSTRAINT, so we recreate the table.
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
  iban TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
  FOREIGN KEY (currency_id) REFERENCES currency (id) ON DELETE RESTRICT,
  CHECK (account_type IN ('account', 'bucket', 'asset', 'partner'))
);

INSERT INTO account (id, name, currency_id, account_type, sort_order, iban, created_at)
SELECT id, name, currency_id, account_type, sort_order, NULL, created_at
FROM account_old;

CREATE INDEX idx_account_currency ON account (currency_id);

CREATE UNIQUE INDEX idx_account_iban ON account (iban);

DROP TABLE account_old;

COMMIT;

PRAGMA legacy_alter_table = OFF;
PRAGMA foreign_keys = ON;

PRAGMA foreign_key_check;
