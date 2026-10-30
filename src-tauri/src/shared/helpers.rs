use crate::error::AppError;
use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::{Connection, OptionalExtension};

/// Execute `f` inside a SAVEPOINT. Rolls back on error, releases on success.
/// Works with &Connection (no &mut needed).
pub fn with_savepoint<T, F>(conn: &Connection, f: F) -> rusqlite::Result<T>
where
    F: FnOnce() -> rusqlite::Result<T>,
{
    run_in_savepoint(conn, f)
}

/// Same as `with_savepoint` but for closures that return `Result<T, AppError>`.
pub fn with_savepoint_app<T, F>(conn: &Connection, f: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError>,
{
    run_in_savepoint(conn, f)
}

/// A deferred transaction that reads and then writes fails with SQLITE_BUSY_SNAPSHOT when another
/// connection committed in between, and `busy_timeout` does not retry that. Taking the write lock
/// first avoids it.
const BEGIN_WRITE: &str = "BEGIN IMMEDIATE";

fn run_in_savepoint<T, E, F>(conn: &Connection, f: F) -> Result<T, E>
where
    E: From<rusqlite::Error>,
    F: FnOnce() -> Result<T, E>,
{
    if conn.is_autocommit() {
        return run_in_transaction(conn, f);
    }
    conn.execute_batch("SAVEPOINT repo_sp")?;
    let result = f().and_then(|val| {
        conn.execute_batch("RELEASE SAVEPOINT repo_sp")?;
        Ok(val)
    });
    if result.is_err() {
        let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT repo_sp");
        let _ = conn.execute_batch("RELEASE SAVEPOINT repo_sp");
    }
    result
}

/// COMMIT fails on a deferred foreign key violation and leaves the transaction open, so a failed
/// commit is rolled back too.
fn run_in_transaction<T, E, F>(conn: &Connection, f: F) -> Result<T, E>
where
    E: From<rusqlite::Error>,
    F: FnOnce() -> Result<T, E>,
{
    conn.execute_batch(BEGIN_WRITE)?;
    let result = f().and_then(|val| {
        conn.execute_batch("COMMIT")?;
        Ok(val)
    });
    if result.is_err() && !conn.is_autocommit() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    result
}

/// Runs `f` in one deferred transaction, so all of its queries read the same snapshot.
pub fn with_read_txn<T, E, F>(conn: &Connection, f: F) -> Result<T, E>
where
    E: From<rusqlite::Error>,
    F: FnOnce() -> Result<T, E>,
{
    conn.execute_batch("BEGIN")?;
    let result = f();
    let ended = if conn.is_autocommit() {
        Ok(())
    } else {
        conn.execute_batch("COMMIT")
    };
    let value = result?;
    ended?;
    Ok(value)
}

pub struct WriteTxn<'c> {
    conn: &'c Connection,
    open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    Commit,
    Rollback,
}

impl<'c> WriteTxn<'c> {
    pub fn begin(conn: &'c Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(BEGIN_WRITE)?;
        Ok(Self { conn, open: true })
    }

    pub fn conn(&self) -> &'c Connection {
        self.conn
    }

    /// SQLite rolls a whole transaction back on its own after some errors, such as a full disk.
    /// A savepoint taken after that opens and commits a transaction of its own, so a caller that
    /// carries on past a failed step checks this first.
    pub fn ensure_open(&self) -> Result<(), AppError> {
        if self.conn.is_autocommit() {
            return Err(AppError {
                code: "INTERNAL".into(),
                message: "SQLite ended the transaction after an error (such as a full disk or \
                          an I/O error). Nothing was written."
                    .into(),
            });
        }
        Ok(())
    }

    /// `event.latest_data_id` is a deferred key, so a broken one would fail the COMMIT with no
    /// row named. The check names it first, and runs before a rollback too, so a dry run fails
    /// wherever its write would.
    pub fn finish(mut self, finish: Finish) -> Result<(), AppError> {
        self.ensure_open()?;
        let violation: Option<i64> = self
            .conn
            .query_row("PRAGMA foreign_key_check(event)", [], |row| row.get(1))
            .optional()?;
        if let Some(event_id) = violation {
            return Err(AppError {
                code: "FOREIGN_KEY_VIOLATION".into(),
                message: format!("Event {event_id} references a row that does not exist"),
            });
        }
        self.conn.execute_batch(match finish {
            Finish::Commit => "COMMIT",
            Finish::Rollback => "ROLLBACK",
        })?;
        self.open = false;
        Ok(())
    }
}

impl Drop for WriteTxn<'_> {
    fn drop(&mut self) {
        if self.open && !self.conn.is_autocommit() {
            let _ = self.conn.execute_batch("ROLLBACK");
        }
    }
}

pub fn local_now() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Returns the last calendar day of the given month, handling leap years.
pub fn last_day_of_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => panic!("last_day_of_month: invalid month {}", month),
    }
}

/// Returns `"YYYY-MM-DDT23:59:59"` where DD is the last calendar day of the given month.
pub fn format_end_of_month(year: i32, month: u32) -> String {
    let last_day = last_day_of_month(year, month);
    format!("{:04}-{:02}-{:02}T23:59:59", year, month, last_day)
}

/// Integer division with rounding to nearest, halves away from zero (symmetric for negatives).
pub fn div_round(numerator: i64, denominator: i64) -> i64 {
    (numerator + numerator.signum() * (denominator.abs() / 2)) / denominator
}

/// `div_round` for the i128 intermediates of `convert_balance`; `denominator` must be positive.
fn div_round_i128(numerator: i128, denominator: i128) -> i128 {
    (numerator + numerator.signum() * (denominator / 2)) / denominator
}

/// True only for a zero-padded `YYYY-MM-DD` date. chrono's `%m` and `%d` also accept one-digit
/// values, but dates are sliced by byte offset and compared as strings elsewhere.
pub fn is_iso_date(date_str: &str) -> bool {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .is_ok_and(|date| date.format("%Y-%m-%d").to_string() == date_str)
}

/// True only for a zero-padded `YYYY-MM-DDTHH:MM:SS` datetime (see `is_iso_date`).
pub fn is_iso_datetime(datetime_str: &str) -> bool {
    NaiveDateTime::parse_from_str(datetime_str, "%Y-%m-%dT%H:%M:%S")
        .is_ok_and(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string() == datetime_str)
}

pub fn validate_event_date(date_str: &str) -> Result<(), AppError> {
    if date_str.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "event_date is required".into(),
        });
    }
    // Accept both YYYY-MM-DD and YYYY-MM-DDTHH:MM:SS
    if is_iso_date(date_str) || is_iso_datetime(date_str) {
        return Ok(());
    }
    Err(AppError {
        code: "VALIDATION".into(),
        message: "event_date must be a valid date in YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS format"
            .into(),
    })
}

/// Validates a date-only `YYYY-MM-DD` value; `field` names it in the error message.
pub fn validate_iso_date(date_str: &str, field: &str) -> Result<(), AppError> {
    if is_iso_date(date_str) {
        return Ok(());
    }
    Err(AppError {
        code: "VALIDATION".into(),
        message: format!("{field} must be a valid date in YYYY-MM-DD format"),
    })
}

/// Convert `balance_minor` (in source currency) to destination currency minor units.
///
/// When `is_direct = false` (standard FX rates):
///   `rate` is stored as: 1 destination unit = `rate_mantissa × 10^rate_exponent` source units.
///   `exp_adj = -rate_exponent + dest_minor_units - source_minor_units`
///   if exp_adj >= 0: `converted = round(balance × 10^exp_adj / mantissa)`
///   if exp_adj < 0: `converted = round(balance / (mantissa × 10^|exp_adj|))`
///
/// When `is_direct = true` (unit asset rates — price per unit stored directly):
///   `rate` is stored as: 1 source unit = `rate_mantissa × 10^rate_exponent` destination units.
///   `exp_adj = rate_exponent + dest_minor_units - source_minor_units`
///   if exp_adj >= 0: `converted = balance × mantissa × 10^exp_adj`
///   if exp_adj < 0: `converted = round(balance × mantissa / 10^|exp_adj|)`
///
/// `round` is half away from zero, so a negative balance converts to the negation of the
/// positive one. All intermediates use i128 to prevent overflow.
pub fn convert_balance(
    balance_minor: i64,
    rate_mantissa: i64,
    rate_exponent: i64,
    source_minor_units: i64,
    dest_minor_units: i64,
    is_direct: bool,
) -> i64 {
    let balance = balance_minor as i128;
    let mantissa = rate_mantissa as i128;

    if is_direct {
        let exp_adj = rate_exponent + dest_minor_units - source_minor_units;
        if exp_adj >= 0 {
            let factor = 10_i128.pow(exp_adj as u32);
            (balance * mantissa * factor) as i64
        } else {
            let factor = 10_i128.pow((-exp_adj) as u32);
            div_round_i128(balance * mantissa, factor) as i64
        }
    } else {
        let exp_adj = -rate_exponent + dest_minor_units - source_minor_units;
        if exp_adj >= 0 {
            let factor = 10_i128.pow(exp_adj as u32);
            div_round_i128(balance * factor, mantissa) as i64
        } else {
            let factor = 10_i128.pow((-exp_adj) as u32);
            div_round_i128(balance, mantissa * factor) as i64
        }
    }
}

#[cfg(test)]
mod savepoint_tests {
    use super::{with_read_txn, with_savepoint, with_savepoint_app, Finish, WriteTxn};
    use crate::error::AppError;
    use rusqlite::Connection;

    fn deferred_fk_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE parent (id INTEGER PRIMARY KEY);
             CREATE TABLE child (
                 id INTEGER PRIMARY KEY,
                 parent_id INTEGER NOT NULL REFERENCES parent (id) DEFERRABLE INITIALLY DEFERRED
             );",
        )
        .unwrap();
        conn
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    fn insert_orphan(conn: &Connection) -> rusqlite::Result<usize> {
        conn.execute("INSERT INTO child (id, parent_id) VALUES (1, 99)", [])
    }

    fn insert_parent(conn: &Connection) -> rusqlite::Result<usize> {
        conn.execute("INSERT INTO parent (id) VALUES (1)", [])
    }

    #[test]
    fn with_savepoint_rolls_back_when_the_commit_fails() {
        let conn = deferred_fk_conn();

        let result = with_savepoint(&conn, || insert_orphan(&conn));

        assert!(result.is_err());
        assert!(conn.is_autocommit());
        with_savepoint(&conn, || insert_parent(&conn)).unwrap();
        assert!(conn.is_autocommit());
        assert_eq!(count(&conn, "parent"), 1);
        assert_eq!(count(&conn, "child"), 0);
    }

    #[test]
    fn with_savepoint_app_rolls_back_when_the_commit_fails() {
        let conn = deferred_fk_conn();

        let result = with_savepoint_app(&conn, || insert_orphan(&conn).map_err(AppError::from));

        assert!(result.is_err());
        assert!(conn.is_autocommit());
        with_savepoint_app(&conn, || insert_parent(&conn).map_err(AppError::from)).unwrap();
        assert!(conn.is_autocommit());
        assert_eq!(count(&conn, "parent"), 1);
        assert_eq!(count(&conn, "child"), 0);
    }

    #[test]
    fn a_failing_nested_scope_rolls_back_alone() {
        let conn = deferred_fk_conn();

        with_savepoint(&conn, || {
            insert_parent(&conn)?;
            let inner = with_savepoint(&conn, || {
                conn.execute("INSERT INTO parent (id) VALUES (2)", [])?;
                conn.execute("INSERT INTO parent (id) VALUES (1)", [])
            });
            assert!(inner.is_err());
            Ok(())
        })
        .unwrap();

        assert!(conn.is_autocommit());
        assert_eq!(count(&conn, "parent"), 1);
    }

    fn two_connections() -> (tempfile::TempDir, Connection, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.db");
        let open = || {
            let conn = Connection::open(&path).unwrap();
            conn.busy_timeout(std::time::Duration::from_millis(50))
                .unwrap();
            conn.query_row("PRAGMA journal_mode = WAL", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
            conn
        };
        let first = open();
        first
            .execute_batch("CREATE TABLE parent (id INTEGER PRIMARY KEY);")
            .unwrap();
        let second = open();
        (dir, first, second)
    }

    #[test]
    fn the_outermost_scope_takes_the_write_lock_before_it_reads() {
        let (_dir, app, other) = two_connections();

        with_savepoint(&app, || {
            let other_write = other.execute("INSERT INTO parent (id) VALUES (7)", []);
            assert!(
                other_write.is_err(),
                "the other process must wait for the lock"
            );
            insert_parent(&app)
        })
        .unwrap();

        other
            .execute("INSERT INTO parent (id) VALUES (7)", [])
            .unwrap();
        assert_eq!(count(&app, "parent"), 2);
    }

    fn saldo_conn() -> Connection {
        let conn = crate::db::initialize_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE parent (id INTEGER PRIMARY KEY);")
            .unwrap();
        conn
    }

    #[test]
    fn write_txn_rolls_back_when_dropped() {
        let conn = saldo_conn();

        {
            let _txn = WriteTxn::begin(&conn).unwrap();
            with_savepoint(&conn, || insert_parent(&conn)).unwrap();
        }

        assert!(conn.is_autocommit());
        assert_eq!(count(&conn, "parent"), 0);
    }

    #[test]
    fn write_txn_rolls_back_when_a_panic_unwinds() {
        let conn = saldo_conn();

        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _txn = WriteTxn::begin(&conn).unwrap();
            insert_parent(&conn).unwrap();
            panic!("handler failed");
        }));

        assert!(unwound.is_err());
        assert!(conn.is_autocommit());
        assert_eq!(count(&conn, "parent"), 0);
    }

    #[test]
    fn write_txn_commits() {
        let conn = saldo_conn();

        let txn = WriteTxn::begin(&conn).unwrap();
        insert_parent(&conn).unwrap();
        txn.finish(Finish::Commit).unwrap();

        assert!(conn.is_autocommit());
        assert_eq!(count(&conn, "parent"), 1);
    }

    #[test]
    fn write_txn_rolls_back_when_told_to() {
        let conn = saldo_conn();

        let txn = WriteTxn::begin(&conn).unwrap();
        insert_parent(&conn).unwrap();
        txn.finish(Finish::Rollback).unwrap();

        assert!(conn.is_autocommit());
        assert_eq!(count(&conn, "parent"), 0);
    }

    #[test]
    fn write_txn_names_a_broken_event_reference_on_commit_and_on_rollback() {
        for finish in [Finish::Commit, Finish::Rollback] {
            let conn = crate::db::initialize_in_memory().unwrap();
            let account_id: i64 = conn
                .query_row("SELECT MIN(id) FROM account", [], |row| row.get(0))
                .unwrap();

            let txn = WriteTxn::begin(&conn).unwrap();
            conn.execute(
                "INSERT INTO event (account_id, event_type, latest_data_id) VALUES (?1, 'expense', 999)",
                [account_id],
            )
            .unwrap();
            let err = txn.finish(finish).unwrap_err();

            assert_eq!(err.code, "FOREIGN_KEY_VIOLATION", "{finish:?}");
            assert!(conn.is_autocommit());
            assert_eq!(count(&conn, "event"), 0);
        }
    }

    #[test]
    fn write_txn_reports_a_transaction_sqlite_ended_and_never_commits_after_it() {
        let conn = saldo_conn();
        let txn = WriteTxn::begin(&conn).unwrap();
        insert_parent(&conn).unwrap();
        conn.execute_batch("ROLLBACK").unwrap();

        assert_eq!(txn.ensure_open().unwrap_err().code, "INTERNAL");
        assert_eq!(txn.finish(Finish::Commit).unwrap_err().code, "INTERNAL");
        assert_eq!(count(&conn, "parent"), 0);
    }

    #[test]
    fn with_read_txn_reads_one_snapshot() {
        let (_dir, reader, writer) = two_connections();

        let (before, during) = with_read_txn(&reader, || {
            let before = count(&reader, "parent");
            insert_parent(&writer)?;
            Ok::<_, rusqlite::Error>((before, count(&reader, "parent")))
        })
        .unwrap();

        assert_eq!((before, during), (0, 0));
        assert!(reader.is_autocommit());
        assert_eq!(count(&reader, "parent"), 1);
    }
}

#[cfg(test)]
mod rounding_tests {
    use super::{convert_balance, div_round};

    #[test]
    fn div_round_rounds_half_away_from_zero() {
        assert_eq!(div_round(7, 2), 4);
        assert_eq!(div_round(-7, 2), -4);
        assert_eq!(div_round(5, 3), 2);
        assert_eq!(div_round(-5, 3), -2);
        assert_eq!(div_round(1, 3), 0);
        assert_eq!(div_round(-1, 3), 0);
        assert_eq!(div_round(6, 3), 2);
        assert_eq!(div_round(-6, 3), -2);
        assert_eq!(div_round(0, 5), 0);
    }

    #[test]
    fn div_round_is_symmetric_for_negative_denominators() {
        assert_eq!(div_round(5, -2), -3);
        assert_eq!(div_round(-5, -2), 3);
    }

    // exact value 100010000 / 10842 = 9224.33
    #[test]
    fn convert_balance_rounds_negative_balances_like_positive_ones() {
        assert_eq!(convert_balance(10001, 10842, -4, 2, 2, false), 9224);
        assert_eq!(convert_balance(-10001, 10842, -4, 2, 2, false), -9224);
    }

    #[test]
    fn convert_balance_rounds_half_away_from_zero_on_exact_halves() {
        // 3 / 2 = 1.5
        assert_eq!(convert_balance(3, 2, 0, 2, 2, false), 2);
        assert_eq!(convert_balance(-3, 2, 0, 2, 2, false), -2);
    }

    #[test]
    fn convert_balance_is_symmetric_in_every_branch() {
        // (mantissa, exponent, source_minor_units, dest_minor_units, is_direct)
        let cases = [
            (10842, -4, 2, 2, false),                // standard, exp_adj >= 0
            (365, 2, 2, 2, false),                   // standard, exp_adj < 0
            (15723, -2, 0, 2, false),                // standard, different minor units
            (425, -1, 0, 2, true),                   // direct, exp_adj >= 0
            (7, -3, 2, 2, true),                     // direct, exp_adj < 0
            (123_456_789_012_345, -19, 2, 2, false), // 15 significant digits
            (123_456_789_012_345, -13, 2, 2, true),
        ];
        for (mantissa, exponent, source, dest, is_direct) in cases {
            for balance in [1, 5, 99, 10_001, 1_234_567, 99_999_999] {
                assert_eq!(
                    convert_balance(-balance, mantissa, exponent, source, dest, is_direct),
                    -convert_balance(balance, mantissa, exponent, source, dest, is_direct),
                    "balance {balance}, rate {mantissa}e{exponent}, direct {is_direct}"
                );
            }
        }
    }

    #[test]
    fn convert_balance_negative_with_high_precision_rate_does_not_overflow() {
        // exp_adj = 19, so the intermediate is balance * 10^19.
        let converted = convert_balance(-10_000_000_000_000, 123_456_789_012_345, -19, 2, 2, false);
        let expected = convert_balance(10_000_000_000_000, 123_456_789_012_345, -19, 2, 2, false);
        assert_eq!(converted, -expected);
        assert!(converted < 0);
    }
}

#[cfg(test)]
mod date_tests {
    use super::{is_iso_date, validate_event_date};

    #[test]
    fn validate_event_date_accepts_padded_dates_and_datetimes() {
        for date in ["2027-01-09", "2027-01-09T07:05:03", "2028-02-29T23:59:59"] {
            assert!(validate_event_date(date).is_ok(), "{date:?}");
        }
    }

    #[test]
    fn validate_event_date_rejects_unpadded_or_malformed_values() {
        for date in [
            "",
            "2027-1-1",
            "2027-01-1",
            "2027-1-01",
            "2027-1-1T00:00:00",
            "2027-01-01T0:00:00",
            "2027-01-01T00:0:00",
            "2027-01-01T00:00:0",
            "2027-01-01T00:00",
            "2027-01-01 00:00:00",
            "2027-01-01T00:00:00.000",
            "2027-02-30",
        ] {
            let err = validate_event_date(date).unwrap_err();
            assert_eq!(err.code, "VALIDATION", "{date:?}");
        }
    }

    #[test]
    fn is_iso_date_accepts_a_zero_padded_date() {
        assert!(is_iso_date("2027-01-09"));
        assert!(is_iso_date("2028-02-29"));
    }

    #[test]
    fn is_iso_date_rejects_other_shapes() {
        for date in [
            "",
            "2027-1-1",
            "2027-01-1",
            "2027-1-01",
            "27-01-01",
            "+999-01-01",
            "2027-01-01T00:00:00",
            "2027-02-30",
            "2027/01/01",
        ] {
            assert!(!is_iso_date(date), "{date:?}");
        }
    }
}

#[cfg(test)]
mod convert_balance_tests {
    use super::convert_balance;

    // Test 1: 1:1 identity — mantissa=1, exponent=0, same minor_units
    #[test]
    fn identity_same_minor_units() {
        assert_eq!(convert_balance(100_000, 1, 0, 2, 2, false), 100_000);
    }

    // Test 2: EUR→USD standard (same minor_units=2)
    // Rate: 1 EUR = 1.0842 USD (mantissa=10842, exponent=-4)
    // Balance: 108420 USD cents (1084.20 USD) → 100000 EUR cents (1000.00 EUR)
    #[test]
    fn eur_to_usd_same_minor_units() {
        assert_eq!(convert_balance(108420, 10842, -4, 2, 2, false), 100_000);
    }

    // Test 3: EUR→JPY (source has minor_units=0, dest minor_units=2)
    // Rate: 1 EUR = 157.23 JPY (mantissa=15723, exponent=-2)
    // Balance: 15723 JPY (no minor units) → 10000 EUR cents (100.00 EUR)
    #[test]
    fn eur_to_jpy_different_minor_units() {
        // exp_adj = 2 + 2 - 0 = 4; converted = 15723 * 10000 / 15723 = 10000
        assert_eq!(convert_balance(15723, 15723, -2, 0, 2, false), 10_000);
    }

    // Test 4: EUR→BTC (source has minor_units=8)
    // Rate: 1 EUR = 0.0000161 BTC (mantissa=161, exponent=-7)
    // Balance: 50_000_000 satoshi (0.5 BTC) → ~3_105_590 EUR cents (~31 055.90 EUR)
    #[test]
    fn eur_to_btc_different_minor_units() {
        // exp_adj = 7 + 2 - 8 = 1; converted = 50_000_000 * 10 / 161 ≈ 3_105_590
        assert_eq!(convert_balance(50_000_000, 161, -7, 8, 2, false), 3_105_590);
    }

    // Test 5: zero balance always yields zero
    #[test]
    fn zero_balance() {
        assert_eq!(convert_balance(0, 10842, -4, 2, 2, false), 0);
    }

    // Test 6: negative balance (exact case to avoid rounding ambiguity)
    // mantissa=1 means 1:1 rate → negative passthrough
    #[test]
    fn negative_balance_identity_rate() {
        assert_eq!(convert_balance(-50_000, 1, 0, 2, 2, false), -50_000);
    }

    // Test 7: large amount — verify i128 intermediates don't overflow
    // Balance: 1_000_000_000_000 USD cents (10 billion USD), rate 1 EUR = 1.0842 USD
    #[test]
    fn large_amount_no_overflow() {
        let result = convert_balance(1_000_000_000_000, 10842, -4, 2, 2, false);
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
        assert_eq!(convert_balance(3_650_000, 365, 2, 2, 2, false), 100);
    }

    // Test 9: rounding — half-up for positive remainder
    // 3 units to convert when rate is 2:1 → 1 (floor) or 2 (round)?
    // 3 USD minor * factor / 2 → 3/2 = 1.5 → rounds to 2 (half-up)
    #[test]
    fn rounding_half_up() {
        // 3 USD minor (minor_units=2), rate 1 EUR = 2 USD (mantissa=2, exponent=0)
        // exp_adj = 0; (3 * 1 + 1) / 2 = 4/2 = 2
        assert_eq!(convert_balance(3, 2, 0, 2, 2, false), 2);
    }

    // is_direct=true tests (multiplicative formula)

    // Test 10: 3 units × EUR 42.50/unit = 12750 EUR minor (127.50 EUR)
    // rate: mantissa=425, exponent=-1 (42.5), source_minor_units=0, dest_minor_units=2
    // exp_adj = -1 + 2 - 0 = 1; converted = (3 * 425 + 5) / 10 = 1280/10 = 128? No...
    // Wait: balance=3 (units, minor_units=0), source_minor_units=0, dest_minor_units=2
    // exp_adj = -1 + 2 - 0 = 1; exp_adj >= 0 would need exp_adj = rate_exponent + dest - source
    // exp_adj = -1 + 2 - 0 = 1; factor = 10; result = 3 * 425 * 10 = 12750
    #[test]
    fn is_direct_units_to_eur() {
        // 3 units, price 42.5 EUR/unit (mantissa=425, exponent=-1)
        // source_minor_units=0 (whole units), dest_minor_units=2 (EUR cents)
        // exp_adj = -1 + 2 - 0 = 1; result = 3 * 425 * 10 = 12750 EUR minor
        assert_eq!(convert_balance(3, 425, -1, 0, 2, true), 12_750);
    }

    // Test 11: is_direct rounding — exp_adj < 0 case
    // 3 units, price 1000 EUR/unit (mantissa=1, exponent=3)
    // source_minor_units=0, dest_minor_units=2
    // exp_adj = 3 + 2 - 0 = 5; result = 3 * 1 * 100000 = 300000 EUR minor (3000.00 EUR)
    #[test]
    fn is_direct_large_price_exp_adj_positive() {
        assert_eq!(convert_balance(3, 1, 3, 0, 2, true), 300_000);
    }

    // Test 12: is_direct with exp_adj < 0 (fractional result, half-up rounding)
    // 1 unit, price 0.1 EUR/unit (mantissa=1, exponent=-1)
    // source_minor_units=2, dest_minor_units=2
    // exp_adj = -1 + 2 - 2 = -1; factor = 10
    // result = (100 * 1 + 5) / 10 = 105 / 10 = 10 (rounds down from 10.5)
    // Actually 100 * 1 = 100; (100 + 5) / 10 = 10 EUR minor (0.10 EUR) — correct!
    #[test]
    fn is_direct_exp_adj_negative_rounding() {
        // 100 minor units, price 0.1 EUR/unit (1.00 unit = 0.10 EUR)
        // mantissa=1, exponent=-1; source_minor_units=2, dest_minor_units=2
        // exp_adj = -1 + 2 - 2 = -1; factor=10; result = (100*1 + 5)/10 = 10
        assert_eq!(convert_balance(100, 1, -1, 2, 2, true), 10);
    }
}
