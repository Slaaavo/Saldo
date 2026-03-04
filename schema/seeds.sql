-- Example seeds and usage for our-finances (MVP)
-- Date: 2026-03-02
-- NOTE: This file is an example seed set for development/testing only. Do NOT treat as an automated migration.

BEGIN TRANSACTION;

-- Ensure EUR currency exists
INSERT INTO currency(code, name, minor_units, created_at)
SELECT 'EUR', 'Euro', 2, strftime('%Y-%m-%dT%H:%M:%f','now')
WHERE NOT EXISTS(SELECT 1 FROM currency WHERE code = 'EUR');

-- Ensure default account exists
INSERT INTO account(name, currency_id, created_at)
SELECT 'Main Account', (SELECT id FROM currency WHERE code = 'EUR'), strftime('%Y-%m-%dT%H:%M:%f','now')
WHERE NOT EXISTS(SELECT 1 FROM account WHERE name = 'Main Account');

-- Example: create a balance_update event for the default account (50.00 EUR)
-- Flow: 1) insert event, 2) insert event_data; trigger will update event.latest_data_id

-- 1) insert event
INSERT INTO event(account_id, event_type, created_at)
VALUES (
  (SELECT id FROM account WHERE name = 'Main Account'),
  'balance_update',
  strftime('%Y-%m-%dT%H:%M:%f','now')
);

-- capture last inserted event id
-- (SQLite: use last_insert_rowid() if running interactively; in app, read returned id)

-- 2) insert event_data (amount_minor is cents; 50.00 EUR -> 5000)
INSERT INTO event_data(event_id, amount_minor, event_date, note, created_at)
VALUES (
  (SELECT id FROM event WHERE rowid = last_insert_rowid()),
  5000,
  strftime('%Y-%m-%dT%H:%M:%f','now'),
  'Initial seed balance',
  strftime('%Y-%m-%dT%H:%M:%f','now')
);

COMMIT;

-- Example snapshot query (replace :selected_datetime with an ISO datetime string)
-- Example usage in sqlite3 CLI:
-- $ sqlite3 db.sqlite "SELECT * FROM account_snapshot('2026-03-02T23:59:59');"

-- Correlated-subquery approach:
-- SELECT
--   a.id AS account_id,
--   a.name,
--   COALESCE((
--     SELECT ed.amount_minor
--     FROM event e
--     JOIN event_data ed ON ed.event_id = e.id
--     WHERE e.account_id = a.id
--       AND e.deleted_at IS NULL
--       AND ed.event_date <= :selected_datetime
--     ORDER BY ed.event_date DESC, ed.created_at DESC
--     LIMIT 1
--   ), 0) AS balance_minor
-- FROM account a
-- ORDER BY a.name;

-- End of seeds file
