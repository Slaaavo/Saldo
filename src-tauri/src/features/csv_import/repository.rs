use chrono::NaiveDate;
use rusqlite::types::Type;
use rusqlite::{params, Connection, Row};

use super::matcher::{
    BankFacts, BankState, CounterpartCashflow, LedgerItem, Recorded, TransferLeg,
};

pub struct CounterpartCashflowsParams {
    pub account_id: i64,
    pub importing_account_id: i64,
}

pub struct InsertBankEntryParams {
    pub event_id: i64,
    pub day: NaiveDate,
    pub amount_minor: i64,
    pub text: Option<String>,
    pub card: Option<String>,
    /// Set for a pending hold: the newest row day of the export that listed it.
    pub hold_seen_to: Option<NaiveDate>,
}

pub struct SeeHoldUntilParams {
    pub event_id: i64,
    pub seen_to: NaiveDate,
}

pub struct PostHoldParams {
    pub event_id: i64,
    pub day: NaiveDate,
    pub amount_minor: i64,
    pub text: Option<String>,
    pub card: Option<String>,
    /// The entry's ledger amount after the posting minus before.
    pub posting_change_minor: i64,
}

pub struct SetSplitGroupNoteParams {
    pub split_group_id: i64,
    pub note: Option<String>,
}

pub struct LiveLeg {
    pub event_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
}

/// The clock of `event.created_at` and `balance_check.checked_at`.
const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%f', 'now')";

const BANK_COLUMNS: &str = "be.day AS bank_day, be.amount_minor AS bank_amount_minor,
                            be.text AS bank_text, be.card AS bank_card,
                            be.hold_seen_to AS hold_seen_to,
                            be.posting_change_minor AS posting_change_minor";

pub fn ledger_items(conn: &Connection, account_id: i64) -> rusqlite::Result<Vec<LedgerItem>> {
    let mut singles = conn.prepare(&format!(
        "SELECT e.id AS bank_entry_event_id, e.latest_data_id AS latest_data_id,
                NULL AS split_group_id,
                ed.event_date AS event_date, ed.amount_minor AS amount_minor, ed.note AS note,
                {BANK_COLUMNS},
                other.id AS other_account_id, other.name AS other_account_name,
                COALESCE(other.iban, '') <> '' AS other_account_has_iban,
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
           AND e.event_type IN ('cashflow', 'transfer')
         ORDER BY e.id"
    ))?;
    let mut items: Vec<LedgerItem> = singles
        .query_map(params![account_id], |row| {
            let other_account_id: Option<i64> = row.get("other_account_id")?;
            let transfer_leg = match other_account_id {
                Some(other_account_id) => Some(TransferLeg {
                    other_account_id,
                    other_account_name: row.get("other_account_name")?,
                    other_account_has_iban: row.get("other_account_has_iban")?,
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

    let mut groups = conn.prepare(&format!(
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
                groups.split_group_id AS split_group_id,
                ed.event_date AS event_date, groups.amount_minor AS amount_minor, sg.note AS note,
                {BANK_COLUMNS}
         FROM groups
         JOIN event lead ON lead.id = groups.lead_id
         JOIN event_data ed ON ed.id = lead.latest_data_id
         JOIN split_group sg ON sg.id = groups.split_group_id
         LEFT JOIN bank_entry be ON be.event_id = lead.id
         ORDER BY lead.id"
    ))?;
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
    let mut stmt = conn.prepare(&format!(
        "SELECT e.id AS bank_entry_event_id, e.latest_data_id AS latest_data_id,
                NULL AS split_group_id,
                ed.event_date AS event_date, ed.amount_minor AS amount_minor, ed.note AS note,
                {BANK_COLUMNS}, a.name AS account_name
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
           AND (ed.counterpart_account_id IS NULL OR ed.counterpart_account_id = ?2)
         ORDER BY e.id"
    ))?;
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
        "INSERT INTO bank_entry (event_id, day, amount_minor, text, card, hold_seen_to)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            params.event_id,
            iso_day(params.day),
            params.amount_minor,
            params.text,
            params.card,
            params.hold_seen_to.map(iso_day),
        ],
    )?;
    Ok(())
}

/// Raises a pending hold's `hold_seen_to` to `seen_to` when that is later.
pub fn see_hold_until(conn: &Connection, params: SeeHoldUntilParams) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE bank_entry SET hold_seen_to = ?2 WHERE event_id = ?1 AND hold_seen_to < ?2",
        params![params.event_id, iso_day(params.seen_to)],
    )?;
    Ok(())
}

/// The hold's bank entry takes the booked line's facts.
pub fn post_hold(conn: &Connection, params: PostHoldParams) -> rusqlite::Result<()> {
    conn.execute(
        &format!(
            "UPDATE bank_entry
             SET day = ?2, amount_minor = ?3, text = ?4, card = ?5, hold_seen_to = NULL,
                 posted_at = {NOW}, posting_change_minor = ?6
             WHERE event_id = ?1"
        ),
        params![
            params.event_id,
            iso_day(params.day),
            params.amount_minor,
            params.text,
            params.card,
            params.posting_change_minor,
        ],
    )?;
    Ok(())
}

pub fn cancel_hold(conn: &Connection, event_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        &format!(
            "UPDATE bank_entry SET hold_seen_to = NULL, cancelled_at = {NOW} WHERE event_id = ?1"
        ),
        params![event_id],
    )?;
    Ok(())
}

pub fn live_legs(conn: &Connection, split_group_id: i64) -> rusqlite::Result<Vec<LiveLeg>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, ed.amount_minor, ed.event_date, ed.note
         FROM event e
         JOIN event_data ed ON ed.id = e.latest_data_id
         WHERE e.split_group_id = ?1 AND e.deleted_at IS NULL
         ORDER BY e.id",
    )?;
    let legs = stmt.query_map(params![split_group_id], |row| {
        Ok(LiveLeg {
            event_id: row.get(0)?,
            amount_minor: row.get(1)?,
            event_date: row.get(2)?,
            note: row.get(3)?,
        })
    })?;
    legs.collect()
}

pub fn set_split_group_note(
    conn: &Connection,
    params: SetSplitGroupNoteParams,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE split_group SET note = ?2 WHERE id = ?1",
        params![params.split_group_id, params.note],
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
            text: row.get("bank_text")?,
            card: row.get("bank_card")?,
            state: bank_state(row)?,
        }),
        None => None,
    };
    Ok(Recorded {
        bank_entry_event_id: row.get("bank_entry_event_id")?,
        latest_data_id: row.get("latest_data_id")?,
        split_group_id: row.get("split_group_id")?,
        event_date: row.get("event_date")?,
        day: day_at(row, "event_date")?,
        amount_minor: row.get("amount_minor")?,
        note: row.get("note")?,
        bank,
    })
}

/// A bank entry is in one state at a time: storage CHECKs refuse a second one.
fn bank_state(row: &Row<'_>) -> rusqlite::Result<BankState> {
    let hold_seen_to: Option<String> = row.get("hold_seen_to")?;
    let posting_change_minor: Option<i64> = row.get("posting_change_minor")?;
    Ok(match (hold_seen_to, posting_change_minor) {
        (Some(_), _) => BankState::Pending {
            seen_to: day_at(row, "hold_seen_to")?,
        },
        (None, Some(change_minor)) => BankState::Posted { change_minor },
        (None, None) => BankState::Booked,
    })
}

fn iso_day(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
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
