BEGIN TRANSACTION;

ALTER TABLE fx_rate ADD COLUMN is_direct INTEGER NOT NULL DEFAULT 0;

-- For custom-currency (unit asset) rows: flip stored 1/price back to price.
-- Currently stored value = 1/price, computed via 12-digit integer division.
-- Recovery: price ≈ 1 / stored_value, computed via SQLite float at 8dp.
-- The string-cast trick (CAST('1e' || exponent AS REAL)) handles negative
-- exponents correctly in SQLite and avoids the missing POWER() function.
UPDATE fx_rate
SET
  rate_mantissa = CAST(ROUND(
    1.0e8 / (
      CAST(rate_mantissa AS REAL) *
      CAST('1e' || CAST(rate_exponent AS TEXT) AS REAL)
    )
  ) AS INTEGER),
  rate_exponent = -8,
  is_direct = 1
WHERE to_currency_id IN (SELECT id FROM currency WHERE is_custom = 1)
  AND rate_mantissa != 0;

COMMIT;
