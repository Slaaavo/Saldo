use chrono::NaiveDate;
use rusqlite::types::Type;
use rusqlite::{params, Connection, Row};

use super::matcher::{BankFacts, CounterpartCashflow, LedgerItem, Recorded, TransferLeg};

pub struct CounterpartCashflowsParams {
    pub account_id: i64,
    pub importing_account_id: i64,
}

pub struct InsertBankEntryParams {
    pub event_id: i64,
    pub day: NaiveDate,
    pub amount_minor: i64,
    pub text: Option<String>,
}

pub fn ledger_items(conn: &Connection, account_id: i64) -> rusqlite::Result<Vec<LedgerItem>> {
    let mut singles = conn.prepare(
        "SELECT e.id AS bank_entry_event_id, e.latest_data_id AS latest_data_id,
                ed.event_date AS event_date, ed.amount_minor AS amount_minor, ed.note AS note,
                be.day AS bank_day, be.amount_minor AS bank_amount_minor,
                other.id AS other_account_id, other.name AS other_account_name,
                e.created_at AS created_at
         FROM event e
         JOIN event_data ed ON ed.id = e.latest_data_id
         LEFT JOIN bank_entry be ON be.event_id = e.id
         LEFT JOIN event linked ON linked.id = e.linked_event_id
         LEFT JOIN account other
                ON e.event_type = 'transfer'
               AND other.id = COALESCE(linked.account_id, ed.counterpart_account_id)
         WHERE e.account_id = ?1
           AND e.deleted_at IS NULL
           AND e.split_group_id IS NULL
           AND e.event_type IN ('cashflow', 'transfer')",
    )?;
    let mut items: Vec<LedgerItem> = singles
        .query_map(params![account_id], |row| {
            let other_account_id: Option<i64> = row.get("other_account_id")?;
            let transfer_leg = match other_account_id {
                Some(other_account_id) => Some(TransferLeg {
                    other_account_id,
                    other_account_name: row.get("other_account_name")?,
                    created_at: row.get("created_at")?,
                }),
                None => None,
            };
            Ok(LedgerItem {
                recorded: recorded(row)?,
                transfer_leg,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut groups = conn.prepare(
        "WITH legs AS (
             SELECT e.split_group_id AS split_group_id, e.id AS id, ed.amount_minor AS amount_minor
             FROM event e
             JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.account_id = ?1
               AND e.deleted_at IS NULL
               AND e.split_group_id IS NOT NULL
               AND e.event_type IN ('cashflow', 'transfer')
         ),
         groups AS (
             SELECT split_group_id, MIN(id) AS lead_id, SUM(amount_minor) AS amount_minor
             FROM legs
             GROUP BY split_group_id
         )
         SELECT lead.id AS bank_entry_event_id, lead.latest_data_id AS latest_data_id,
                ed.event_date AS event_date, groups.amount_minor AS amount_minor, sg.note AS note,
                be.day AS bank_day, be.amount_minor AS bank_amount_minor
         FROM groups
         JOIN event lead ON lead.id = groups.lead_id
         JOIN event_data ed ON ed.id = lead.latest_data_id
         JOIN split_group sg ON sg.id = groups.split_group_id
         LEFT JOIN bank_entry be ON be.event_id = lead.id",
    )?;
    let split_groups = groups.query_map(params![account_id], |row| {
        Ok(LedgerItem {
            recorded: recorded(row)?,
            transfer_leg: None,
        })
    })?;
    for group in split_groups {
        items.push(group?);
    }
    Ok(items)
}

pub fn counterpart_cashflows(
    conn: &Connection,
    params: CounterpartCashflowsParams,
) -> rusqlite::Result<Vec<CounterpartCashflow>> {
    let CounterpartCashflowsParams {
        account_id,
        importing_account_id,
    } = params;
    let mut stmt = conn.prepare(
        "SELECT e.id AS bank_entry_event_id, e.latest_data_id AS latest_data_id,
                ed.event_date AS event_date, ed.amount_minor AS amount_minor, ed.note AS note,
                be.day AS bank_day, be.amount_minor AS bank_amount_minor, a.name AS account_name
         FROM event e
         JOIN event_data ed ON ed.id = e.latest_data_id
         JOIN account a ON a.id = e.account_id
         LEFT JOIN bank_entry be ON be.event_id = e.id
         WHERE e.account_id = ?1
           AND e.deleted_at IS NULL
           AND e.event_type = 'cashflow'
           AND e.split_group_id IS NULL
           AND e.linked_event_id IS NULL
           AND e.linked_asset_id IS NULL
           AND e.is_system_generated = 0
           AND NOT EXISTS (
               SELECT 1 FROM taxable_cashflow_link tcl WHERE tcl.cashflow_event_id = e.id
           )
           AND (ed.counterpart_account_id IS NULL OR ed.counterpart_account_id = ?2)",
    )?;
    let rows = stmt.query_map(params![account_id, importing_account_id], |row| {
        Ok(CounterpartCashflow {
            recorded: recorded(row)?,
            account_id,
            account_name: row.get("account_name")?,
        })
    })?;
    rows.collect()
}

pub fn balance_update_days(conn: &Connection, account_id: i64) -> rusqlite::Result<Vec<NaiveDate>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT substr(ed.event_date, 1, 10) AS day
         FROM event e
         JOIN event_data ed ON ed.id = e.latest_data_id
         WHERE e.account_id = ?1
           AND e.deleted_at IS NULL
           AND e.event_type = 'balance_update'",
    )?;
    let days = stmt.query_map(params![account_id], |row| day_at(row, "day"))?;
    days.collect()
}

pub fn insert_bank_entry(conn: &Connection, params: InsertBankEntryParams) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO bank_entry (event_id, day, amount_minor, text) VALUES (?1, ?2, ?3, ?4)",
        params![
            params.event_id,
            params.day.format("%Y-%m-%d").to_string(),
            params.amount_minor,
            params.text
        ],
    )?;
    Ok(())
}

pub fn lowest_leg_id(conn: &Connection, split_group_id: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT MIN(id) FROM event WHERE split_group_id = ?1",
        params![split_group_id],
        |row| row.get(0),
    )
}

fn recorded(row: &Row<'_>) -> rusqlite::Result<Recorded> {
    let bank_day: Option<String> = row.get("bank_day")?;
    let bank = match bank_day {
        Some(_) => Some(BankFacts {
            day: day_at(row, "bank_day")?,
            amount_minor: row.get("bank_amount_minor")?,
        }),
        None => None,
    };
    Ok(Recorded {
        bank_entry_event_id: row.get("bank_entry_event_id")?,
        latest_data_id: row.get("latest_data_id")?,
        event_date: row.get("event_date")?,
        day: day_at(row, "event_date")?,
        amount_minor: row.get("amount_minor")?,
        note: row.get("note")?,
        bank,
    })
}

fn day_at(row: &Row<'_>, column: &str) -> rusqlite::Result<NaiveDate> {
    let index = row.as_ref().column_index(column)?;
    let value: String = row.get(index)?;
    value
        .get(..10)
        .and_then(|day| NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
        .ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                Type::Text,
                format!("not a date: {value}").into(),
            )
        })
}
