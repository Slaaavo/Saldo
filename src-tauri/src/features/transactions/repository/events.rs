use crate::error::AppError;
use crate::shared::{local_now, with_savepoint, with_savepoint_app};
use rusqlite::{params, Connection, OptionalExtension};

pub struct UpdateEventParams {
    pub event_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
}

pub fn update_event(conn: &Connection, params: UpdateEventParams) -> Result<(), String> {
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
        .optional()
        .map_err(|e| e.to_string())?;

    match maybe_deleted_at {
        None => return Err("Event not found".to_string()),
        Some(Some(_)) => return Err("Cannot update a deleted event".to_string()),
        Some(None) => {} // active event, proceed
    }

    conn.execute(
        "INSERT INTO event_data (
            event_id, amount_minor, event_date, note,
            counterpart_account_id, bucket_id, original_currency_id,
            original_amount_minor, fx_rate_mantissa, fx_rate_exponent,
            vat_rate_bps, vat_reclaimable_pct_bps,
            expense_deductible_pct_bps, prepaid_period_months, reclaimed_vat
        )
        SELECT ?1, ?2, ?3, ?4,
            counterpart_account_id, bucket_id, original_currency_id,
            original_amount_minor, fx_rate_mantissa, fx_rate_exponent,
            vat_rate_bps, vat_reclaimable_pct_bps,
            expense_deductible_pct_bps, prepaid_period_months, reclaimed_vat
        FROM event_data
        WHERE id = (SELECT latest_data_id FROM event WHERE id = ?1)",
        params![event_id, amount_minor, event_date, note],
    )
    .map_err(|e| e.to_string())?;

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

fn delete_event_inner(conn: &Connection, event_id: i64, now: &str) -> rusqlite::Result<()> {
    let rows = conn.execute(
        "UPDATE event SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, event_id],
    )?;
    if rows == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn delete_event(conn: &Connection, event_id: i64) -> rusqlite::Result<Vec<i64>> {
    with_savepoint(conn, || {
        let now = local_now();

        let linked_event_id: Option<i64> = conn
            .query_row(
                "SELECT linked_event_id FROM event WHERE id = ?1",
                params![event_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();

        delete_event_inner(conn, event_id, &now)?;

        let mut deleted = vec![event_id];
        if let Some(linked_id) = linked_event_id {
            delete_event_inner(conn, linked_id, &now)?;
            deleted.push(linked_id);
        }

        Ok(deleted)
    })
}
