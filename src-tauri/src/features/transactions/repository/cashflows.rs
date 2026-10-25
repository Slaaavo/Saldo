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
fn counterpart_leg_amount(amount_minor: i64, original_amount_minor: Option<i64>) -> i64 {
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

/// Inserts both legs of a transfer between two accounts and links them to each other. The source
/// leg is `entry` as given. The counterpart leg mirrors it: opposite sign, same date and note,
/// pointing back at the source account, and without the bucket or original-currency details.
/// Returns `(source_id, counterpart_id)`.
fn insert_transfer_pair(conn: &Connection, entry: &CashflowEntry) -> rusqlite::Result<(i64, i64)> {
    let counterpart_account_id = entry
        .counterpart_account_id
        .expect("a transfer requires counterpart_account_id");

    let source_id = create_cashflow_inner(conn, entry, "transfer", None)?;

    let counterpart_entry = CashflowEntry {
        account_id: counterpart_account_id,
        amount_minor: counterpart_leg_amount(entry.amount_minor, entry.original_amount_minor),
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
}

/// Inserts `entry` as a transfer pair when its counterpart is a regular account, and as a plain
/// cashflow otherwise. Returns the id of the entry's own event and, for a transfer, of the
/// counterpart leg. Callers wrap this in their own savepoint.
pub(crate) fn create_cashflow_or_transfer(
    conn: &Connection,
    entry: &CashflowEntry,
) -> rusqlite::Result<(i64, Option<i64>)> {
    let counterpart_type = match entry.counterpart_account_id {
        Some(cp_id) => get_account_type(conn, cp_id)?,
        None => None,
    };

    if counterpart_type.as_deref() == Some("account") {
        let (source_id, counterpart_id) = insert_transfer_pair(conn, entry)?;
        Ok((source_id, Some(counterpart_id)))
    } else {
        let event_id = create_cashflow_inner(conn, entry, "cashflow", None)?;
        Ok((event_id, None))
    }
}

pub fn create_transfer(conn: &Connection, entry: &CashflowEntry) -> rusqlite::Result<(i64, i64)> {
    with_savepoint(conn, || insert_transfer_pair(conn, entry))
}

pub fn bulk_create_cashflows(
    conn: &Connection,
    entries: &[CashflowEntry],
) -> rusqlite::Result<Vec<i64>> {
    with_savepoint(conn, || {
        let mut ids = Vec::new();
        for entry in entries {
            let (event_id, counterpart_id) = create_cashflow_or_transfer(conn, entry)?;
            ids.push(event_id);
            ids.extend(counterpart_id);
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

    #[derive(Debug, PartialEq)]
    struct LegRow {
        account_id: i64,
        event_type: String,
        linked_event_id: Option<i64>,
        split_group_id: Option<i64>,
        amount_minor: i64,
        event_date: String,
        note: Option<String>,
        counterpart_account_id: Option<i64>,
        bucket_id: Option<i64>,
        original_currency_id: Option<i64>,
        original_amount_minor: Option<i64>,
        fx_rate_mantissa: Option<i64>,
        fx_rate_exponent: Option<i64>,
    }

    fn leg_row(conn: &Connection, event_id: i64) -> LegRow {
        conn.query_row(
            "SELECT e.account_id, e.event_type, e.linked_event_id, e.split_group_id,
                    ed.amount_minor, ed.event_date, ed.note, ed.counterpart_account_id,
                    ed.bucket_id, ed.original_currency_id, ed.original_amount_minor,
                    ed.fx_rate_mantissa, ed.fx_rate_exponent
             FROM event e JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.id = ?1",
            params![event_id],
            |row| {
                Ok(LegRow {
                    account_id: row.get(0)?,
                    event_type: row.get(1)?,
                    linked_event_id: row.get(2)?,
                    split_group_id: row.get(3)?,
                    amount_minor: row.get(4)?,
                    event_date: row.get(5)?,
                    note: row.get(6)?,
                    counterpart_account_id: row.get(7)?,
                    bucket_id: row.get(8)?,
                    original_currency_id: row.get(9)?,
                    original_amount_minor: row.get(10)?,
                    fx_rate_mantissa: row.get(11)?,
                    fx_rate_exponent: row.get(12)?,
                })
            },
        )
        .unwrap()
    }

    fn mk_bucket(conn: &Connection) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: "Test Bucket".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .expect("create bucket failed")
    }

    /// A cross-currency transfer with every optional field set, so a leg that copies too much
    /// or too little from the source shows up.
    fn full_transfer(from: i64, to: i64, bucket: i64) -> CashflowEntry {
        CashflowEntry {
            account_id: from,
            amount_minor: -5000,
            event_date: "2026-03-01".to_owned(),
            note: Some("rent".to_owned()),
            counterpart_account_id: Some(to),
            bucket_id: Some(bucket),
            original_currency_id: Some(1),
            original_amount_minor: Some(-4500),
            fx_rate_mantissa: Some(9),
            fx_rate_exponent: Some(-1),
        }
    }

    /// The source leg keeps everything the caller gave; the counterpart is its mirror image:
    /// opposite sign, same date and note, pointing back at the source account, with no bucket
    /// and no original-currency details. Each leg links to the other.
    fn assert_transfer_pair(
        conn: &Connection,
        (source_id, counterpart_id): (i64, i64),
        (from, to, bucket): (i64, i64, i64),
        split_group_id: Option<i64>,
    ) {
        assert_eq!(
            leg_row(conn, source_id),
            LegRow {
                account_id: from,
                event_type: "transfer".to_owned(),
                linked_event_id: Some(counterpart_id),
                split_group_id,
                amount_minor: -5000,
                event_date: "2026-03-01".to_owned(),
                note: Some("rent".to_owned()),
                counterpart_account_id: Some(to),
                bucket_id: Some(bucket),
                original_currency_id: Some(1),
                original_amount_minor: Some(-4500),
                fx_rate_mantissa: Some(9),
                fx_rate_exponent: Some(-1),
            }
        );
        assert_eq!(
            leg_row(conn, counterpart_id),
            LegRow {
                account_id: to,
                event_type: "transfer".to_owned(),
                linked_event_id: Some(source_id),
                split_group_id: None,
                amount_minor: 4500,
                event_date: "2026-03-01".to_owned(),
                note: Some("rent".to_owned()),
                counterpart_account_id: Some(from),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            }
        );
    }

    #[test]
    fn create_transfer_links_a_source_leg_and_its_mirrored_counterpart() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (from, to, bucket) = (mk_account(&conn), mk_account(&conn), mk_bucket(&conn));

        let ids = create_transfer(&conn, &full_transfer(from, to, bucket)).unwrap();

        assert_transfer_pair(&conn, ids, (from, to, bucket), None);
    }

    #[test]
    fn bulk_create_cashflows_links_a_source_leg_and_its_mirrored_counterpart() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (from, to, bucket) = (mk_account(&conn), mk_account(&conn), mk_bucket(&conn));

        let ids = bulk_create_cashflows(&conn, &[full_transfer(from, to, bucket)]).unwrap();

        assert_eq!(ids.len(), 2);
        assert_transfer_pair(&conn, (ids[0], ids[1]), (from, to, bucket), None);
    }

    #[test]
    fn bulk_create_cashflows_makes_a_plain_cashflow_unless_the_counterpart_is_a_regular_account() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (from, bucket) = (mk_account(&conn), mk_bucket(&conn));
        let to_bucket = CashflowEntry {
            counterpart_account_id: Some(bucket),
            ..fx_transfer(from, 0, -300, None)
        };
        let no_counterpart = CashflowEntry {
            counterpart_account_id: None,
            ..fx_transfer(from, 0, -200, None)
        };

        let ids = bulk_create_cashflows(&conn, &[to_bucket, no_counterpart]).unwrap();

        assert_eq!(ids.len(), 2);
        let first = leg_row(&conn, ids[0]);
        assert_eq!(first.event_type, "cashflow");
        assert_eq!(first.linked_event_id, None);
        assert_eq!(first.counterpart_account_id, Some(bucket));
        let second = leg_row(&conn, ids[1]);
        assert_eq!(second.event_type, "cashflow");
        assert_eq!(second.counterpart_account_id, None);
    }

    #[test]
    fn split_group_transfer_leg_links_a_source_leg_and_its_mirrored_counterpart() {
        use crate::features::transactions::models::SplitGroupEntry;
        use crate::features::transactions::repository::split_groups::{
            create_split_group_with_legs, CreateSplitGroupWithLegsParams,
        };

        let conn = initialize_in_memory().expect("DB init failed");
        let (from, to, bucket) = (mk_account(&conn), mk_account(&conn), mk_bucket(&conn));
        let transfer_leg = SplitGroupEntry {
            amount_minor: -5000,
            event_date: "2026-03-01".to_owned(),
            note: Some("rent".to_owned()),
            counterpart_account_id: Some(to),
            bucket_id: Some(bucket),
            original_currency_id: Some(1),
            original_amount_minor: Some(-4500),
            fx_rate_mantissa: Some(9),
            fx_rate_exponent: Some(-1),
        };
        let cashflow_leg = SplitGroupEntry {
            amount_minor: -700,
            counterpart_account_id: Some(bucket),
            bucket_id: None,
            original_currency_id: None,
            original_amount_minor: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
            note: None,
            event_date: "2026-03-01".to_owned(),
        };

        let group_id = create_split_group_with_legs(
            &conn,
            CreateSplitGroupWithLegsParams {
                account_id: from,
                group_note: None,
                legs: vec![transfer_leg, cashflow_leg],
            },
        )
        .unwrap();

        let event_ids: Vec<i64> = conn
            .prepare("SELECT id FROM event ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(event_ids.len(), 3);
        assert_transfer_pair(
            &conn,
            (event_ids[0], event_ids[1]),
            (from, to, bucket),
            Some(group_id),
        );
        let plain = leg_row(&conn, event_ids[2]);
        assert_eq!(plain.event_type, "cashflow");
        assert_eq!(plain.linked_event_id, None);
        assert_eq!(plain.split_group_id, Some(group_id));
        assert_eq!(plain.counterpart_account_id, Some(bucket));
    }
}
