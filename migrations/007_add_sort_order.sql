BEGIN TRANSACTION;

ALTER TABLE account ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

-- Backfill: assign 0-based sequential sort_order within each account_type group,
-- ordered by name (with id as tiebreaker to be deterministic).
UPDATE account
SET sort_order = (
    SELECT COUNT(*)
    FROM account a2
    WHERE a2.account_type = account.account_type
      AND (a2.name < account.name OR (a2.name = account.name AND a2.id < account.id))
);

COMMIT;
