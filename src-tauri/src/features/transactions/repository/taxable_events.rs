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
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_until: Option<String>,
    pub reclaimed_vat: Option<bool>,
    /// The eKasa receipt this event was imported from; see migration 035.
    pub ekasa_receipt_ref: Option<String>,
}

pub struct TaxableEventLeg {
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_until: Option<String>,
    pub reclaimed_vat: Option<bool>,
}

#[derive(Default)]
pub struct CreateTaxableSplitGroupWithLegsParams {
    pub account_id: i64,
    pub event_type: String,
    pub group_note: Option<String>,
    pub legs: Vec<TaxableEventLeg>,
    /// Stored on every leg; see migration 035.
    pub ekasa_receipt_ref: Option<String>,
}

#[derive(Default)]
pub struct UpdateTaxableEventParams {
    pub event_id: i64,
    /// The type the caller believes the event has; it must match the stored type.
    pub event_type: String,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_until: Option<String>,
    pub reclaimed_vat: Option<bool>,
}

pub struct UpdatedSplitLeg {
    pub event_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_until: Option<String>,
    pub reclaimed_vat: Option<bool>,
}

pub struct NewSplitLeg {
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_until: Option<String>,
    pub reclaimed_vat: Option<bool>,
}

#[derive(Default)]
pub struct UpdateTaxableSplitGroupParams {
    pub split_group_id: i64,
    /// The type the caller believes the group has; it must match the stored type.
    pub event_type: String,
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
pub(crate) fn insert_taxable_event_data(
    conn: &Connection,
    event_id: i64,
    amount_minor: i64,
    event_date: &str,
    note: Option<&str>,
    vat_rate_bps: Option<i64>,
    vat_reclaimable_pct_bps: Option<i64>,
    expense_deductible_pct_bps: Option<i64>,
    prepaid_until: Option<&str>,
    reclaimed_vat: Option<bool>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO event_data (
            event_id, amount_minor, event_date, note,
            vat_rate_bps, vat_reclaimable_pct_bps,
            expense_deductible_pct_bps, prepaid_until, reclaimed_vat
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            event_id,
            amount_minor,
            event_date,
            note,
            vat_rate_bps,
            vat_reclaimable_pct_bps,
            expense_deductible_pct_bps,
            prepaid_until,
            reclaimed_vat.map(|v| v as i64),
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
            "INSERT INTO event (account_id, event_type, ekasa_receipt_ref) VALUES (?1, ?2, ?3)",
            params![
                params.account_id,
                params.event_type.as_str(),
                params.ekasa_receipt_ref
            ],
        )?;
        let event_id = conn.last_insert_rowid();

        insert_taxable_event_data(
            conn,
            event_id,
            params.amount_minor,
            params.event_date.as_str(),
            params.note.as_deref(),
            params.vat_rate_bps,
            params.vat_reclaimable_pct_bps,
            params.expense_deductible_pct_bps,
            params.prepaid_until.as_deref(),
            params.reclaimed_vat,
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
                "INSERT INTO event (account_id, event_type, split_group_id, ekasa_receipt_ref)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    params.account_id,
                    params.event_type.as_str(),
                    split_group_id,
                    params.ekasa_receipt_ref
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
                leg.vat_reclaimable_pct_bps,
                leg.expense_deductible_pct_bps,
                leg.prepaid_until.as_deref(),
                leg.reclaimed_vat,
            )
            .map_err(AppError::from)?;
        }

        Ok(split_group_id)
    })
}

/// What `update_taxable_event` stored, for callers that regenerate prepaid children.
pub struct UpdatedTaxableEvent {
    pub account_id: i64,
    pub reclaimed_vat: Option<bool>,
}

/// An expense-kind event with no `reclaimed_vat` takes the owning person's `vat_payer`;
/// other events never store one.
fn resolve_reclaimed_vat(
    event_type: &str,
    reclaimed_vat: Option<bool>,
    person_vat_payer: Option<bool>,
) -> Option<bool> {
    match event_type {
        "expense" | "prepaid_expense" => reclaimed_vat.or(person_vat_payer),
        _ => None,
    }
}

/// Update a standalone (or split-group) taxable event by inserting a new event_data row.
pub fn update_taxable_event(
    conn: &Connection,
    params: UpdateTaxableEventParams,
) -> Result<UpdatedTaxableEvent, AppError> {
    let row: Option<(Option<String>, String, i64, Option<bool>)> = conn
        .query_row(
            "SELECT e.deleted_at, e.event_type, e.account_id, p.vat_payer
             FROM event e
             JOIN account a ON a.id = e.account_id
             LEFT JOIN person p ON p.id = a.person_id
             WHERE e.id = ?1",
            params![params.event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(AppError::from)?;

    let (account_id, event_type, person_vat_payer) = match row {
        None => {
            return Err(AppError {
                code: "NOT_FOUND".into(),
                message: "Event not found".into(),
            })
        }
        Some((Some(_), _, _, _)) => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Cannot update a deleted event".into(),
            })
        }
        Some((None, ref ev_type, _, _))
            if ev_type != "revenue" && ev_type != "expense" && ev_type != "prepaid_expense" =>
        {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Event is not a taxable event type (revenue, expense, or prepaid_expense)"
                    .into(),
            });
        }
        Some((None, ev_type, account_id, vat_payer)) => (account_id, ev_type, vat_payer),
    };
    if event_type != params.event_type {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: format!(
                "Event is a '{}' event, not '{}'; its type cannot be changed",
                event_type, params.event_type
            ),
        });
    }
    let reclaimed_vat = resolve_reclaimed_vat(&event_type, params.reclaimed_vat, person_vat_payer);

    insert_taxable_event_data(
        conn,
        params.event_id,
        params.amount_minor,
        params.event_date.as_str(),
        params.note.as_deref(),
        params.vat_rate_bps,
        params.vat_reclaimable_pct_bps,
        params.expense_deductible_pct_bps,
        params.prepaid_until.as_deref(),
        reclaimed_vat,
    )
    .map_err(AppError::from)?;

    Ok(UpdatedTaxableEvent {
        account_id,
        reclaimed_vat,
    })
}

/// Atomically update a taxable split group: soft-delete legs, update legs, and add new legs.
///
/// Returns an error if fewer than 2 non-deleted legs would remain after applying all changes.
pub fn update_taxable_split_group(
    conn: &Connection,
    params: UpdateTaxableSplitGroupParams,
) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        // Fetch the split group's account_id, event_type and the owner's vat_payer from a
        // non-deleted leg.
        let group_row: Option<(i64, String, Option<bool>)> = conn
            .query_row(
                "SELECT e.account_id, e.event_type, p.vat_payer FROM event e
                 JOIN account a ON a.id = e.account_id
                 LEFT JOIN person p ON p.id = a.person_id
                 WHERE e.split_group_id = ?1 AND e.deleted_at IS NULL
                 LIMIT 1",
                params![params.split_group_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let (account_id, event_type, person_vat_payer) = group_row.ok_or_else(|| AppError {
            code: "NOT_FOUND".into(),
            message: "Split group not found or has no active legs".into(),
        })?;
        if event_type != "revenue" && event_type != "expense" {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Split group is not a revenue or expense group".into(),
            });
        }
        if event_type != params.event_type {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: format!(
                    "Split group is a '{}' group, not '{}'",
                    event_type, params.event_type
                ),
            });
        }

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
                leg.vat_reclaimable_pct_bps,
                leg.expense_deductible_pct_bps,
                leg.prepaid_until.as_deref(),
                resolve_reclaimed_vat(&event_type, leg.reclaimed_vat, person_vat_payer),
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
                leg.vat_reclaimable_pct_bps,
                leg.expense_deductible_pct_bps,
                leg.prepaid_until.as_deref(),
                resolve_reclaimed_vat(&event_type, leg.reclaimed_vat, person_vat_payer),
            )?;
        }

        Ok(())
    })
}

/// Soft-deletes all non-deleted legs belonging to a split group, plus the counterpart
/// leg of each transfer leg (only the source leg carries `split_group_id`).
pub fn delete_split_group(conn: &Connection, split_group_id: i64) -> rusqlite::Result<()> {
    with_savepoint(conn, || {
        let now = local_now();
        conn.execute(
            "UPDATE event SET deleted_at = ?1
             WHERE deleted_at IS NULL
               AND id IN (SELECT linked_event_id FROM event WHERE split_group_id = ?2)",
            params![now, split_group_id],
        )?;
        conn.execute(
            "UPDATE event SET deleted_at = ?1 WHERE split_group_id = ?2 AND deleted_at IS NULL",
            params![now, split_group_id],
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::transactions::models::SplitGroupEntry;
    use crate::features::transactions::repository::{
        create_split_group_with_legs, CreateSplitGroupWithLegsParams,
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

    fn leg(amount_minor: i64, counterpart_account_id: Option<i64>) -> SplitGroupEntry {
        SplitGroupEntry {
            amount_minor,
            event_date: "2026-03-01T12:00:00".to_owned(),
            note: None,
            counterpart_account_id,
            bucket_id: None,
            original_currency_id: None,
            original_amount_minor: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
        }
    }

    #[test]
    fn delete_split_group_also_deletes_transfer_counterpart() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn, "Checking");
        let other_id = mk_account(&conn, "Savings");
        let split_group_id = create_split_group_with_legs(
            &conn,
            CreateSplitGroupWithLegsParams {
                account_id,
                group_note: None,
                legs: vec![leg(-1_000, Some(other_id)), leg(-500, None)],
            },
        )
        .expect("create_split_group_with_legs failed");

        delete_split_group(&conn, split_group_id).expect("delete_split_group failed");

        let active_on_other: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event WHERE account_id = ?1 AND deleted_at IS NULL",
                params![other_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active_on_other, 0, "the counterpart leg must be deleted");
    }
}
