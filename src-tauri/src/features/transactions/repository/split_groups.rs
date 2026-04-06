use super::cashflows::{create_cashflow_inner, CashflowEntry};
use crate::error::AppError;
use crate::features::accounts::repository::get_account_type;
use crate::features::transactions::models::SplitGroupEntry;
use crate::shared::with_savepoint_app;
use rusqlite::{params, Connection, OptionalExtension};

pub struct CreateSplitGroupWithLegsParams {
    pub account_id: i64,
    pub group_note: Option<String>,
    pub legs: Vec<SplitGroupEntry>,
}

pub fn create_split_group_with_legs(
    conn: &Connection,
    params: CreateSplitGroupWithLegsParams,
) -> Result<i64, AppError> {
    let CreateSplitGroupWithLegsParams {
        account_id,
        group_note,
        legs,
    } = params;
    with_savepoint_app(conn, || {
        conn.execute(
            "INSERT INTO split_group (note) VALUES (?1)",
            params![group_note],
        )?;
        let split_group_id = conn.last_insert_rowid();

        for leg in &legs {
            let counterpart_type = match leg.counterpart_account_id {
                Some(cp_id) => get_account_type(conn, cp_id)?,
                None => None,
            };

            let leg_event_id = if counterpart_type.as_deref() == Some("account") {
                let counterpart_account_id = leg.counterpart_account_id.unwrap();
                let counterpart_amount = leg.original_amount_minor.unwrap_or(-leg.amount_minor);

                let source_entry = CashflowEntry {
                    account_id,
                    amount_minor: leg.amount_minor,
                    event_date: leg.event_date.clone(),
                    note: leg.note.clone(),
                    counterpart_account_id: leg.counterpart_account_id,
                    bucket_id: leg.bucket_id,
                    original_currency_id: leg.original_currency_id,
                    original_amount_minor: leg.original_amount_minor,
                    fx_rate_mantissa: leg.fx_rate_mantissa,
                    fx_rate_exponent: leg.fx_rate_exponent,
                };
                let source_id = create_cashflow_inner(conn, &source_entry, "transfer", None)?;

                let counterpart_entry = CashflowEntry {
                    account_id: counterpart_account_id,
                    amount_minor: counterpart_amount,
                    event_date: leg.event_date.clone(),
                    note: leg.note.clone(),
                    counterpart_account_id: Some(account_id),
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

                source_id
            } else {
                let entry = CashflowEntry {
                    account_id,
                    amount_minor: leg.amount_minor,
                    event_date: leg.event_date.clone(),
                    note: leg.note.clone(),
                    counterpart_account_id: leg.counterpart_account_id,
                    bucket_id: leg.bucket_id,
                    original_currency_id: leg.original_currency_id,
                    original_amount_minor: leg.original_amount_minor,
                    fx_rate_mantissa: leg.fx_rate_mantissa,
                    fx_rate_exponent: leg.fx_rate_exponent,
                };
                create_cashflow_inner(conn, &entry, "cashflow", None)?
            };

            conn.execute(
                "UPDATE event SET split_group_id = ?1 WHERE id = ?2",
                params![split_group_id, leg_event_id],
            )?;
        }

        Ok(split_group_id)
    })
}

pub struct UpdateSplitGroupDateParams {
    pub split_group_id: i64,
    pub new_date: String,
}

pub fn update_split_group_date(
    conn: &Connection,
    params: UpdateSplitGroupDateParams,
) -> Result<(), AppError> {
    let UpdateSplitGroupDateParams {
        split_group_id,
        new_date,
    } = params;
    with_savepoint_app(conn, || {
        let mut stmt = conn.prepare(
            "SELECT e.id FROM event e WHERE e.split_group_id = ?1 AND e.deleted_at IS NULL",
        )?;
        let event_ids: Vec<i64> = stmt
            .query_map(params![split_group_id], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;

        if event_ids.is_empty() {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Split group not found or has no active legs".into(),
            });
        }

        for event_id in event_ids {
            conn.execute(
                "INSERT INTO event_data (event_id, amount_minor, event_date, note, counterpart_account_id, bucket_id, original_currency_id, original_amount_minor, fx_rate_mantissa, fx_rate_exponent)
                 SELECT event_id, amount_minor, ?1, note, counterpart_account_id, bucket_id, original_currency_id, original_amount_minor, fx_rate_mantissa, fx_rate_exponent
                 FROM event_data WHERE id = (SELECT latest_data_id FROM event WHERE id = ?2)",
                params![new_date, event_id],
            )?;
        }

        Ok(())
    })
}

pub struct CheckEventSplitGroupDateConflictParams {
    pub event_id: i64,
    pub new_date: String,
}

pub fn check_event_split_group_date_conflict(
    conn: &Connection,
    params: CheckEventSplitGroupDateConflictParams,
) -> Result<(), AppError> {
    let CheckEventSplitGroupDateConflictParams { event_id, new_date } = params;
    let result: Option<(Option<i64>, String)> = conn
        .query_row(
            "SELECT e.split_group_id, ed.event_date
             FROM event e
             JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.id = ?1 AND e.deleted_at IS NULL",
            params![event_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    if let Some((Some(_), current_date)) = result {
        if current_date != new_date {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Cannot change the date of an event that belongs to a split group. Use update_split_group_date instead.".into(),
            });
        }
    }

    Ok(())
}
