-- Migration 033: Relax the UNIQUE constraint on taxable_cashflow_link.
--
-- Original constraint: UNIQUE(cashflow_event_id)
--   — allowed only one taxable event per cashflow event.
--
-- New constraint: UNIQUE(taxable_event_id, cashflow_event_id)
--   — allows the same cashflow event to be linked to multiple taxable events,
--     but prevents duplicate (taxable, cashflow) pairs.
--
-- SQLite does not support ALTER TABLE ... DROP CONSTRAINT, so the table is
-- recreated: new table created, data copied, original dropped, new renamed.

BEGIN TRANSACTION;

CREATE TABLE taxable_cashflow_link_new (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  taxable_event_id  INTEGER NOT NULL REFERENCES event(id),
  cashflow_event_id INTEGER NOT NULL REFERENCES event(id),
  created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now','localtime')),
  UNIQUE (taxable_event_id, cashflow_event_id)
);

INSERT INTO taxable_cashflow_link_new (id, taxable_event_id, cashflow_event_id, created_at)
SELECT id, taxable_event_id, cashflow_event_id, created_at
FROM taxable_cashflow_link;

DROP TABLE taxable_cashflow_link;

ALTER TABLE taxable_cashflow_link_new RENAME TO taxable_cashflow_link;

CREATE INDEX idx_taxable_cashflow_link_taxable ON taxable_cashflow_link (taxable_event_id);

COMMIT;
