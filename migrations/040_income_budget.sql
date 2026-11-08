ALTER TABLE account ADD COLUMN bucket_kind TEXT NOT NULL DEFAULT 'balance'
  CHECK (bucket_kind IN ('balance', 'income_budget')
         AND (bucket_kind = 'balance' OR (account_type = 'bucket' AND person_id IS NOT NULL)));

CREATE UNIQUE INDEX idx_account_one_income_budget_per_person
  ON account (person_id) WHERE bucket_kind = 'income_budget';

CREATE TABLE income_budget_month (
  bucket_id    INTEGER NOT NULL REFERENCES account (id) ON DELETE CASCADE,
  month        TEXT    NOT NULL CHECK (month GLOB '[0-9][0-9][0-9][0-9]-0[1-9]'
                                    OR month GLOB '[0-9][0-9][0-9][0-9]-1[0-2]'),
  budget_minor INTEGER NULL CHECK (budget_minor >= 0),
  vat_bps      INTEGER NULL CHECK (vat_bps BETWEEN 0 AND 10000),
  PRIMARY KEY (bucket_id, month),
  CHECK (budget_minor IS NOT NULL OR vat_bps IS NOT NULL)
);

CREATE TRIGGER trg_income_budget_owns_no_events BEFORE INSERT ON event
WHEN EXISTS (SELECT 1 FROM account WHERE id = NEW.account_id AND bucket_kind = 'income_budget')
BEGIN SELECT RAISE(ABORT, 'An income budget is computed from its budget and takes no entries.'); END;

CREATE TRIGGER trg_income_budget_takes_no_tags BEFORE INSERT ON event_data
WHEN NEW.bucket_id IS NOT NULL
 AND EXISTS (SELECT 1 FROM account WHERE id = NEW.bucket_id AND bucket_kind = 'income_budget')
BEGIN SELECT RAISE(ABORT, 'A cashflow cannot be tagged to an income budget.'); END;

CREATE TRIGGER trg_bucket_kind_is_fixed BEFORE UPDATE OF bucket_kind ON account
WHEN NEW.bucket_kind IS NOT OLD.bucket_kind
BEGIN SELECT RAISE(ABORT, 'A bucket keeps the kind it was created with.'); END;
