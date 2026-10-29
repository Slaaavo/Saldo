ALTER TABLE tax_model ADD COLUMN actuals_through_month INTEGER NOT NULL DEFAULT 12 CHECK (actuals_through_month BETWEEN 0 AND 12);
ALTER TABLE tax_model ADD COLUMN payroll_monthly_minor INTEGER NOT NULL DEFAULT 0 CHECK (payroll_monthly_minor >= 0);

CREATE TABLE tax_model_typed_value (
    tax_model_id  INTEGER NOT NULL REFERENCES tax_model(id) ON DELETE CASCADE,
    line          TEXT    NOT NULL,
    so_far_minor  INTEGER CHECK (so_far_minor IS NULL OR so_far_minor >= 0),
    rest_minor    INTEGER CHECK (rest_minor IS NULL OR rest_minor >= 0),
    PRIMARY KEY (tax_model_id, line),
    CHECK (so_far_minor IS NOT NULL OR rest_minor IS NOT NULL)
);

CREATE TABLE tax_model_earlier_profit (
    tax_model_id           INTEGER NOT NULL REFERENCES tax_model(id) ON DELETE CASCADE,
    profit_year            INTEGER NOT NULL,
    amount_minor           INTEGER NOT NULL CHECK (amount_minor >= 0),
    dividend_tax_rate_bps  INTEGER NOT NULL CHECK (dividend_tax_rate_bps BETWEEN 0 AND 10000),
    PRIMARY KEY (tax_model_id, profit_year)
);

CREATE TABLE tax_model_minimum_tax_band (
    tax_model_id       INTEGER NOT NULL REFERENCES tax_model(id) ON DELETE CASCADE,
    lower_bound_minor  INTEGER NOT NULL CHECK (lower_bound_minor >= 0),
    amount_minor       INTEGER NOT NULL CHECK (amount_minor >= 0),
    PRIMARY KEY (tax_model_id, lower_bound_minor)
);
