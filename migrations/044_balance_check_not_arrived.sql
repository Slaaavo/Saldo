CREATE TABLE balance_check_not_arrived (
  check_id INTEGER NOT NULL REFERENCES balance_check (id) ON DELETE CASCADE,
  event_id INTEGER NOT NULL REFERENCES event (id) ON DELETE CASCADE,
  PRIMARY KEY (check_id, event_id)
);

CREATE TRIGGER trg_balance_check_not_arrived_is_own_transfer BEFORE INSERT ON balance_check_not_arrived
WHEN NOT EXISTS (
  SELECT 1
  FROM balance_check bc
  JOIN event e ON e.account_id = bc.account_id
  WHERE bc.id = NEW.check_id
    AND e.id = NEW.event_id
    AND e.event_type = 'transfer'
)
BEGIN SELECT RAISE(ABORT, 'Only a transfer on the checked account can be left out of a check.'); END;
