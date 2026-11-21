-- Setup for features/bank-checks.md "Verified rows". Running it again adds nothing.
-- Verify Guard: balance update 1000.00 on 1 Nov, coffee -12.50 on 5 Nov, a check on 10 Nov that
-- matches (987.50).
-- Verify Guard Two and Verify Guard Three: balance updates 500.00 and 200.00 on 1 Nov, a transfer of
-- 100.00 from Two to Three on 6 Nov, checks on 10 Nov that match (400.00 and 300.00).
-- Verify Booked: balance update 700.00 on 1 Nov, a booked unexplained difference of -12.50 dated
-- 8 Nov, a check on 8 Nov that matches (687.50), then the real card payment of -12.50 dated 12 Nov,
-- entered after the check. Live balance 675.00, and the bank app shows 687.50.
-- The four accounts sort first, so their cards show without scrolling the card row.

INSERT INTO account (name, currency_id, account_type, sort_order)
SELECT name, (SELECT id FROM currency WHERE code = 'EUR'), 'account', sort_order
FROM (SELECT 'Verify Guard' AS name, -4 AS sort_order
      UNION ALL SELECT 'Verify Guard Two', -3
      UNION ALL SELECT 'Verify Guard Three', -2
      UNION ALL SELECT 'Verify Booked', -1) AS wanted
WHERE NOT EXISTS (SELECT 1 FROM account a WHERE a.name = wanted.name);

UPDATE account SET sort_order = CASE name
  WHEN 'Verify Guard' THEN -4 WHEN 'Verify Guard Two' THEN -3 WHEN 'Verify Guard Three' THEN -2 ELSE -1 END
WHERE name IN ('Verify Guard', 'Verify Guard Two', 'Verify Guard Three', 'Verify Booked');

CREATE TEMP TABLE seed (
  account TEXT, event_type TEXT, amount_minor INTEGER, event_date TEXT, created_at TEXT, note TEXT, counterpart TEXT
);
INSERT INTO seed VALUES
  ('Verify Guard',       'balance_update',         100000, '2026-11-01T23:59:00', '2026-11-01T08:00:00.000', 'verify-guard opening', NULL),
  ('Verify Guard',       'cashflow',                -1250, '2026-11-05T12:00:00', '2026-11-05T08:00:00.000', 'verify-guard coffee', NULL),
  ('Verify Guard Two',   'balance_update',          50000, '2026-11-01T23:59:00', '2026-11-01T08:00:00.000', 'verify-guard-two opening', NULL),
  ('Verify Guard Two',   'transfer',               -10000, '2026-11-06T12:00:00', '2026-11-06T08:00:00.000', 'verify-guard savings', 'Verify Guard Three'),
  ('Verify Guard Three', 'balance_update',          20000, '2026-11-01T23:59:00', '2026-11-01T08:00:00.000', 'verify-guard-three opening', NULL),
  ('Verify Guard Three', 'transfer',                10000, '2026-11-06T12:00:00', '2026-11-06T08:00:00.000', 'verify-guard savings', 'Verify Guard Two'),
  ('Verify Booked',      'balance_update',          70000, '2026-11-01T23:59:00', '2026-11-01T08:00:00.000', 'verify-booked opening', NULL),
  ('Verify Booked',      'unexplained_difference',  -1250, '2026-11-08T12:00:00', '2026-11-08T08:00:00.000', NULL, NULL),
  ('Verify Booked',      'cashflow',                -1250, '2026-11-12T12:00:00', '2026-11-12T08:00:00.000', 'verify-booked card payment', NULL);

INSERT INTO event (account_id, event_type, created_at)
SELECT a.id, s.event_type, s.created_at
FROM seed s JOIN account a ON a.name = s.account
WHERE NOT EXISTS (
  SELECT 1 FROM event e
  WHERE e.account_id = a.id AND e.created_at = s.created_at AND e.event_type = s.event_type
);

INSERT INTO event_data (event_id, amount_minor, event_date, note, counterpart_account_id)
SELECT e.id, s.amount_minor, s.event_date, s.note, cp.id
FROM seed s
JOIN account a ON a.name = s.account
JOIN event e ON e.account_id = a.id AND e.created_at = s.created_at AND e.event_type = s.event_type
LEFT JOIN account cp ON cp.name = s.counterpart
WHERE e.latest_data_id IS NULL;

UPDATE event SET linked_event_id = (
  SELECT other.id FROM event other
  JOIN account oa ON oa.id = other.account_id
  JOIN account own ON own.id = event.account_id
  WHERE other.event_type = 'transfer' AND other.created_at = event.created_at
    AND oa.name IN ('Verify Guard Two', 'Verify Guard Three') AND oa.name <> own.name
)
WHERE event_type = 'transfer' AND linked_event_id IS NULL
  AND account_id IN (SELECT id FROM account WHERE name IN ('Verify Guard Two', 'Verify Guard Three'));

CREATE TEMP TABLE seed_check (account TEXT, checked_at TEXT, day TEXT, bank_balance_minor INTEGER);
INSERT INTO seed_check VALUES
  ('Verify Guard',       '2026-11-10T08:00:00.000', '2026-11-10', 98750),
  ('Verify Guard Two',   '2026-11-10T08:00:00.000', '2026-11-10', 40000),
  ('Verify Guard Three', '2026-11-10T08:00:00.000', '2026-11-10', 30000),
  ('Verify Booked',      '2026-11-08T08:00:01.000', '2026-11-08', 68750);

INSERT INTO balance_check (account_id, checked_at, day, bank_balance_minor)
SELECT a.id, c.checked_at, c.day, c.bank_balance_minor
FROM seed_check c JOIN account a ON a.name = c.account
WHERE NOT EXISTS (
  SELECT 1 FROM balance_check b WHERE b.account_id = a.id AND b.checked_at = c.checked_at
);
