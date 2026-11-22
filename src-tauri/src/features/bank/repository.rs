use rusqlite::types::Type;
use rusqlite::{named_params, params, Connection, OptionalExtension};

use super::causes::{CountedRow, FlowType};
use crate::error::AppError;
use crate::features::transactions::repository::balance_sql::{
    balance_sql, counted_flow_sql, latest_balance_update_sql, AS_STORED,
};

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

pub struct NewestFlowDayParams {
    pub account_id: i64,
    pub on_or_before: String,
}

pub struct CheckedAccount {
    pub id: i64,
    pub name: String,
    pub currency_code: String,
    pub currency_minor_units: i64,
}

pub fn balance_seen_by_check(
    conn: &Connection,
    params: BalanceSeenByCheckParams,
) -> rusqlite::Result<i64> {
    let balance = balance_sql(
        ":account_id",
        |e, ed| format!("{e}.created_at <= :checked_at OR substr({ed}.event_date, 1, 10) < :day"),
        &AS_STORED,
    );
    conn.query_row(
        &format!("SELECT {balance}"),
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

pub fn checked_accounts(conn: &Connection) -> rusqlite::Result<Vec<CheckedAccount>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, c.code, c.minor_units
         FROM account a
         JOIN currency c ON c.id = a.currency_id
         WHERE a.account_type = 'account'
           AND EXISTS (SELECT 1 FROM balance_check bc WHERE bc.account_id = a.id)
         ORDER BY a.sort_order, a.id",
    )?;
    let accounts = stmt.query_map([], |row| {
        Ok(CheckedAccount {
            id: row.get(0)?,
            name: row.get(1)?,
            currency_code: row.get(2)?,
            currency_minor_units: row.get(3)?,
        })
    })?;
    accounts.collect()
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
    let latest = latest_balance_update_sql("?1", &|_, _| "1".to_owned(), "event_date");
    conn.query_row(&format!("SELECT {latest}"), params![account_id], |row| {
        row.get(0)
    })
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

pub fn counted_flow_rows(conn: &Connection, account_id: i64) -> rusqlite::Result<Vec<CountedRow>> {
    let counted = counted_flow_sql("?1", &|_, _| "1".to_owned(), &AS_STORED, "e", "ed");
    let mut stmt = conn.prepare(&format!(
        "SELECT e.id, e.event_type, e.split_group_id,
                e.linked_event_id IS NOT NULL,
                ed.original_amount_minor IS NOT NULL,
                ed.amount_minor, ed.event_date,
                CASE WHEN e.split_group_id IS NULL THEN ed.note ELSE COALESCE(sg.note, ed.note) END,
                counter_a.name,
                MAX(e.created_at, ed.created_at)
         FROM event e
         JOIN event_data ed ON ed.id = e.latest_data_id
         LEFT JOIN split_group sg ON sg.id = e.split_group_id
         LEFT JOIN account counter_a ON counter_a.id = ed.counterpart_account_id
         WHERE {counted}
         ORDER BY e.id"
    ))?;
    let rows = stmt.query_map(params![account_id], |row| {
        let event_type: String = row.get(1)?;
        Ok(CountedRow {
            event_id: row.get(0)?,
            flow: flow_type(&event_type)?,
            split_group_id: row.get(2)?,
            has_linked_event: row.get(3)?,
            has_original_amount: row.get(4)?,
            amount_minor: row.get(5)?,
            event_date: row.get(6)?,
            note: row.get(7)?,
            counterpart_name: row.get(8)?,
            changed_at: row.get(9)?,
        })
    })?;
    rows.collect()
}

fn flow_type(event_type: &str) -> rusqlite::Result<FlowType> {
    match event_type {
        "cashflow" => Ok(FlowType::Cashflow),
        "transfer" => Ok(FlowType::Transfer),
        "unexplained_difference" => Ok(FlowType::UnexplainedDifference),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            1,
            Type::Text,
            format!("{other} is not a balance flow type").into(),
        )),
    }
}

pub fn newest_flow_day(
    conn: &Connection,
    params: NewestFlowDayParams,
) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT MAX(substr(ed.event_date, 1, 10))
         FROM event e
         JOIN event_data ed ON ed.id = e.latest_data_id
         WHERE e.account_id = ?1
           AND e.deleted_at IS NULL
           AND e.event_type IN ('cashflow', 'transfer')
           AND substr(ed.event_date, 1, 10) <= ?2",
        params![params.account_id, params.on_or_before],
        |row| row.get(0),
    )
}

pub fn has_import_profile(conn: &Connection, account_id: i64) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM account WHERE id = ?1 AND preferred_profile_id IS NOT NULL)",
        params![account_id],
        |row| row.get(0),
    )
}
