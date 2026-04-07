use crate::error::AppError;
use crate::shared::{local_now, with_savepoint, with_savepoint_app};
use rusqlite::{params, Connection, OptionalExtension};

// ---------------------------------------------------------------------------
// Params structs
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct CreateTaxableEventParams {
    pub account_id: i64,
    pub event_type: String,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_deductible_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_period_months: Option<i64>,
}

pub struct TaxableEventLeg {
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_deductible_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_period_months: Option<i64>,
}

#[derive(Default)]
pub struct CreateTaxableSplitGroupWithLegsParams {
    pub account_id: i64,
    pub event_type: String,
    pub group_note: Option<String>,
    pub legs: Vec<TaxableEventLeg>,
}

#[derive(Default)]
pub struct UpdateTaxableEventParams {
    pub event_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_deductible_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_period_months: Option<i64>,
}

pub struct UpdatedSplitLeg {
    pub event_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_deductible_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_period_months: Option<i64>,
}

pub struct NewSplitLeg {
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_deductible_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_period_months: Option<i64>,
}

#[derive(Default)]
pub struct UpdateTaxableSplitGroupParams {
    pub split_group_id: i64,
    pub group_note: Option<String>,
    pub updated_legs: Vec<UpdatedSplitLeg>,
    pub new_legs: Vec<NewSplitLeg>,
    pub removed_leg_ids: Vec<i64>,
}

// ---------------------------------------------------------------------------
// Helper: insert a taxable event_data row (used by both create and update).
// The trigger trg_eventdata_after_insert automatically updates event.latest_data_id.
// ---------------------------------------------------------------------------
#[allow(clippy::too_many_arguments)]
fn insert_taxable_event_data(
    conn: &Connection,
    event_id: i64,
    amount_minor: i64,
    event_date: &str,
    note: Option<&str>,
    vat_rate_bps: Option<i64>,
    vat_deductible_pct_bps: Option<i64>,
    expense_deductible_pct_bps: Option<i64>,
    prepaid_period_months: Option<i64>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO event_data (
            event_id, amount_minor, event_date, note,
            vat_rate_bps, vat_deductible_pct_bps,
            expense_deductible_pct_bps, prepaid_period_months
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            event_id,
            amount_minor,
            event_date,
            note,
            vat_rate_bps,
            vat_deductible_pct_bps,
            expense_deductible_pct_bps,
            prepaid_period_months,
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Insert a single revenue or expense event with its event_data.
pub fn create_taxable_event(
    conn: &Connection,
    params: CreateTaxableEventParams,
) -> Result<i64, AppError> {
    with_savepoint_app(conn, || {
        conn.execute(
            "INSERT INTO event (account_id, event_type) VALUES (?1, ?2)",
            params![params.account_id, params.event_type.as_str()],
        )?;
        let event_id = conn.last_insert_rowid();

        insert_taxable_event_data(
            conn,
            event_id,
            params.amount_minor,
            params.event_date.as_str(),
            params.note.as_deref(),
            params.vat_rate_bps,
            params.vat_deductible_pct_bps,
            params.expense_deductible_pct_bps,
            params.prepaid_period_months,
        )
        .map_err(AppError::from)?;

        Ok(event_id)
    })
}

/// Insert a split group with multiple taxable legs.
pub fn create_taxable_split_group_with_legs(
    conn: &Connection,
    params: CreateTaxableSplitGroupWithLegsParams,
) -> Result<i64, AppError> {
    with_savepoint_app(conn, || {
        conn.execute(
            "INSERT INTO split_group (note) VALUES (?1)",
            params![params.group_note],
        )?;
        let split_group_id = conn.last_insert_rowid();

        for leg in &params.legs {
            conn.execute(
                "INSERT INTO event (account_id, event_type, split_group_id) VALUES (?1, ?2, ?3)",
                params![
                    params.account_id,
                    params.event_type.as_str(),
                    split_group_id
                ],
            )?;
            let event_id = conn.last_insert_rowid();

            insert_taxable_event_data(
                conn,
                event_id,
                leg.amount_minor,
                leg.event_date.as_str(),
                leg.note.as_deref(),
                leg.vat_rate_bps,
                leg.vat_deductible_pct_bps,
                leg.expense_deductible_pct_bps,
                leg.prepaid_period_months,
            )
            .map_err(AppError::from)?;
        }

        Ok(split_group_id)
    })
}

/// Update a standalone (or split-group) taxable event by inserting a new event_data row.
pub fn update_taxable_event(
    conn: &Connection,
    params: UpdateTaxableEventParams,
) -> Result<(), AppError> {
    let row: Option<(Option<String>, String)> = conn
        .query_row(
            "SELECT deleted_at, event_type FROM event WHERE id = ?1",
            params![params.event_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(AppError::from)?;

    match row {
        None => {
            return Err(AppError {
                code: "NOT_FOUND".into(),
                message: "Event not found".into(),
            })
        }
        Some((Some(_), _)) => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Cannot update a deleted event".into(),
            })
        }
        Some((None, ref ev_type)) if ev_type != "revenue" && ev_type != "expense" => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Event is not a taxable event type (revenue or expense)".into(),
            });
        }
        Some((None, _)) => {} // valid, proceed
    }

    insert_taxable_event_data(
        conn,
        params.event_id,
        params.amount_minor,
        params.event_date.as_str(),
        params.note.as_deref(),
        params.vat_rate_bps,
        params.vat_deductible_pct_bps,
        params.expense_deductible_pct_bps,
        params.prepaid_period_months,
    )
    .map_err(AppError::from)?;

    Ok(())
}

/// Atomically update a taxable split group: soft-delete legs, update legs, and add new legs.
///
/// Returns an error if fewer than 2 non-deleted legs would remain after applying all changes.
pub fn update_taxable_split_group(
    conn: &Connection,
    params: UpdateTaxableSplitGroupParams,
) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        // Fetch the split group's account_id and event_type from a non-deleted leg.
        let group_row: Option<(i64, String)> = conn
            .query_row(
                "SELECT e.account_id, e.event_type FROM event e
                 WHERE e.split_group_id = ?1 AND e.deleted_at IS NULL
                 LIMIT 1",
                params![params.split_group_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        let (account_id, event_type) = group_row.ok_or_else(|| AppError {
            code: "NOT_FOUND".into(),
            message: "Split group not found or has no active legs".into(),
        })?;

        // Count non-deleted legs currently in the group.
        let current_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM event WHERE split_group_id = ?1 AND deleted_at IS NULL",
            params![params.split_group_id],
            |row| row.get(0),
        )?;

        // After removals and additions, at least 2 legs must remain.
        let remaining =
            current_count - params.removed_leg_ids.len() as i64 + params.new_legs.len() as i64;
        if remaining < 2 {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "A split group must have at least 2 non-deleted legs after all changes"
                    .into(),
            });
        }

        // Update split group note.
        conn.execute(
            "UPDATE split_group SET note = ?1 WHERE id = ?2",
            params![params.group_note, params.split_group_id],
        )?;

        // Process removals.
        let now = local_now();
        for &leg_id in &params.removed_leg_ids {
            let row: Option<(i64, String)> = conn
                .query_row(
                    "SELECT split_group_id, event_type FROM event WHERE id = ?1 AND deleted_at IS NULL",
                    params![leg_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;

            match row {
                None => {
                    return Err(AppError {
                        code: "NOT_FOUND".into(),
                        message: format!("Event {} not found or already deleted", leg_id),
                    })
                }
                Some((sg_id, ref ev_type)) => {
                    if sg_id != params.split_group_id {
                        return Err(AppError {
                            code: "VALIDATION".into(),
                            message: format!(
                                "Event {} does not belong to this split group",
                                leg_id
                            ),
                        });
                    }
                    if ev_type != "revenue" && ev_type != "expense" {
                        return Err(AppError {
                            code: "VALIDATION".into(),
                            message: format!("Event {} is not a taxable event type", leg_id),
                        });
                    }
                }
            }

            conn.execute(
                "UPDATE event SET deleted_at = ?1 WHERE id = ?2",
                params![now, leg_id],
            )?;
        }

        // Process updates.
        for leg in &params.updated_legs {
            let row: Option<i64> = conn
                .query_row(
                    "SELECT split_group_id FROM event WHERE id = ?1 AND deleted_at IS NULL",
                    params![leg.event_id],
                    |row| row.get(0),
                )
                .optional()?;

            match row {
                None => {
                    return Err(AppError {
                        code: "NOT_FOUND".into(),
                        message: format!("Event {} not found or deleted", leg.event_id),
                    })
                }
                Some(sg_id) if sg_id != params.split_group_id => {
                    return Err(AppError {
                        code: "VALIDATION".into(),
                        message: format!(
                            "Event {} does not belong to this split group",
                            leg.event_id
                        ),
                    });
                }
                _ => {}
            }

            insert_taxable_event_data(
                conn,
                leg.event_id,
                leg.amount_minor,
                leg.event_date.as_str(),
                leg.note.as_deref(),
                leg.vat_rate_bps,
                leg.vat_deductible_pct_bps,
                leg.expense_deductible_pct_bps,
                leg.prepaid_period_months,
            )?;
        }

        // Process additions.
        for leg in &params.new_legs {
            conn.execute(
                "INSERT INTO event (account_id, event_type, split_group_id) VALUES (?1, ?2, ?3)",
                params![account_id, event_type.as_str(), params.split_group_id],
            )?;
            let event_id = conn.last_insert_rowid();

            insert_taxable_event_data(
                conn,
                event_id,
                leg.amount_minor,
                leg.event_date.as_str(),
                leg.note.as_deref(),
                leg.vat_rate_bps,
                leg.vat_deductible_pct_bps,
                leg.expense_deductible_pct_bps,
                leg.prepaid_period_months,
            )?;
        }

        Ok(())
    })
}

/// Soft-deletes all non-deleted legs belonging to a taxable split group.
pub fn delete_split_group(conn: &Connection, split_group_id: i64) -> rusqlite::Result<()> {
    with_savepoint(conn, || {
        let now = local_now();
        conn.execute(
            "UPDATE event SET deleted_at = ?1 WHERE split_group_id = ?2 AND deleted_at IS NULL",
            params![now, split_group_id],
        )?;
        Ok(())
    })
}
