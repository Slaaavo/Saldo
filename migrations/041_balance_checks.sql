-- Migration 041: Checks against the bank app, and the unexplained difference entry.
--
-- A check stores only what the bank app showed. Whether it still matches Saldo is derived on
-- every read from the rows the check could see, so nothing here is updated after the insert.

CREATE TABLE balance_check (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  account_id         INTEGER NOT NULL REFERENCES account (id) ON DELETE CASCADE,
  -- UTC, same format and clock as event.created_at, so "created after the check" is a string compare.
  checked_at         TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f', 'now')),
  -- The user's local day of the check (YYYY-MM-DD), compared with event_data.event_date.
  checked_on         TEXT    NOT NULL CHECK (date(checked_on) IS checked_on),
  bank_balance_minor INTEGER NOT NULL
                     CHECK (bank_balance_minor BETWEEN -10000000000000 AND 10000000000000)
);

CREATE INDEX idx_balance_check_account ON balance_check (account_id, checked_at DESC);

CREATE TRIGGER trg_balance_check_own_account BEFORE INSERT ON balance_check
WHEN NOT EXISTS (SELECT 1 FROM account WHERE id = NEW.account_id AND account_type = 'account')
BEGIN SELECT RAISE(ABORT, 'Only an own account can be checked against the bank.'); END;

CREATE TRIGGER trg_unexplained_difference_is_single BEFORE INSERT ON event
WHEN NEW.event_type = 'unexplained_difference'
 AND (NEW.split_group_id IS NOT NULL
      OR NEW.linked_event_id IS NOT NULL
      OR NOT EXISTS (SELECT 1 FROM account WHERE id = NEW.account_id AND account_type = 'account'))
BEGIN SELECT RAISE(ABORT, 'An unexplained difference is a single entry on an own account.'); END;

CREATE TRIGGER trg_unexplained_difference_stays_single
BEFORE UPDATE OF event_type, account_id, split_group_id, linked_event_id ON event
WHEN NEW.event_type = 'unexplained_difference'
 AND (NEW.split_group_id IS NOT NULL
      OR NEW.linked_event_id IS NOT NULL
      OR NOT EXISTS (SELECT 1 FROM account WHERE id = NEW.account_id AND account_type = 'account'))
BEGIN SELECT RAISE(ABORT, 'An unexplained difference is a single entry on an own account.'); END;
