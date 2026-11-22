CREATE TABLE bank_entry (
  event_id     INTEGER PRIMARY KEY REFERENCES event (id) ON DELETE CASCADE,
  day          TEXT    NOT NULL CHECK (date(day) IS day),
  amount_minor INTEGER NOT NULL,
  text         TEXT
);
