CREATE TABLE bank_entry_new (
  event_id             INTEGER PRIMARY KEY REFERENCES event (id) ON DELETE CASCADE,
  day                  TEXT    NOT NULL CHECK (date(day) IS day),
  amount_minor         INTEGER NOT NULL,
  text                 TEXT,
  card                 TEXT,
  hold_seen_to         TEXT    CHECK (date(hold_seen_to) IS hold_seen_to),
  posted_at            TEXT    CHECK (strftime('%Y-%m-%dT%H:%M:%f', posted_at) IS posted_at),
  posting_change_minor INTEGER,
  held_day             TEXT    CHECK (date(held_day) IS held_day),
  held_amount_minor    INTEGER,
  cancelled_at         TEXT    CHECK (strftime('%Y-%m-%dT%H:%M:%f', cancelled_at) IS cancelled_at),
  cancelled_by_day     TEXT    CHECK (date(cancelled_by_day) IS cancelled_by_day),
  CHECK ((posted_at IS NULL) + (posting_change_minor IS NULL) + (held_day IS NULL)
         + (held_amount_minor IS NULL) IN (0, 4)),
  CHECK ((cancelled_at IS NULL) = (cancelled_by_day IS NULL)),
  CHECK ((hold_seen_to IS NOT NULL) + (posted_at IS NOT NULL) + (cancelled_at IS NOT NULL) <= 1)
);

INSERT INTO bank_entry_new (event_id, day, amount_minor, text)
SELECT event_id, day, amount_minor, text FROM bank_entry;

DROP TABLE bank_entry;

ALTER TABLE bank_entry_new RENAME TO bank_entry;
