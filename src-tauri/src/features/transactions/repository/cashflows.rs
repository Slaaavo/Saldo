use crate::features::accounts::repository::get_account_type;
use crate::shared::with_savepoint;
use rusqlite::{params, Connection};

pub struct CashflowEntry {
    pub account_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub counterpart_account_id: Option<i64>,
    pub bucket_id: Option<i64>,
    pub original_currency_id: Option<i64>,
    pub original_amount_minor: Option<i64>,
    pub fx_rate_mantissa: Option<i64>,
    pub fx_rate_exponent: Option<i64>,
}

pub(crate) fn create_cashflow_inner(
    conn: &Connection,
    entry: &CashflowEntry,
    event_type: &str,
    linked_event_id: Option<i64>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO event (account_id, event_type, linked_event_id) VALUES (?1, ?2, ?3)",
        params![entry.account_id, event_type, linked_event_id],
    )?;
    let event_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO event_data (event_id, amount_minor, event_date, note, counterpart_account_id, bucket_id, original_currency_id, original_amount_minor, fx_rate_mantissa, fx_rate_exponent)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            event_id,
            entry.amount_minor,
            entry.event_date,
            entry.note.as_deref(),
            entry.counterpart_account_id,
            entry.bucket_id,
            entry.original_currency_id,
            entry.original_amount_minor,
            entry.fx_rate_mantissa,
            entry.fx_rate_exponent
        ],
    )?;

    Ok(event_id)
}

pub fn create_cashflow(conn: &Connection, entry: &CashflowEntry) -> rusqlite::Result<i64> {
    with_savepoint(conn, || {
        create_cashflow_inner(conn, entry, "cashflow", None)
    })
}

pub fn create_transfer(conn: &Connection, entry: &CashflowEntry) -> rusqlite::Result<(i64, i64)> {
    with_savepoint(conn, || {
        let counterpart_account_id = entry
            .counterpart_account_id
            .expect("create_transfer requires counterpart_account_id");

        let source_id = create_cashflow_inner(conn, entry, "transfer", None)?;

        // For cross-currency transfers, counterpart amount = source's original_amount_minor.
        // For same-currency transfers, counterpart amount = inverted source amount.
        let counterpart_amount = entry.original_amount_minor.unwrap_or(-entry.amount_minor);

        let counterpart_entry = CashflowEntry {
            account_id: counterpart_account_id,
            amount_minor: counterpart_amount,
            event_date: entry.event_date.clone(),
            note: entry.note.clone(),
            counterpart_account_id: Some(entry.account_id),
            bucket_id: None,
            original_currency_id: None,
            original_amount_minor: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
        };

        let counterpart_id =
            create_cashflow_inner(conn, &counterpart_entry, "transfer", Some(source_id))?;

        conn.execute(
            "UPDATE event SET linked_event_id = ?1 WHERE id = ?2",
            params![counterpart_id, source_id],
        )?;

        Ok((source_id, counterpart_id))
    })
}

pub fn bulk_create_cashflows(
    conn: &Connection,
    entries: &[CashflowEntry],
) -> rusqlite::Result<Vec<i64>> {
    with_savepoint(conn, || {
        let mut ids = Vec::new();
        for entry in entries {
            let counterpart_type = match entry.counterpart_account_id {
                Some(cp_id) => get_account_type(conn, cp_id)?,
                None => None,
            };

            if counterpart_type.as_deref() == Some("account") {
                let counterpart_account_id = entry.counterpart_account_id.unwrap();
                let counterpart_amount = entry.original_amount_minor.unwrap_or(-entry.amount_minor);

                let source_id = create_cashflow_inner(conn, entry, "transfer", None)?;

                let counterpart_entry = CashflowEntry {
                    account_id: counterpart_account_id,
                    amount_minor: counterpart_amount,
                    event_date: entry.event_date.clone(),
                    note: entry.note.clone(),
                    counterpart_account_id: Some(entry.account_id),
                    bucket_id: None,
                    original_currency_id: None,
                    original_amount_minor: None,
                    fx_rate_mantissa: None,
                    fx_rate_exponent: None,
                };

                let counterpart_id =
                    create_cashflow_inner(conn, &counterpart_entry, "transfer", Some(source_id))?;

                conn.execute(
                    "UPDATE event SET linked_event_id = ?1 WHERE id = ?2",
                    params![counterpart_id, source_id],
                )?;

                ids.push(source_id);
                ids.push(counterpart_id);
            } else {
                let event_id = create_cashflow_inner(conn, entry, "cashflow", None)?;
                ids.push(event_id);
            }
        }
        Ok(ids)
    })
}
