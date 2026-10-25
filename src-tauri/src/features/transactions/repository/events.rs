use crate::error::AppError;
use crate::shared::{local_now, with_savepoint_app};
use rusqlite::{params, Connection, OptionalExtension};

pub struct UpdateEventParams {
    pub event_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
}

pub fn update_event(conn: &Connection, params: UpdateEventParams) -> Result<(), AppError> {
    let UpdateEventParams {
        event_id,
        amount_minor,
        event_date,
        note,
    } = params;
    let maybe_deleted_at: Option<Option<String>> = conn
        .query_row(
            "SELECT deleted_at FROM event WHERE id = ?1",
            params![event_id],
            |row| row.get(0),
        )
        .optional()?;

    match maybe_deleted_at {
        None => return Err(event_not_found()),
        Some(Some(_)) => {
            return Err(AppError {
                code: "NOT_FOUND".into(),
                message: "Cannot update a deleted event".into(),
            })
        }
        Some(None) => {} // active event, proceed
    }

    // Copies every column the edit does not change. prepaid_period_months is dead
    // (superseded by prepaid_until) and is left NULL.
    conn.execute(
        "INSERT INTO event_data (
            event_id, amount_minor, event_date, note,
            counterpart_account_id, bucket_id, original_currency_id,
            original_amount_minor, fx_rate_mantissa, fx_rate_exponent,
            vat_rate_bps, vat_reclaimable_pct_bps,
            expense_deductible_pct_bps, reclaimed_vat, prepaid_until
        )
        SELECT ?1, ?2, ?3, ?4,
            counterpart_account_id, bucket_id, original_currency_id,
            original_amount_minor, fx_rate_mantissa, fx_rate_exponent,
            vat_rate_bps, vat_reclaimable_pct_bps,
            expense_deductible_pct_bps, reclaimed_vat, prepaid_until
        FROM event_data
        WHERE id = (SELECT latest_data_id FROM event WHERE id = ?1)",
        params![event_id, amount_minor, event_date, note],
    )?;

    Ok(())
}

pub struct UpdateTransferParams {
    pub from_event_id: i64,
    pub to_event_id: i64,
    pub from_date: String,
    pub to_date: String,
    pub from_amount_minor: i64,
    pub to_amount_minor: i64,
    pub note: Option<String>,
    pub original_currency_id: Option<i64>,
    pub original_amount_minor_for_from_leg: Option<i64>,
    pub fx_rate_mantissa: Option<i64>,
    pub fx_rate_exponent: Option<i64>,
}

pub fn update_transfer(conn: &Connection, params: UpdateTransferParams) -> Result<(), AppError> {
    let UpdateTransferParams {
        from_event_id,
        to_event_id,
        from_date,
        to_date,
        from_amount_minor,
        to_amount_minor,
        note,
        original_currency_id,
        original_amount_minor_for_from_leg,
        fx_rate_mantissa,
        fx_rate_exponent,
    } = params;
    with_savepoint_app(conn, || {
        let from_row: Option<(String, Option<i64>, Option<String>)> = conn
            .query_row(
                "SELECT event_type, linked_event_id, deleted_at FROM event WHERE id = ?1",
                params![from_event_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let (from_event_type, from_linked_event_id, from_deleted_at) =
            from_row.ok_or_else(|| AppError {
                code: "VALIDATION".into(),
                message: "From event not found".into(),
            })?;

        if from_deleted_at.is_some() {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "From event has been deleted".into(),
            });
        }

        if from_event_type != "transfer" {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "From event is not a transfer".into(),
            });
        }

        let to_row: Option<(String, Option<i64>, Option<String>)> = conn
            .query_row(
                "SELECT event_type, linked_event_id, deleted_at FROM event WHERE id = ?1",
                params![to_event_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let (to_event_type, to_linked_event_id, to_deleted_at) =
            to_row.ok_or_else(|| AppError {
                code: "VALIDATION".into(),
                message: "To event not found".into(),
            })?;

        if to_deleted_at.is_some() {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "To event has been deleted".into(),
            });
        }

        if to_event_type != "transfer" {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "To event is not a transfer".into(),
            });
        }

        if from_linked_event_id != Some(to_event_id) || to_linked_event_id != Some(from_event_id) {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Events are not linked to each other".into(),
            });
        }

        conn.execute(
            "INSERT INTO event_data (
                event_id, amount_minor, event_date, note,
                counterpart_account_id, bucket_id,
                original_currency_id, original_amount_minor, fx_rate_mantissa, fx_rate_exponent
            )
            SELECT ?1, ?2, ?3, ?4,
                counterpart_account_id, bucket_id,
                ?5, ?6, ?7, ?8
            FROM event_data
            WHERE id = (SELECT latest_data_id FROM event WHERE id = ?1)",
            params![
                from_event_id,
                from_amount_minor,
                from_date,
                note.as_deref(),
                original_currency_id,
                original_amount_minor_for_from_leg,
                fx_rate_mantissa,
                fx_rate_exponent
            ],
        )?;

        conn.execute(
            "INSERT INTO event_data (
                event_id, amount_minor, event_date, note,
                counterpart_account_id, bucket_id,
                original_currency_id, original_amount_minor, fx_rate_mantissa, fx_rate_exponent
            )
            SELECT ?1, ?2, ?3, ?4,
                counterpart_account_id, bucket_id,
                NULL, NULL, NULL, NULL
            FROM event_data
            WHERE id = (SELECT latest_data_id FROM event WHERE id = ?1)",
            params![to_event_id, to_amount_minor, to_date, note.as_deref()],
        )?;

        Ok(())
    })
}

fn event_not_found() -> AppError {
    AppError {
        code: "NOT_FOUND".into(),
        message: "Event not found".into(),
    }
}

/// Returns the event type of a non-deleted event, or NOT_FOUND.
pub fn get_active_event_type(conn: &Connection, event_id: i64) -> Result<String, AppError> {
    conn.query_row(
        "SELECT event_type FROM event WHERE id = ?1 AND deleted_at IS NULL",
        params![event_id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or_else(event_not_found)
}

/// Soft-deletes the event and returns whether it was still active.
fn soft_delete_event(conn: &Connection, event_id: i64, now: &str) -> rusqlite::Result<bool> {
    let rows = conn.execute(
        "UPDATE event SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, event_id],
    )?;
    Ok(rows > 0)
}

pub fn delete_event(conn: &Connection, event_id: i64) -> Result<Vec<i64>, AppError> {
    with_savepoint_app(conn, || {
        let now = local_now();

        let linked_event_id: Option<i64> = conn
            .query_row(
                "SELECT linked_event_id FROM event WHERE id = ?1",
                params![event_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();

        if !soft_delete_event(conn, event_id, &now)? {
            return Err(event_not_found());
        }

        let mut deleted = vec![event_id];
        // The linked leg may already be gone (e.g. orphaned by an older split-group delete).
        if let Some(linked_id) = linked_event_id {
            if soft_delete_event(conn, linked_id, &now)? {
                deleted.push(linked_id);
            }
        }

        Ok(deleted)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::transactions::repository::{
        create_taxable_event, create_transfer, CashflowEntry, CreateTaxableEventParams,
    };

    fn mk_account(conn: &Connection, name: &str) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: name.to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn mk_transfer(conn: &Connection) -> (i64, i64) {
        let from_id = mk_account(conn, "From");
        let to_id = mk_account(conn, "To");
        create_transfer(
            conn,
            &CashflowEntry {
                account_id: from_id,
                amount_minor: -1_000,
                event_date: "2026-03-01T12:00:00".to_owned(),
                note: None,
                counterpart_account_id: Some(to_id),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .expect("create_transfer failed")
    }

    #[test]
    fn update_event_keeps_taxable_columns() {
        let conn = initialize_in_memory().expect("DB init failed");
        let expense_account_id =
            crate::features::persons::repository::get_default_expense_account_id(&conn, 1).unwrap();
        let event_id = create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id: expense_account_id,
                event_type: "prepaid_expense".to_owned(),
                amount_minor: -12_000,
                event_date: "2026-01-15T00:00:00".to_owned(),
                note: Some("Insurance".to_owned()),
                vat_rate_bps: Some(2300),
                vat_reclaimable_pct_bps: Some(5000),
                expense_deductible_pct_bps: Some(8000),
                prepaid_until: Some("2027-01-14".to_owned()),
                reclaimed_vat: Some(true),
                ..Default::default()
            },
        )
        .unwrap();

        update_event(
            &conn,
            UpdateEventParams {
                event_id,
                amount_minor: -13_000,
                event_date: "2026-01-15T00:00:00".to_owned(),
                note: Some("Insurance (edited)".to_owned()),
            },
        )
        .expect("update_event failed");

        type Row = (
            i64,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<String>,
        );
        let row: Row = conn
            .query_row(
                "SELECT ed.amount_minor, ed.vat_rate_bps, ed.vat_reclaimable_pct_bps,
                        ed.expense_deductible_pct_bps, ed.reclaimed_vat, ed.prepaid_until
                 FROM event e JOIN event_data ed ON ed.id = e.latest_data_id
                 WHERE e.id = ?1",
                params![event_id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            row,
            (
                -13_000,
                Some(2300),
                Some(5000),
                Some(8000),
                Some(1),
                Some("2027-01-14".to_owned())
            )
        );
    }

    #[test]
    fn update_event_returns_not_found_for_missing_or_deleted_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (source_id, _) = mk_transfer(&conn);
        delete_event(&conn, source_id).unwrap();
        for event_id in [source_id, 999_999] {
            let err = update_event(
                &conn,
                UpdateEventParams {
                    event_id,
                    amount_minor: 1,
                    event_date: "2026-03-01T12:00:00".to_owned(),
                    note: None,
                },
            )
            .unwrap_err();
            assert_eq!(err.code, "NOT_FOUND");
        }
    }

    #[test]
    fn delete_event_succeeds_when_linked_leg_already_deleted() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (source_id, counterpart_id) = mk_transfer(&conn);
        conn.execute(
            "UPDATE event SET deleted_at = '2026-03-02T00:00:00' WHERE id = ?1",
            params![source_id],
        )
        .unwrap();

        let deleted = delete_event(&conn, counterpart_id).expect("delete_event failed");

        assert_eq!(deleted, vec![counterpart_id]);
        let deleted_at: Option<String> = conn
            .query_row(
                "SELECT deleted_at FROM event WHERE id = ?1",
                params![counterpart_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(deleted_at.is_some());
    }

    #[test]
    fn delete_event_on_deleted_event_returns_not_found() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (source_id, _) = mk_transfer(&conn);
        delete_event(&conn, source_id).expect("first delete failed");

        let err = delete_event(&conn, source_id).unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(
            get_active_event_type(&conn, source_id).unwrap_err().code,
            "NOT_FOUND"
        );
    }
}
