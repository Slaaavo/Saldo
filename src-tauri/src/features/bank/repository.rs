use rusqlite::{named_params, params, Connection, OptionalExtension};

use crate::error::AppError;
use crate::features::transactions::repository::BALANCE_FLOW_TYPES;

pub struct BalanceSeenByCheckParams {
    pub account_id: i64,
    pub checked_at: String,
    pub day: String,
}

pub struct StoredCheck {
    pub id: i64,
    pub checked_at: String,
    pub day: String,
    pub bank_balance_minor: i64,
}

pub struct InsertCheckParams {
    pub account_id: i64,
    pub day: String,
    pub bank_balance_minor: i64,
}

pub struct InsertUnexplainedDifferenceParams {
    pub account_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
}

pub fn balance_seen_by_check(
    conn: &Connection,
    params: BalanceSeenByCheckParams,
) -> rusqlite::Result<i64> {
    conn.query_row(
        &format!(
            "WITH seen_by_check AS (
               SELECT e.event_type, e.created_at, ed.event_date, ed.amount_minor
               FROM event e
               JOIN event_data ed ON ed.id = e.latest_data_id
               WHERE e.account_id = :account_id
                 AND e.deleted_at IS NULL
                 AND (e.created_at <= :checked_at OR substr(ed.event_date, 1, 10) < :day)
             ),
             anchor AS (
               SELECT event_date, amount_minor
               FROM seen_by_check
               WHERE event_type = 'balance_update'
               ORDER BY event_date DESC, created_at DESC
               LIMIT 1
             )
             SELECT COALESCE((SELECT amount_minor FROM anchor), 0)
                  + COALESCE((SELECT SUM(amount_minor)
                              FROM seen_by_check
                              WHERE event_type IN ({BALANCE_FLOW_TYPES})
                                AND event_date > COALESCE((SELECT event_date FROM anchor), '')),
                             0)"
        ),
        named_params! {
            ":account_id": params.account_id,
            ":checked_at": params.checked_at,
            ":day": params.day,
        },
        |row| row.get(0),
    )
}

pub fn clock_now(conn: &Connection) -> rusqlite::Result<String> {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%f', 'now')", [], |row| {
        row.get(0)
    })
}

pub fn newest_check(conn: &Connection, account_id: i64) -> rusqlite::Result<Option<StoredCheck>> {
    conn.query_row(
        "SELECT id, checked_at, day, bank_balance_minor
         FROM balance_check
         WHERE account_id = ?1
         ORDER BY checked_at DESC, id DESC
         LIMIT 1",
        params![account_id],
        |row| {
            Ok(StoredCheck {
                id: row.get(0)?,
                checked_at: row.get(1)?,
                day: row.get(2)?,
                bank_balance_minor: row.get(3)?,
            })
        },
    )
    .optional()
}

pub fn insert_check(conn: &Connection, params: InsertCheckParams) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO balance_check (account_id, day, bank_balance_minor)
         VALUES (?1, ?2, ?3)",
        params![params.account_id, params.day, params.bank_balance_minor],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_check(conn: &Connection, check_id: i64) -> Result<(), AppError> {
    let deleted = conn.execute("DELETE FROM balance_check WHERE id = ?1", params![check_id])?;
    if deleted == 0 {
        return Err(AppError {
            code: "NOT_FOUND".into(),
            message: "This check no longer exists.".into(),
        });
    }
    Ok(())
}

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
