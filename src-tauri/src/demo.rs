use chrono::{Datelike, Local, NaiveDate};
use rusqlite::{params, Connection};

use crate::error::AppError;

/// Returns the last calendar day of the month that ended `months_ago` months before today.
fn last_day_of_month_ago(months_ago: u32) -> NaiveDate {
    let today = Local::now().date_naive();
    let mut year = today.year();
    let mut month = today.month() as i32 - months_ago as i32;
    while month <= 0 {
        month += 12;
        year -= 1;
    }
    let first_of_next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, (month + 1) as u32, 1).unwrap()
    };
    first_of_next.pred_opt().unwrap()
}

/// Seed `conn` with demo accounts, events, bucket allocations, and FX rates.
/// Carries over `oxr_app_id` and `theme` from the user's persistent settings so the
/// demo experience matches the user's existing API key and colour theme.
pub fn seed_demo_data(
    conn: &Connection,
    oxr_app_id: Option<String>,
    theme: Option<String>,
) -> Result<(), AppError> {
    // A. Look up currency IDs from the already-migrated schema.
    let eur_id: i64 = conn.query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |row| {
        row.get(0)
    })?;
    let btc_id: i64 = conn.query_row("SELECT id FROM currency WHERE code = 'BTC'", [], |row| {
        row.get(0)
    })?;

    // B. Compute dynamic event dates (last day of each of the 3 preceding months).
    let m1_naive = last_day_of_month_ago(3);
    let m2_naive = last_day_of_month_ago(2);
    let m3_naive = last_day_of_month_ago(1);

    // Events use end-of-day datetime strings (YYYY-MM-DDT23:59:59).
    let m1_event = format!("{}T23:59:59", m1_naive);
    let m2_event = format!("{}T23:59:59", m2_naive);
    let m3_event = format!("{}T23:59:59", m3_naive);

    // Allocation / FX-rate rows use plain date strings (YYYY-MM-DD).
    let m1_date = m1_naive.format("%Y-%m-%d").to_string();
    let m2_date = m2_naive.format("%Y-%m-%d").to_string();
    let m3_date = m3_naive.format("%Y-%m-%d").to_string();

    conn.execute_batch("BEGIN")?;

    match seed_impl(
        conn, eur_id, btc_id, &m1_event, &m2_event, &m3_event, &m1_date, &m2_date, &m3_date,
        oxr_app_id, theme,
    ) {
        Ok(()) => conn.execute_batch("COMMIT").map_err(AppError::from),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn seed_impl(
    conn: &Connection,
    eur_id: i64,
    btc_id: i64,
    m1_event: &str,
    m2_event: &str,
    m3_event: &str,
    m1_date: &str,
    m2_date: &str,
    m3_date: &str,
    oxr_app_id: Option<String>,
    theme: Option<String>,
) -> Result<(), AppError> {
    // C. App settings: set consolidation currency; carry over optional settings.
    conn.execute(
        "INSERT INTO app_setting (key, value) VALUES ('consolidation_currency_code', 'EUR')
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [],
    )?;
    if let Some(ref api_id) = oxr_app_id {
        conn.execute(
            "INSERT INTO app_setting (key, value) VALUES ('oxr_app_id', ?1)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![api_id],
        )?;
    }
    if let Some(ref t) = theme {
        conn.execute(
            "INSERT INTO app_setting (key, value) VALUES ('theme', ?1)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![t],
        )?;
    }

    // D. Insert 6 accounts (3 regular + 3 bucket).
    conn.execute(
        "INSERT INTO account (name, currency_id, account_type, sort_order)
         VALUES ('Checking Account', ?1, 'account', 0)",
        params![eur_id],
    )?;
    let checking_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO account (name, currency_id, account_type, sort_order)
         VALUES ('Credit Card', ?1, 'account', 1)",
        params![eur_id],
    )?;
    let credit_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO account (name, currency_id, account_type, sort_order)
         VALUES ('BTC Wallet', ?1, 'account', 2)",
        params![btc_id],
    )?;
    let btc_wallet_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO account (name, currency_id, account_type, sort_order)
         VALUES ('Emergency Fund', ?1, 'bucket', 0)",
        params![eur_id],
    )?;
    let emergency_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO account (name, currency_id, account_type, sort_order)
         VALUES ('Retirement Fund', ?1, 'bucket', 1)",
        params![eur_id],
    )?;
    let retirement_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO account (name, currency_id, account_type, sort_order)
         VALUES ('Holiday Savings', ?1, 'bucket', 2)",
        params![eur_id],
    )?;
    let holiday_id = conn.last_insert_rowid();

    // E. Insert 9 account balance update events (3 accounts × 3 months).
    // Checking Account
    insert_balance_update(conn, checking_id, 425_000, m1_event)?;
    insert_balance_update(conn, checking_id, 458_000, m2_event)?;
    insert_balance_update(conn, checking_id, 482_000, m3_event)?;
    // Credit Card
    insert_balance_update(conn, credit_id, -32_000, m1_event)?;
    insert_balance_update(conn, credit_id, -18_500, m2_event)?;
    insert_balance_update(conn, credit_id, -41_000, m3_event)?;
    // BTC Wallet (amounts in satoshis)
    insert_balance_update(conn, btc_wallet_id, 8_500_000, m1_event)?;
    insert_balance_update(conn, btc_wallet_id, 8_500_000, m2_event)?;
    insert_balance_update(conn, btc_wallet_id, 9_200_000, m3_event)?;

    // F. Insert 6 bucket balance update events (2 buckets × 3 months).
    // Emergency Fund (EUR cents)
    insert_balance_update(conn, emergency_id, 273_000, m1_event)?;
    insert_balance_update(conn, emergency_id, 309_500, m2_event)?;
    insert_balance_update(conn, emergency_id, 286_000, m3_event)?;
    // Holiday Savings (M1 baseline = 0, then grows)
    insert_balance_update(conn, holiday_id, 0, m1_event)?;
    insert_balance_update(conn, holiday_id, 20_000, m2_event)?;
    insert_balance_update(conn, holiday_id, 45_000, m3_event)?;

    // G. Insert 3 bucket allocations: Retirement Fund ← BTC Wallet (satoshis).
    conn.execute(
        "INSERT INTO bucket_allocation (bucket_id, source_account_id, amount_minor, effective_date)
         VALUES (?1, ?2, ?3, ?4)",
        params![retirement_id, btc_wallet_id, 8_500_000_i64, m1_date],
    )?;
    conn.execute(
        "INSERT INTO bucket_allocation (bucket_id, source_account_id, amount_minor, effective_date)
         VALUES (?1, ?2, ?3, ?4)",
        params![retirement_id, btc_wallet_id, 8_500_000_i64, m2_date],
    )?;
    conn.execute(
        "INSERT INTO bucket_allocation (bucket_id, source_account_id, amount_minor, effective_date)
         VALUES (?1, ?2, ?3, ?4)",
        params![retirement_id, btc_wallet_id, 9_200_000_i64, m3_date],
    )?;

    // H. Insert 3 FX rates (EUR → BTC, is_manual = 1).
    // The rate represents whole BTC per 1 whole EUR: stored as mantissa × 10^exponent.
    // Approximate BTC prices: M1 ≈ €82,450, M2 ≈ €88,200, M3 ≈ €91,750 per BTC.
    // Correct representation: 1/82450 ≈ 121309×10⁻¹⁰; etc.
    // (The plan's raw values of mantissa=82450, exponent=0 would yield ~€0 on conversion
    //  because the price direction was inverted; the corrected values are used here.)
    conn.execute(
        "INSERT INTO fx_rate
           (date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent, is_manual)
         VALUES (?1, ?2, ?3, 121309, -10, 1)
         ON CONFLICT (date, from_currency_id, to_currency_id) DO UPDATE SET
           rate_mantissa = excluded.rate_mantissa,
           rate_exponent = excluded.rate_exponent,
           is_manual     = excluded.is_manual",
        params![m1_date, eur_id, btc_id],
    )?;
    conn.execute(
        "INSERT INTO fx_rate
           (date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent, is_manual)
         VALUES (?1, ?2, ?3, 113379, -10, 1)
         ON CONFLICT (date, from_currency_id, to_currency_id) DO UPDATE SET
           rate_mantissa = excluded.rate_mantissa,
           rate_exponent = excluded.rate_exponent,
           is_manual     = excluded.is_manual",
        params![m2_date, eur_id, btc_id],
    )?;
    conn.execute(
        "INSERT INTO fx_rate
           (date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent, is_manual)
         VALUES (?1, ?2, ?3, 108981, -10, 1)
         ON CONFLICT (date, from_currency_id, to_currency_id) DO UPDATE SET
           rate_mantissa = excluded.rate_mantissa,
           rate_exponent = excluded.rate_exponent,
           is_manual     = excluded.is_manual",
        params![m3_date, eur_id, btc_id],
    )?;

    Ok(())
}

fn insert_balance_update(
    conn: &Connection,
    account_id: i64,
    amount_minor: i64,
    event_date: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO event (account_id, event_type) VALUES (?1, 'balance_update')",
        params![account_id],
    )?;
    let event_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO event_data (event_id, amount_minor, event_date) VALUES (?1, ?2, ?3)",
        params![event_id, amount_minor, event_date],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;

    #[test]
    fn seed_demo_data_populates_expected_rows() {
        let conn = initialize_in_memory().expect("DB init failed");
        seed_demo_data(&conn, None, None).expect("seed failed");

        // 6 accounts total
        let account_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM account", [], |r| r.get(0))
            .unwrap();
        assert_eq!(account_count, 6);

        // 3 regular accounts
        let regular_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM account WHERE account_type = 'account'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(regular_count, 3);

        // 3 bucket accounts
        let bucket_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM account WHERE account_type = 'bucket'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(bucket_count, 3);

        // 15 events: 9 account (3×3) + 6 bucket (2×3, including holiday M1=0)
        let event_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM event", [], |r| r.get(0))
            .unwrap();
        assert_eq!(event_count, 15);

        // 3 bucket allocations (Retirement Fund ← BTC Wallet, one per month)
        let alloc_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM bucket_allocation", [], |r| r.get(0))
            .unwrap();
        assert_eq!(alloc_count, 3);

        // 3 FX rates (EUR → BTC, one per month)
        let rate_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM fx_rate", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rate_count, 3);

        // Consolidation currency is EUR
        let consol: String = conn
            .query_row(
                "SELECT value FROM app_setting WHERE key = 'consolidation_currency_code'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(consol, "EUR");
    }

    #[test]
    fn seed_demo_data_carries_over_settings() {
        let conn = initialize_in_memory().expect("DB init failed");
        seed_demo_data(
            &conn,
            Some("test-api-key".to_string()),
            Some("dark".to_string()),
        )
        .expect("seed failed");

        let api_key: String = conn
            .query_row(
                "SELECT value FROM app_setting WHERE key = 'oxr_app_id'",
                [],
                |r| r.get(0),
            )
            .expect("oxr_app_id should be set");
        assert_eq!(api_key, "test-api-key");

        let theme: String = conn
            .query_row(
                "SELECT value FROM app_setting WHERE key = 'theme'",
                [],
                |r| r.get(0),
            )
            .expect("theme should be set");
        assert_eq!(theme, "dark");
    }
}
