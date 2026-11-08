INSERT INTO person (name, person_type, created_at)
SELECT 'Verify Consulting', 'legal', strftime('%Y-%m-%dT%H:%M:%S', 'now')
WHERE NOT EXISTS (SELECT 1 FROM person WHERE name = 'Verify Consulting');

INSERT INTO account (name, currency_id, account_type, sort_order, person_id)
SELECT 'Verify Income', (SELECT id FROM currency WHERE code = 'EUR'), 'account', 90,
       (SELECT id FROM person WHERE name = 'Verify Consulting')
WHERE NOT EXISTS (SELECT 1 FROM account WHERE name = 'Verify Income');

CREATE TEMP TABLE inflow (amount_minor INTEGER, event_date TEXT, note TEXT);
INSERT INTO inflow VALUES
  (350550, '2026-07-09T12:00:00', 'verify-saldo income July'),
  (662601, '2026-08-14T12:00:00', 'verify-saldo income August'),
  (674163, '2026-09-10T12:00:00', 'verify-saldo income September'),
  (720411, '2026-10-12T12:00:00', 'verify-saldo income October');

INSERT INTO event (account_id, event_type, created_at)
SELECT (SELECT id FROM account WHERE name = 'Verify Income'), 'cashflow', i.event_date
FROM inflow i
WHERE NOT EXISTS (
  SELECT 1 FROM event e JOIN event_data d ON d.id = e.latest_data_id
  WHERE e.account_id = (SELECT id FROM account WHERE name = 'Verify Income') AND d.note = i.note
);

INSERT INTO event_data (event_id, amount_minor, event_date, note)
SELECT e.id, i.amount_minor, i.event_date, i.note
FROM event e JOIN inflow i ON i.event_date = e.created_at
WHERE e.account_id = (SELECT id FROM account WHERE name = 'Verify Income') AND e.latest_data_id IS NULL;
