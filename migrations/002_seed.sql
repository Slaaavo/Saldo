-- Migration 002: seed EUR currency and default account
BEGIN TRANSACTION;

INSERT INTO currency (code, name, minor_units)
VALUES ('EUR', 'Euro', 2);

INSERT INTO account (name, currency_id)
VALUES ('Main Account', (SELECT id FROM currency WHERE code = 'EUR'));

COMMIT;
