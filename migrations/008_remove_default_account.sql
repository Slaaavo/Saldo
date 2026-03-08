-- Migration 008: remove seeded "Main Account" if it has no events.
-- Protects existing users: accounts with balance-update history are kept.
DELETE FROM account WHERE name = 'Main Account' AND id NOT IN (SELECT DISTINCT account_id FROM event);
