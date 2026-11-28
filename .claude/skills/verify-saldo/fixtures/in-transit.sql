-- Setup for features/bank-checks.md "Transfer on its way" and dashboard-snapshot.md "In transit".
-- Running it again adds nothing.
-- Verify Sender (SK2199990000001000000004) and Verify Receiver (SK9899990000001000000005): own EUR
-- accounts, each with a balance update of 1000.00 on 1 Nov, sorted first. The CSV files are written
-- at run time with yesterday's and today's dates; the recipe gives their contents.

INSERT INTO account (name, currency_id, account_type, iban, sort_order)
SELECT name, (SELECT id FROM currency WHERE code = 'EUR'), 'account', iban, sort_order
FROM (SELECT 'Verify Sender' AS name, 'SK2199990000001000000004' AS iban, -5 AS sort_order
      UNION ALL SELECT 'Verify Receiver', 'SK9899990000001000000005', -4) AS wanted
WHERE NOT EXISTS (SELECT 1 FROM account a WHERE a.name = wanted.name);

INSERT INTO event (account_id, event_type)
SELECT a.id, 'balance_update'
FROM account a
WHERE a.name IN ('Verify Sender', 'Verify Receiver')
  AND NOT EXISTS (SELECT 1 FROM event e WHERE e.account_id = a.id AND e.event_type = 'balance_update');

INSERT INTO event_data (event_id, amount_minor, event_date, note)
SELECT e.id, 100000, '2026-11-01T23:59:00', 'verify-in-transit opening'
FROM event e JOIN account a ON a.id = e.account_id
WHERE a.name IN ('Verify Sender', 'Verify Receiver')
  AND e.event_type = 'balance_update'
  AND NOT EXISTS (SELECT 1 FROM event_data d WHERE d.event_id = e.id);
