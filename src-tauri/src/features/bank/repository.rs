use rusqlite::{params, Connection, OptionalExtension};

use crate::features::transactions::repository::BALANCE_FLOW_TYPES;

/// How many of an account's checks a verification reads, newest first.
const NEWEST_CHECKS: i64 = 20;

pub struct SaldoSideParams {
    pub account_id: i64,
    /// UTC, in `event.created_at`'s format.
    pub checked_at: String,
    /// The user's local day of the check, `YYYY-MM-DD`.
    pub checked_on: String,
}

pub struct StoredCheck {
    pub id: i64,
    pub checked_at: String,
    pub checked_on: String,
    pub bank_balance_minor: i64,
}

pub struct InsertCheckParams {
    pub account_id: i64,
    pub checked_on: String,
    pub bank_balance_minor: i64,
    /// None takes the database clock at the insert. Tests pass a moment to place the check
    /// before or after an event's `created_at`.
    pub checked_at: Option<String>,
}

pub struct InsertUnexplainedDifferenceParams {
    pub account_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
}

/// The account's live balance as a check saw it, recomputed from today's data. The one rule for
/// what a check saw: an event counts with its current version when it existed at the check, or
/// when it is dated before the check's day (Saldo was missing it). A row created later and dated
/// on or after that day happened after the check. A deleted row counts as gone. The balance is
/// the latest visible balance update plus the visible flows dated after it, with no date cutoff,
/// as in live mode.
pub fn saldo_side(conn: &Connection, params: SaldoSideParams) -> rusqlite::Result<i64> {
    conn.query_row(
        &format!(
            "WITH visible AS (
               SELECT e.event_type, e.created_at, ed.event_date, ed.amount_minor
               FROM event e
               JOIN event_data ed ON ed.id = e.latest_data_id
               WHERE e.account_id = ?1
                 AND e.deleted_at IS NULL
                 AND (e.created_at <= ?2 OR substr(ed.event_date, 1, 10) < ?3)
             ),
             anchor AS (
               SELECT event_date, amount_minor
               FROM visible
               WHERE event_type = 'balance_update'
               ORDER BY event_date DESC, created_at DESC
               LIMIT 1
             )
             SELECT COALESCE((SELECT amount_minor FROM anchor), 0)
                  + COALESCE((SELECT SUM(amount_minor)
                              FROM visible
                              WHERE event_type IN ({BALANCE_FLOW_TYPES})
                                AND event_date > COALESCE((SELECT event_date FROM anchor), '')),
                             0)"
        ),
        params![params.account_id, params.checked_at, params.checked_on],
        |row| row.get(0),
    )
}

/// The database clock in `event.created_at`'s format, so a check taken now sees every row
/// written so far.
pub fn clock_now(conn: &Connection) -> rusqlite::Result<String> {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%f', 'now')", [], |row| {
        row.get(0)
    })
}

pub fn newest_checks(conn: &Connection, account_id: i64) -> rusqlite::Result<Vec<StoredCheck>> {
    let mut stmt = conn.prepare(
        "SELECT id, checked_at, checked_on, bank_balance_minor
         FROM balance_check
         WHERE account_id = ?1
         ORDER BY checked_at DESC, id DESC
         LIMIT ?2",
    )?;
    let checks = stmt
        .query_map(params![account_id, NEWEST_CHECKS], |row| {
            Ok(StoredCheck {
                id: row.get(0)?,
                checked_at: row.get(1)?,
                checked_on: row.get(2)?,
                bank_balance_minor: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(checks)
}

pub fn insert_check(conn: &Connection, params: InsertCheckParams) -> rusqlite::Result<i64> {
    match params.checked_at {
        Some(checked_at) => conn.execute(
            "INSERT INTO balance_check (account_id, checked_at, checked_on, bank_balance_minor)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                params.account_id,
                checked_at,
                params.checked_on,
                params.bank_balance_minor
            ],
        )?,
        None => conn.execute(
            "INSERT INTO balance_check (account_id, checked_on, bank_balance_minor)
             VALUES (?1, ?2, ?3)",
            params![
                params.account_id,
                params.checked_on,
                params.bank_balance_minor
            ],
        )?,
    };
    Ok(conn.last_insert_rowid())
}

/// Returns whether the check existed.
pub fn delete_check(conn: &Connection, check_id: i64) -> rusqlite::Result<bool> {
    let deleted = conn.execute("DELETE FROM balance_check WHERE id = ?1", params![check_id])?;
    Ok(deleted > 0)
}

/// The date of the balance update that sets the account's live balance, if any.
pub fn latest_balance_update_date(
    conn: &Connection,
    account_id: i64,
) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT ed.event_date
         FROM event e
         JOIN event_data ed ON ed.id = e.latest_data_id
         WHERE e.account_id = ?1
           AND e.deleted_at IS NULL
           AND e.event_type = 'balance_update'
         ORDER BY ed.event_date DESC, e.created_at DESC
         LIMIT 1",
        params![account_id],
        |row| row.get(0),
    )
    .optional()
}

pub fn insert_unexplained_difference(
    conn: &Connection,
    params: InsertUnexplainedDifferenceParams,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO event (account_id, event_type) VALUES (?1, 'unexplained_difference')",
        params![params.account_id],
    )?;
    let event_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO event_data (event_id, amount_minor, event_date, note)
         VALUES (?1, ?2, ?3, NULL)",
        params![event_id, params.amount_minor, params.event_date],
    )?;
    Ok(event_id)
}
