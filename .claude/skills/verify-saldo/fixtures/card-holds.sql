-- Setup for features/csv-import.md "Card holds". Running it again adds nothing.
-- Verify Holds: an own EUR account with a balance update of 500.00 on 10 Nov, sorted first.
-- Profile "Verify TB card" maps the Tatra banka card export and is the account's preferred profile:
-- sign from Typ transakcie, pending when Stav transakcie starts "Autorizovan", declined when it
-- starts "Zamietnut", card from Číslo karty.
-- The CSV files that go with it: card-holds-1.csv, then card-holds-2.csv.

INSERT INTO account (name, currency_id, account_type, sort_order)
SELECT 'Verify Holds', (SELECT id FROM currency WHERE code = 'EUR'), 'account', -4
WHERE NOT EXISTS (SELECT 1 FROM account WHERE name = 'Verify Holds');

INSERT INTO event (account_id, event_type)
SELECT a.id, 'balance_update'
FROM account a
WHERE a.name = 'Verify Holds'
  AND NOT EXISTS (SELECT 1 FROM event e WHERE e.account_id = a.id AND e.event_type = 'balance_update');

INSERT INTO event_data (event_id, amount_minor, event_date, note)
SELECT e.id, 50000, '2026-11-10T23:59:00', 'verify-holds opening'
FROM event e JOIN account a ON a.id = e.account_id
WHERE a.name = 'Verify Holds'
  AND e.event_type = 'balance_update'
  AND NOT EXISTS (SELECT 1 FROM event_data d WHERE d.event_id = e.id);

INSERT INTO import_profile (name, column_mapping_json, created_at, updated_at)
SELECT 'Verify TB card',
       '{"date":"Dátum transakcie","amount":"Suma EUR","partner":null,"note":"Názov obchodníka","currency":null,"fxRate":null,"card":"Číslo karty"}',
       '2026-11-22T12:00:00', '2026-11-22T12:00:00'
WHERE NOT EXISTS (SELECT 1 FROM import_profile WHERE name = 'Verify TB card');

INSERT INTO import_profile_rule (profile_id, rule_type, sort_order, params_json)
SELECT p.id, wanted.rule_type, wanted.sort_order, wanted.params_json
FROM import_profile p
CROSS JOIN (SELECT 'sign_from_column' AS rule_type, 0 AS sort_order,
                   '{"typeColumn":"Typ transakcie","negativeType":"Debet"}' AS params_json
            UNION ALL SELECT 'pending_from_column', 1,
                   '{"column":"Stav transakcie","pattern":"^Autorizovan"}'
            UNION ALL SELECT 'declined_from_column', 2,
                   '{"column":"Stav transakcie","pattern":"^Zamietnut"}') AS wanted
WHERE p.name = 'Verify TB card'
  AND NOT EXISTS (SELECT 1 FROM import_profile_rule r WHERE r.profile_id = p.id);

UPDATE account
SET preferred_profile_id = (SELECT id FROM import_profile WHERE name = 'Verify TB card')
WHERE name = 'Verify Holds';
