BEGIN;

CREATE TABLE tax_model (
    id                           INTEGER PRIMARY KEY AUTOINCREMENT,
    name                         TEXT    NOT NULL,
    calendar_year                INTEGER NOT NULL,
    person_id                    INTEGER NOT NULL REFERENCES person(id) ON DELETE RESTRICT,
    vat_status                   TEXT    NOT NULL DEFAULT 'none' CHECK (vat_status IN ('none', 'all_year', 'from_date')),
    vat_from_date                TEXT,
    reserve_fund_current_minor   INTEGER,
    reserve_fund_pct_bps         INTEGER,
    reserve_fund_max_minor       INTEGER,
    dividend_tax_rate_bps        INTEGER,
    created_at                   TEXT    NOT NULL,
    updated_at                   TEXT    NOT NULL
);

CREATE TABLE tax_model_bracket (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    tax_model_id      INTEGER NOT NULL REFERENCES tax_model(id) ON DELETE CASCADE,
    sort_order        INTEGER NOT NULL,
    lower_bound_minor INTEGER NOT NULL DEFAULT 0,
    rate_type         TEXT    NOT NULL CHECK (rate_type IN ('flat', 'progressive')),
    flat_rate_bps     INTEGER,
    tiers_json        TEXT
);

CREATE INDEX idx_tax_model_bracket_tax_model_id ON tax_model_bracket (tax_model_id);

COMMIT;
