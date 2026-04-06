BEGIN;

CREATE TABLE person (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL,
    person_type TEXT    NOT NULL CHECK (person_type IN ('physical', 'legal')),
    is_default  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL
);

INSERT INTO person (name, person_type, is_default, created_at)
VALUES ('Personal', 'physical', 1, strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime'));

ALTER TABLE account ADD COLUMN person_id INTEGER REFERENCES person(id) ON DELETE RESTRICT;

UPDATE account
SET person_id = (SELECT id FROM person WHERE is_default = 1)
WHERE account_type != 'partner';

COMMIT;
