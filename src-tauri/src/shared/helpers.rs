use crate::error::AppError;
use chrono::NaiveDate;
use rusqlite::Connection;

/// Execute `f` inside a SAVEPOINT. Rolls back on error, releases on success.
/// Works with &Connection (no &mut needed).
pub fn with_savepoint<T, F>(conn: &Connection, f: F) -> rusqlite::Result<T>
where
    F: FnOnce() -> rusqlite::Result<T>,
{
    conn.execute_batch("SAVEPOINT repo_sp")?;
    match f() {
        Ok(val) => {
            conn.execute_batch("RELEASE SAVEPOINT repo_sp")?;
            Ok(val)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT repo_sp");
            let _ = conn.execute_batch("RELEASE SAVEPOINT repo_sp");
            Err(e)
        }
    }
}

/// Same as `with_savepoint` but for closures that return `Result<T, AppError>`.
pub fn with_savepoint_app<T, F>(conn: &Connection, f: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError>,
{
    conn.execute_batch("SAVEPOINT repo_sp")
        .map_err(AppError::from)?;
    match f() {
        Ok(val) => {
            conn.execute_batch("RELEASE SAVEPOINT repo_sp")
                .map_err(AppError::from)?;
            Ok(val)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT repo_sp");
            let _ = conn.execute_batch("RELEASE SAVEPOINT repo_sp");
            Err(e)
        }
    }
}

pub fn local_now() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn validate_event_date(date_str: &str) -> Result<(), AppError> {
    if date_str.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "event_date is required".into(),
        });
    }
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|_| AppError {
        code: "VALIDATION".into(),
        message: "event_date must be a valid date in YYYY-MM-DD format".into(),
    })?;
    Ok(())
}

/// Convert `balance_minor` (in source currency) to destination currency minor units.
///
/// `rate` is stored as: 1 destination unit = `rate_mantissa × 10^rate_exponent` source units.
/// Formula (corrected for minor-unit mismatch):
///   `exp_adj = -rate_exponent + dest_minor_units - source_minor_units`
///   if exp_adj >= 0: `converted = (balance × 10^exp_adj + mantissa/2) / mantissa`
///   if exp_adj < 0: `converted = (balance + mantissa×10^|exp_adj|/2) / (mantissa×10^|exp_adj|)`
/// All intermediates use i128 to prevent overflow.
pub fn convert_balance(
    balance_minor: i64,
    rate_mantissa: i64,
    rate_exponent: i64,
    source_minor_units: i64,
    dest_minor_units: i64,
) -> i64 {
    let exp_adj = -rate_exponent + dest_minor_units - source_minor_units;
    let balance = balance_minor as i128;
    let mantissa = rate_mantissa as i128;

    if exp_adj >= 0 {
        let factor = 10_i128.pow(exp_adj as u32);
        (((balance * factor) + mantissa / 2) / mantissa) as i64
    } else {
        let factor = 10_i128.pow((-exp_adj) as u32);
        let denominator = mantissa * factor;
        ((balance + denominator / 2) / denominator) as i64
    }
}

#[cfg(test)]
mod convert_balance_tests {
    use super::convert_balance;

    // Test 1: 1:1 identity — mantissa=1, exponent=0, same minor_units
    #[test]
    fn identity_same_minor_units() {
        assert_eq!(convert_balance(100_000, 1, 0, 2, 2), 100_000);
    }

    // Test 2: EUR→USD standard (same minor_units=2)
    // Rate: 1 EUR = 1.0842 USD (mantissa=10842, exponent=-4)
    // Balance: 108420 USD cents (1084.20 USD) → 100000 EUR cents (1000.00 EUR)
    #[test]
    fn eur_to_usd_same_minor_units() {
        assert_eq!(convert_balance(108420, 10842, -4, 2, 2), 100_000);
    }

    // Test 3: EUR→JPY (source has minor_units=0, dest minor_units=2)
    // Rate: 1 EUR = 157.23 JPY (mantissa=15723, exponent=-2)
    // Balance: 15723 JPY (no minor units) → 10000 EUR cents (100.00 EUR)
    #[test]
    fn eur_to_jpy_different_minor_units() {
        // exp_adj = 2 + 2 - 0 = 4; converted = 15723 * 10000 / 15723 = 10000
        assert_eq!(convert_balance(15723, 15723, -2, 0, 2), 10_000);
    }

    // Test 4: EUR→BTC (source has minor_units=8)
    // Rate: 1 EUR = 0.0000161 BTC (mantissa=161, exponent=-7)
    // Balance: 50_000_000 satoshi (0.5 BTC) → ~3_105_590 EUR cents (~31 055.90 EUR)
    #[test]
    fn eur_to_btc_different_minor_units() {
        // exp_adj = 7 + 2 - 8 = 1; converted = 50_000_000 * 10 / 161 ≈ 3_105_590
        assert_eq!(convert_balance(50_000_000, 161, -7, 8, 2), 3_105_590);
    }

    // Test 5: zero balance always yields zero
    #[test]
    fn zero_balance() {
        assert_eq!(convert_balance(0, 10842, -4, 2, 2), 0);
    }

    // Test 6: negative balance (exact case to avoid rounding ambiguity)
    // mantissa=1 means 1:1 rate → negative passthrough
    #[test]
    fn negative_balance_identity_rate() {
        assert_eq!(convert_balance(-50_000, 1, 0, 2, 2), -50_000);
    }

    // Test 7: large amount — verify i128 intermediates don't overflow
    // Balance: 1_000_000_000_000 USD cents (10 billion USD), rate 1 EUR = 1.0842 USD
    #[test]
    fn large_amount_no_overflow() {
        let result = convert_balance(1_000_000_000_000, 10842, -4, 2, 2);
        // 10^12 * 10^4 / 10842 ≈ 922_528_128_581
        assert!(result > 900_000_000_000);
        assert!(result < 1_000_000_000_000);
    }

    // Test 8: exp_adj < 0 (hyperinflation — 1 EUR = 36 500 VES, minor_units=2 both)
    // mantissa=365, exponent=2 → rate = 36500
    // Balance: 3_650_000 VES minor (36 500.00 VES) → 100 EUR cents (1.00 EUR)
    #[test]
    fn exp_adj_negative_hyperinflation() {
        // exp_adj = -2 + 2 - 2 = -2; denom = 365*100 = 36500
        // (3_650_000 + 18250) / 36500 = 100
        assert_eq!(convert_balance(3_650_000, 365, 2, 2, 2), 100);
    }

    // Test 9: rounding — half-up for positive remainder
    // 3 units to convert when rate is 2:1 → 1 (floor) or 2 (round)?
    // 3 USD minor * factor / 2 → 3/2 = 1.5 → rounds to 2 (half-up)
    #[test]
    fn rounding_half_up() {
        // 3 USD minor (minor_units=2), rate 1 EUR = 2 USD (mantissa=2, exponent=0)
        // exp_adj = 0; (3 * 1 + 1) / 2 = 4/2 = 2
        assert_eq!(convert_balance(3, 2, 0, 2, 2), 2);
    }
}
