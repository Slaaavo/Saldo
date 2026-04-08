-- Migration 024: Add default taxable accounts for each person.
-- New account_type values 'default_revenue' and 'default_expense' hold auto-assigned
-- accounting accounts so users never manually pick accounts for taxable events.
-- Rebuild account table to expand the CHECK constraint (SQLite does not support
-- ALTER TABLE ADD CONSTRAINT, so we recreate the table).
-- Add FK columns to person for fast default account lookup.

PRAGMA foreign_keys = OFF;
PRAGMA legacy_alter_table = ON;

BEGIN TRANSACTION;

-- Rebuild account with expanded CHECK constraint

ALTER TABLE account RENAME TO account_old;

DROP INDEX IF EXISTS idx_account_currency;
DROP INDEX IF EXISTS idx_account_iban;

CREATE TABLE account (
  id                         INTEGER PRIMARY KEY AUTOINCREMENT,
  name                       TEXT    NOT NULL,
  currency_id                INTEGER NOT NULL,
  account_type               TEXT    NOT NULL DEFAULT 'account',
  sort_order                 INTEGER NOT NULL DEFAULT 0,
  iban                       TEXT,
  created_at                 TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
  person_id                  INTEGER REFERENCES person(id) ON DELETE RESTRICT,
  purchase_price_minor       INTEGER DEFAULT NULL,
  purchase_date              TEXT    DEFAULT NULL,
  depreciation_period_months INTEGER DEFAULT NULL,
  FOREIGN KEY (currency_id) REFERENCES currency (id) ON DELETE RESTRICT,
  CHECK (account_type IN ('account', 'bucket', 'asset', 'partner', 'default_revenue', 'default_expense'))
);

INSERT INTO account (id, name, currency_id, account_type, sort_order, iban, created_at,
                     person_id, purchase_price_minor, purchase_date, depreciation_period_months)
SELECT              id, name, currency_id, account_type, sort_order, iban, created_at,
                     person_id, purchase_price_minor, purchase_date, depreciation_period_months
FROM account_old;

CREATE INDEX idx_account_currency ON account (currency_id);
CREATE UNIQUE INDEX idx_account_iban ON account (iban);

DROP TABLE account_old;

-- Add default account FK columns to person (nullable until backfilled below)

ALTER TABLE person ADD COLUMN default_revenue_account_id INTEGER REFERENCES account(id);
ALTER TABLE person ADD COLUMN default_expense_account_id INTEGER REFERENCES account(id);

-- Backfill: create one default_revenue account per existing person

INSERT INTO account (name, currency_id, account_type, sort_order, person_id)
SELECT
    '__default_revenue',
    COALESCE(
        (SELECT c.id FROM currency c
         JOIN app_setting s ON s.value = c.code
         WHERE s.key = 'consolidation_currency_code'
         LIMIT 1),
        (SELECT id FROM currency ORDER BY id LIMIT 1)
    ),
    'default_revenue',
    0,
    p.id
FROM person p;

UPDATE person
SET default_revenue_account_id = (
    SELECT a.id FROM account a
    WHERE a.account_type = 'default_revenue'
      AND a.person_id = person.id
    LIMIT 1
);

-- Backfill: create one default_expense account per existing person

INSERT INTO account (name, currency_id, account_type, sort_order, person_id)
SELECT
    '__default_expense',
    COALESCE(
        (SELECT c.id FROM currency c
         JOIN app_setting s ON s.value = c.code
         WHERE s.key = 'consolidation_currency_code'
         LIMIT 1),
        (SELECT id FROM currency ORDER BY id LIMIT 1)
    ),
    'default_expense',
    0,
    p.id
FROM person p;

UPDATE person
SET default_expense_account_id = (
    SELECT a.id FROM account a
    WHERE a.account_type = 'default_expense'
      AND a.person_id = person.id
    LIMIT 1
);

COMMIT;

PRAGMA legacy_alter_table = OFF;
PRAGMA foreign_keys = ON;

PRAGMA foreign_key_check;
