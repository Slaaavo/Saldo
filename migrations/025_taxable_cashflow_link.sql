CREATE TABLE taxable_cashflow_link (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  taxable_event_id  INTEGER NOT NULL REFERENCES event(id),
  cashflow_event_id INTEGER NOT NULL REFERENCES event(id),
  created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now','localtime')),
  UNIQUE (cashflow_event_id)
);

CREATE INDEX idx_taxable_cashflow_link_taxable ON taxable_cashflow_link (taxable_event_id);
