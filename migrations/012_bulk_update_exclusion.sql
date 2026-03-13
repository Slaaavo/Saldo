CREATE TABLE bulk_update_exclusion (
    account_id INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    PRIMARY KEY (account_id)
);
