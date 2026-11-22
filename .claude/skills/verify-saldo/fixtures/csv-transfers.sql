-- Setup for features/csv-import.md "Already in Saldo and transfers". Running it again adds nothing.
-- Verify TB (SK9699990000001000000001), Verify CSOB (SK6999990000001000000002) and
-- Verify Card (SK4299990000001000000003): own EUR accounts, each with a balance update of 1000.00
-- on 1 Nov. The three accounts sort first, so their cards show without scrolling the card row.
-- The CSV files that go with it: csv-transfers-card.csv, csv-transfers-tb.csv, csv-transfers-csob.csv.

INSERT INTO account (name, currency_id, account_type, iban, sort_order)
SELECT name, (SELECT id FROM currency WHERE code = 'EUR'), 'account', iban, sort_order
FROM (SELECT 'Verify TB' AS name, 'SK9699990000001000000001' AS iban, -3 AS sort_order
      UNION ALL SELECT 'Verify CSOB', 'SK6999990000001000000002', -2
      UNION ALL SELECT 'Verify Card', 'SK4299990000001000000003', -1) AS wanted
WHERE NOT EXISTS (SELECT 1 FROM account a WHERE a.name = wanted.name);

INSERT INTO event (account_id, event_type)
SELECT a.id, 'balance_update'
FROM account a
WHERE a.name IN ('Verify TB', 'Verify CSOB', 'Verify Card')
  AND NOT EXISTS (SELECT 1 FROM event e WHERE e.account_id = a.id AND e.event_type = 'balance_update');

INSERT INTO event_data (event_id, amount_minor, event_date, note)
SELECT e.id, 100000, '2026-11-01T23:59:00', 'verify-transfers opening'
FROM event e JOIN account a ON a.id = e.account_id
WHERE a.name IN ('Verify TB', 'Verify CSOB', 'Verify Card')
  AND e.event_type = 'balance_update'
  AND NOT EXISTS (SELECT 1 FROM event_data d WHERE d.event_id = e.id);
