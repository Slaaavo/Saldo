-- Setup for features/bank-checks.md "Causes". Running it again adds nothing.
-- Verify Causes: balance update 1000.00 on 1 Nov, coffee -12.50 on 5 Nov, a check on 10 Nov that matched
-- (987.50), then three rows entered after the check: a copy of the coffee dated 7 Nov, parking +30.00
-- (sign flipped), groceries -45.00. Live balance 960.00.
-- Verify Stale: imported from CSV (preferred profile Verify CSV), balance update 500.00 on 1 Nov, newest
-- row -20.00 on 10 Nov. Live balance 480.00.

INSERT INTO import_profile (name, column_mapping_json, created_at, updated_at)
SELECT 'Verify CSV', '{"date":"Date","amount":"Amount","note":"Note"}',
       strftime('%Y-%m-%dT%H:%M:%S', 'now'), strftime('%Y-%m-%dT%H:%M:%S', 'now')
WHERE NOT EXISTS (SELECT 1 FROM import_profile WHERE name = 'Verify CSV');

INSERT INTO account (name, currency_id, account_type, sort_order)
SELECT 'Verify Causes', (SELECT id FROM currency WHERE code = 'EUR'), 'account', 91
WHERE NOT EXISTS (SELECT 1 FROM account WHERE name = 'Verify Causes');

INSERT INTO account (name, currency_id, account_type, sort_order)
SELECT 'Verify Stale', (SELECT id FROM currency WHERE code = 'EUR'), 'account', 92
WHERE NOT EXISTS (SELECT 1 FROM account WHERE name = 'Verify Stale');

UPDATE account SET preferred_profile_id = (SELECT id FROM import_profile WHERE name = 'Verify CSV')
WHERE name = 'Verify Stale';

CREATE TEMP TABLE seed (
  account TEXT, event_type TEXT, amount_minor INTEGER, event_date TEXT, created_at TEXT, note TEXT
);
INSERT INTO seed VALUES
  ('Verify Causes', 'balance_update', 100000, '2026-11-01T23:59:00', '2026-11-01T08:00:00.000', 'verify-causes opening'),
  ('Verify Causes', 'cashflow',        -1250, '2026-11-05T12:00:00', '2026-11-05T08:00:00.000', 'verify-causes coffee'),
  ('Verify Causes', 'cashflow',        -1250, '2026-11-07T12:00:00', '2026-11-12T08:00:00.000', 'verify-causes coffee again'),
  ('Verify Causes', 'cashflow',         3000, '2026-11-12T12:00:00', '2026-11-12T08:01:00.000', 'verify-causes parking'),
  ('Verify Causes', 'cashflow',        -4500, '2026-11-13T12:00:00', '2026-11-13T08:00:00.000', 'verify-causes groceries'),
  ('Verify Stale',  'balance_update',  50000, '2026-11-01T23:59:00', '2026-11-01T08:00:00.000', 'verify-stale opening'),
  ('Verify Stale',  'cashflow',        -2000, '2026-11-10T12:00:00', '2026-11-10T09:00:00.000', 'verify-stale last import');

INSERT INTO event (account_id, event_type, created_at)
SELECT a.id, s.event_type, s.created_at
FROM seed s JOIN account a ON a.name = s.account
WHERE NOT EXISTS (
  SELECT 1 FROM event e JOIN event_data d ON d.id = e.latest_data_id
  WHERE e.account_id = a.id AND d.note = s.note
);

INSERT INTO event_data (event_id, amount_minor, event_date, note)
SELECT e.id, s.amount_minor, s.event_date, s.note
FROM seed s
JOIN account a ON a.name = s.account
JOIN event e ON e.account_id = a.id AND e.created_at = s.created_at AND e.event_type = s.event_type
WHERE e.latest_data_id IS NULL;

INSERT INTO balance_check (account_id, checked_at, day, bank_balance_minor)
SELECT a.id, '2026-11-10T08:00:00.000', '2026-11-10', 98750
FROM account a
WHERE a.name = 'Verify Causes'
  AND NOT EXISTS (SELECT 1 FROM balance_check c WHERE c.account_id = a.id AND c.checked_at = '2026-11-10T08:00:00.000');
