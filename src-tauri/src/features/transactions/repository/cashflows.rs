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

/// Amount of the counterpart leg of a transfer: the opposite sign of the source `amount_minor`,
/// with the magnitude of `original_amount_minor` when the transfer crosses currencies.
pub(crate) fn counterpart_leg_amount(amount_minor: i64, original_amount_minor: Option<i64>) -> i64 {
    match original_amount_minor {
        Some(original) if amount_minor > 0 => -original.abs(),
        Some(original) => original.abs(),
        None => -amount_minor,
    }
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

        let counterpart_amount =
            counterpart_leg_amount(entry.amount_minor, entry.original_amount_minor);

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
                let counterpart_amount =
                    counterpart_leg_amount(entry.amount_minor, entry.original_amount_minor);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};

    fn mk_account(conn: &Connection) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: "Test Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn latest_amount(conn: &Connection, event_id: i64) -> i64 {
        conn.query_row(
            "SELECT ed.amount_minor FROM event e JOIN event_data ed ON ed.id = e.latest_data_id WHERE e.id = ?1",
            params![event_id],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn fx_transfer(
        from: i64,
        to: i64,
        amount_minor: i64,
        original_amount_minor: Option<i64>,
    ) -> CashflowEntry {
        CashflowEntry {
            account_id: from,
            amount_minor,
            event_date: "2026-03-01".to_owned(),
            note: None,
            counterpart_account_id: Some(to),
            bucket_id: None,
            original_currency_id: original_amount_minor.map(|_| 1),
            original_amount_minor,
            fx_rate_mantissa: original_amount_minor.map(|_| 1),
            fx_rate_exponent: original_amount_minor.map(|_| 0),
        }
    }

    #[test]
    fn counterpart_leg_amount_inverts_the_source_amount_without_an_original() {
        assert_eq!(counterpart_leg_amount(-5000, None), 5000);
        assert_eq!(counterpart_leg_amount(5000, None), -5000);
    }

    #[test]
    fn counterpart_leg_amount_takes_the_original_magnitude_with_the_opposite_sign() {
        assert_eq!(counterpart_leg_amount(-5000, Some(4500)), 4500);
        assert_eq!(counterpart_leg_amount(-5000, Some(-4500)), 4500);
        assert_eq!(counterpart_leg_amount(5000, Some(4500)), -4500);
        assert_eq!(counterpart_leg_amount(5000, Some(-4500)), -4500);
    }

    #[test]
    fn create_transfer_credits_the_counterpart_when_original_has_the_source_sign() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (from, to) = (mk_account(&conn), mk_account(&conn));

        let (source_id, counterpart_id) =
            create_transfer(&conn, &fx_transfer(from, to, -5000, Some(-4500))).unwrap();

        assert_eq!(latest_amount(&conn, source_id), -5000);
        assert_eq!(latest_amount(&conn, counterpart_id), 4500);
    }

    #[test]
    fn create_transfer_still_credits_the_counterpart_with_a_positive_original() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (from, to) = (mk_account(&conn), mk_account(&conn));

        let (_, counterpart_id) =
            create_transfer(&conn, &fx_transfer(from, to, -5000, Some(4500))).unwrap();

        assert_eq!(latest_amount(&conn, counterpart_id), 4500);
    }

    #[test]
    fn bulk_create_cashflows_credits_the_counterpart_when_original_has_the_source_sign() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (from, to) = (mk_account(&conn), mk_account(&conn));

        let ids =
            bulk_create_cashflows(&conn, &[fx_transfer(from, to, -5000, Some(-4500))]).unwrap();

        assert_eq!(latest_amount(&conn, ids[0]), -5000);
        assert_eq!(latest_amount(&conn, ids[1]), 4500);
    }
}
