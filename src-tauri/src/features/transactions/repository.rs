use crate::error::AppError;
use crate::features::assets::repository::get_all_account_asset_link_ids;
use crate::features::buckets::repository::list_all_latest_bucket_links;
use crate::features::currency::repository::{
    get_consolidation_currency, get_fx_rate_for_conversion,
};
use crate::shared::{convert_balance, local_now, with_savepoint, with_savepoint_app};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

use super::models::{EventWithData, ListEventsResult, SnapshotRow, SplitGroupEntry};

type SnapshotRawRow = (
    i64,
    String,
    String,
    Option<String>,
    i64,
    String,
    i64,
    i64,
    i64,
);

pub(crate) fn create_balance_update_inner(
    conn: &Connection,
    account_id: i64,
    amount_minor: i64,
    event_date: &str,
    note: Option<&str>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO event (account_id, event_type) VALUES (?1, 'balance_update')",
        params![account_id],
    )?;
    let event_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO event_data (event_id, amount_minor, event_date, note) VALUES (?1, ?2, ?3, ?4)",
        params![event_id, amount_minor, event_date, note],
    )?;

    Ok(event_id)
}

pub fn create_balance_update(
    conn: &Connection,
    account_id: i64,
    amount_minor: i64,
    event_date: &str,
    note: Option<&str>,
) -> rusqlite::Result<i64> {
    with_savepoint(conn, || {
        create_balance_update_inner(conn, account_id, amount_minor, event_date, note)
    })
}

pub fn bulk_create_balance_updates(
    conn: &Connection,
    entries: &[(i64, i64)],
    event_date: &str,
    note: Option<&str>,
) -> rusqlite::Result<Vec<i64>> {
    with_savepoint(conn, || {
        let mut ids = Vec::with_capacity(entries.len());
        for &(account_id, amount_minor) in entries {
            let event_id =
                create_balance_update_inner(conn, account_id, amount_minor, event_date, note)?;
            let account_type: Option<String> = conn
                .query_row(
                    "SELECT account_type FROM account WHERE id = ?1",
                    params![account_id],
                    |row| row.get(0),
                )
                .optional()?;
            if account_type.as_deref() == Some("bucket") {
                crate::features::buckets::repository::carry_forward_bucket_links(
                    conn, account_id, event_id,
                )?;
            }
            ids.push(event_id);
        }
        Ok(ids)
    })
}

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
                Some(cp_id) => {
                    crate::features::accounts::repository::get_account_type(conn, cp_id)?
                }
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

pub fn update_event(
    conn: &Connection,
    event_id: i64,
    amount_minor: i64,
    event_date: &str,
    note: Option<&str>,
) -> Result<(), String> {
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
            original_amount_minor, fx_rate_mantissa, fx_rate_exponent
        )
        SELECT ?1, ?2, ?3, ?4,
            counterpart_account_id, bucket_id, original_currency_id,
            original_amount_minor, fx_rate_mantissa, fx_rate_exponent
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

pub fn create_split_group_with_legs(
    conn: &Connection,
    account_id: i64,
    group_note: Option<&str>,
    legs: &[SplitGroupEntry],
) -> Result<i64, AppError> {
    with_savepoint_app(conn, || {
        conn.execute(
            "INSERT INTO split_group (note) VALUES (?1)",
            params![group_note],
        )?;
        let split_group_id = conn.last_insert_rowid();

        for leg in legs {
            let counterpart_type = match leg.counterpart_account_id {
                Some(cp_id) => {
                    crate::features::accounts::repository::get_account_type(conn, cp_id)?
                }
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

pub fn update_split_group_date(
    conn: &Connection,
    split_group_id: i64,
    new_date: &str,
) -> Result<(), AppError> {
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

pub fn check_event_split_group_date_conflict(
    conn: &Connection,
    event_id: i64,
    new_date: &str,
) -> Result<(), AppError> {
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

#[derive(Default)]
pub struct ListEventsQuery {
    pub account_id: Option<i64>,
    pub account_ids: Option<Vec<i64>>,
    pub before_date: Option<String>,
    pub from_date: Option<String>,
    pub event_types: Option<Vec<String>>,
    pub limit: Option<i64>,
    pub bucket_ids: Option<Vec<i64>>,
}

pub fn list_events(
    conn: &Connection,
    query: ListEventsQuery,
) -> rusqlite::Result<ListEventsResult> {
    let account_id = query.account_id;
    let account_ids = query.account_ids.as_deref();
    let before_date = query.before_date.as_deref();
    let from_date = query.from_date.as_deref();
    let event_types = query.event_types.as_deref();
    let limit = query.limit;
    let bucket_ids = query.bucket_ids.as_deref();
    let base = "FROM event e
        JOIN account a ON a.id = e.account_id
        JOIN currency c ON c.id = a.currency_id
        JOIN event_data ed ON ed.id = e.latest_data_id
        LEFT JOIN account counter_a ON counter_a.id = ed.counterpart_account_id
        LEFT JOIN account bucket_a ON bucket_a.id = ed.bucket_id
        LEFT JOIN currency orig_c ON orig_c.id = ed.original_currency_id
        LEFT JOIN split_group sg ON sg.id = e.split_group_id
        WHERE e.deleted_at IS NULL";

    let mut where_suffix = String::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    // account_ids multi-select filter: non-empty slice takes precedence over account_id
    let use_multi = account_ids.is_some_and(|ids| !ids.is_empty());
    let use_bucket_ids = bucket_ids.is_some_and(|ids| !ids.is_empty());
    if use_multi && use_bucket_ids {
        // Both filters present: match either account or tagged bucket
        let acct_ids = account_ids.unwrap();
        let bkt_ids = bucket_ids.unwrap();
        let acct_placeholders = std::iter::repeat_n("?", acct_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let bkt_placeholders = std::iter::repeat_n("?", bkt_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        where_suffix.push_str(&format!(
            " AND (e.account_id IN ({acct_placeholders}) OR ed.bucket_id IN ({bkt_placeholders}))"
        ));
        for &id in acct_ids {
            params.push(Box::new(id));
        }
        for &id in bkt_ids {
            params.push(Box::new(id));
        }
    } else if use_multi {
        let ids = account_ids.unwrap();
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        where_suffix.push_str(&format!(" AND e.account_id IN ({placeholders})"));
        for &id in ids {
            params.push(Box::new(id));
        }
    } else if use_bucket_ids {
        let ids = bucket_ids.unwrap();
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        where_suffix.push_str(&format!(" AND ed.bucket_id IN ({placeholders})"));
        for &id in ids {
            params.push(Box::new(id));
        }
    } else if let Some(id) = account_id {
        where_suffix.push_str(" AND e.account_id = ?");
        params.push(Box::new(id));
    }

    if let Some(bd) = before_date {
        where_suffix.push_str(" AND ed.event_date <= ?");
        params.push(Box::new(bd.to_owned()));
    }

    if let Some(fd) = from_date {
        where_suffix.push_str(" AND ed.event_date >= ?");
        params.push(Box::new(fd.to_owned()));
    }

    if let Some(types) = event_types {
        if !types.is_empty() {
            let placeholders = std::iter::repeat_n("?", types.len())
                .collect::<Vec<_>>()
                .join(", ");
            where_suffix.push_str(&format!(" AND e.event_type IN ({placeholders})"));
            for t in types {
                params.push(Box::new(t.to_owned()));
            }
        }
    }

    // Count query: same conditions, no ORDER BY or LIMIT
    let count_sql = format!("SELECT COUNT(*) {}{}", base, where_suffix);
    let total_count: i64 = conn.query_row(
        &count_sql,
        rusqlite::params_from_iter(params.iter()),
        |row| row.get(0),
    )?;

    // Events query: add ORDER BY and optional LIMIT
    let mut sql = format!(
        "SELECT
          e.id,
          e.account_id,
          a.name AS account_name,
          a.account_type,
          e.event_type,
          ed.event_date,
          ed.amount_minor,
          ed.note,
          e.created_at,
          c.code AS currency_code,
          c.minor_units AS currency_minor_units,
          e.linked_event_id,
          ed.counterpart_account_id,
          ed.bucket_id,
          ed.original_currency_id,
          ed.original_amount_minor,
          ed.fx_rate_mantissa,
          ed.fx_rate_exponent,
          counter_a.name AS counterpart_account_name,
          bucket_a.name AS bucket_name,
          orig_c.code AS original_currency_code,
          orig_c.minor_units AS original_currency_minor_units,
          e.split_group_id,
          sg.note AS split_group_note
        {}{}",
        base, where_suffix
    );
    sql.push_str(" ORDER BY ed.event_date DESC, e.created_at DESC");

    if let Some(lim) = limit {
        sql.push_str(" LIMIT ?");
        params.push(Box::new(lim));
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
        Ok(EventWithData {
            id: row.get(0)?,
            account_id: row.get(1)?,
            account_name: row.get(2)?,
            account_type: row.get(3)?,
            event_type: row.get(4)?,
            event_date: row.get(5)?,
            amount_minor: row.get(6)?,
            note: row.get(7)?,
            created_at: row.get(8)?,
            currency_code: row.get(9)?,
            currency_minor_units: row.get(10)?,
            linked_event_id: row.get(11)?,
            counterpart_account_id: row.get(12)?,
            bucket_id: row.get(13)?,
            original_currency_id: row.get(14)?,
            original_amount_minor: row.get(15)?,
            fx_rate_mantissa: row.get(16)?,
            fx_rate_exponent: row.get(17)?,
            counterpart_account_name: row.get(18)?,
            bucket_name: row.get(19)?,
            original_currency_code: row.get(20)?,
            original_currency_minor_units: row.get(21)?,
            split_group_id: row.get(22)?,
            split_group_note: row.get(23)?,
        })
    })?;

    let events = rows.collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(ListEventsResult {
        events,
        total_count,
    })
}

pub fn get_event_by_id(
    conn: &Connection,
    event_id: i64,
) -> rusqlite::Result<Option<EventWithData>> {
    conn.query_row(
        "SELECT
          e.id,
          e.account_id,
          a.name AS account_name,
          a.account_type,
          e.event_type,
          ed.event_date,
          ed.amount_minor,
          ed.note,
          e.created_at,
          c.code AS currency_code,
          c.minor_units AS currency_minor_units,
          e.linked_event_id,
          ed.counterpart_account_id,
          ed.bucket_id,
          ed.original_currency_id,
          ed.original_amount_minor,
          ed.fx_rate_mantissa,
          ed.fx_rate_exponent,
          counter_a.name AS counterpart_account_name,
          bucket_a.name AS bucket_name,
          orig_c.code AS original_currency_code,
          orig_c.minor_units AS original_currency_minor_units,
          e.split_group_id,
          sg.note AS split_group_note
        FROM event e
        JOIN account a ON a.id = e.account_id
        JOIN currency c ON c.id = a.currency_id
        JOIN event_data ed ON ed.id = e.latest_data_id
        LEFT JOIN account counter_a ON counter_a.id = ed.counterpart_account_id
        LEFT JOIN account bucket_a ON bucket_a.id = ed.bucket_id
        LEFT JOIN currency orig_c ON orig_c.id = ed.original_currency_id
        LEFT JOIN split_group sg ON sg.id = e.split_group_id
        WHERE e.deleted_at IS NULL AND e.id = ?1",
        params![event_id],
        |row| {
            Ok(EventWithData {
                id: row.get(0)?,
                account_id: row.get(1)?,
                account_name: row.get(2)?,
                account_type: row.get(3)?,
                event_type: row.get(4)?,
                event_date: row.get(5)?,
                amount_minor: row.get(6)?,
                note: row.get(7)?,
                created_at: row.get(8)?,
                currency_code: row.get(9)?,
                currency_minor_units: row.get(10)?,
                linked_event_id: row.get(11)?,
                counterpart_account_id: row.get(12)?,
                bucket_id: row.get(13)?,
                original_currency_id: row.get(14)?,
                original_amount_minor: row.get(15)?,
                fx_rate_mantissa: row.get(16)?,
                fx_rate_exponent: row.get(17)?,
                counterpart_account_name: row.get(18)?,
                bucket_name: row.get(19)?,
                original_currency_code: row.get(20)?,
                original_currency_minor_units: row.get(21)?,
                split_group_id: row.get(22)?,
                split_group_note: row.get(23)?,
            })
        },
    )
    .optional()
}

pub fn get_accounts_snapshot(
    conn: &Connection,
    selected_datetime: &str,
) -> rusqlite::Result<Vec<SnapshotRow>> {
    let consolidation = get_consolidation_currency(conn)?;
    // Extract YYYY-MM-DD from datetime string for fx_rate date comparison.
    let snapshot_date = &selected_datetime[..10.min(selected_datetime.len())];

    let mut stmt = conn.prepare(
        "SELECT
           a.id AS account_id,
           a.name AS account_name,
           a.account_type,
           a.iban,
           c.id AS currency_id,
           c.code AS currency_code,
           c.minor_units AS currency_minor_units,
           c.is_custom AS currency_is_custom,
           COALESCE(
             (SELECT ed.amount_minor
              FROM event e
              JOIN event_data ed ON ed.id = e.latest_data_id
              WHERE e.account_id = a.id
                AND e.deleted_at IS NULL
                AND e.event_type = 'balance_update'
                AND ed.event_date <= ?1
              ORDER BY ed.event_date DESC, e.created_at DESC
              LIMIT 1),
             0
           ) + COALESCE(
             (SELECT SUM(ed2.amount_minor)
              FROM event e2
              JOIN event_data ed2 ON ed2.id = e2.latest_data_id
              WHERE e2.account_id = a.id
                AND e2.deleted_at IS NULL
                AND e2.event_type IN ('cashflow', 'transfer')
                AND ed2.event_date <= ?1
                AND ed2.event_date > COALESCE(
                  (SELECT ed3.event_date
                   FROM event e3
                   JOIN event_data ed3 ON ed3.id = e3.latest_data_id
                   WHERE e3.account_id = a.id
                     AND e3.deleted_at IS NULL
                     AND e3.event_type = 'balance_update'
                     AND ed3.event_date <= ?1
                   ORDER BY ed3.event_date DESC, e3.created_at DESC
                   LIMIT 1),
                  ''
                )),
             0
           ) AS balance_minor
         FROM account a
         JOIN currency c ON c.id = a.currency_id
         WHERE a.account_type IN ('account', 'bucket', 'asset')
         ORDER BY a.account_type, a.sort_order, a.id",
    )?;

    let row_data: Vec<SnapshotRawRow> = stmt
        .query_map(params![selected_datetime], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut result = Vec::with_capacity(row_data.len());
    for (
        account_id,
        account_name,
        account_type,
        iban,
        currency_id,
        currency_code,
        currency_minor_units,
        currency_is_custom,
        balance_minor,
    ) in row_data
    {
        let (converted_balance_minor, fx_rate_missing) = if currency_id == consolidation.id {
            (balance_minor, false)
        } else {
            match get_fx_rate_for_conversion(conn, consolidation.id, currency_id, snapshot_date)? {
                Some((mantissa, exponent, is_direct)) => {
                    let converted = convert_balance(
                        balance_minor,
                        mantissa,
                        exponent,
                        currency_minor_units,
                        consolidation.minor_units,
                        is_direct,
                    );
                    (converted, false)
                }
                None => {
                    // 1:1 fallback: mantissa=1, exponent=0
                    let converted = convert_balance(
                        balance_minor,
                        1,
                        0,
                        currency_minor_units,
                        consolidation.minor_units,
                        false,
                    );
                    (converted, true)
                }
            }
        };

        result.push(SnapshotRow {
            account_id,
            account_name,
            account_type,
            iban,
            balance_minor,
            currency_code,
            currency_minor_units,
            is_custom: currency_is_custom != 0,
            converted_balance_minor,
            fx_rate_missing,
            is_linked_to_asset: false,
            linked_asset_ids: vec![],
            is_bucket_linked: false,
            bucket_links: vec![],
            linked_balance_minor: 0,
            cashflow_tagged_minor: 0,
        });
    }

    // Populate asset-link fields using a single bulk query to avoid N+1.
    let (linked_account_ids_set, account_to_assets, asset_to_accounts) =
        get_all_account_asset_link_ids(conn)?;
    for row in &mut result {
        if row.account_type == "account" {
            if linked_account_ids_set.contains(&row.account_id) {
                row.is_linked_to_asset = true;
                if let Some(asset_ids) = account_to_assets.get(&row.account_id) {
                    row.linked_asset_ids = asset_ids.clone();
                }
            }
        } else if row.account_type == "asset" {
            if let Some(account_ids) = asset_to_accounts.get(&row.account_id) {
                row.linked_asset_ids = account_ids.clone();
            }
        }
    }

    // Pass 2: Event-bound bucket links
    let all_bucket_links = list_all_latest_bucket_links(conn, snapshot_date)?;
    let account_index_map: HashMap<i64, usize> = result
        .iter()
        .enumerate()
        .map(|(idx, row)| (row.account_id, idx))
        .collect();

    for (bucket_id, link) in &all_bucket_links {
        let bucket_idx = match account_index_map.get(bucket_id) {
            Some(&idx) => idx,
            None => continue,
        };
        let source_idx = match account_index_map.get(&link.source_account_id) {
            Some(&idx) => idx,
            None => continue,
        };

        let source_balance = result[source_idx].balance_minor;
        let source_currency_id = link.source_currency_id;
        let source_minor_units = link.source_currency_minor_units;

        let converted = if source_currency_id == consolidation.id {
            source_balance
        } else {
            match get_fx_rate_for_conversion(
                conn,
                consolidation.id,
                source_currency_id,
                snapshot_date,
            )? {
                Some((mantissa, exponent, is_direct)) => convert_balance(
                    source_balance,
                    mantissa,
                    exponent,
                    source_minor_units,
                    consolidation.minor_units,
                    is_direct,
                ),
                None => {
                    result[bucket_idx].fx_rate_missing = true;
                    convert_balance(
                        source_balance,
                        1,
                        0,
                        source_minor_units,
                        consolidation.minor_units,
                        false,
                    )
                }
            }
        };

        result[bucket_idx].bucket_links.push(link.clone());
        result[bucket_idx].linked_balance_minor += converted;
        result[bucket_idx].converted_balance_minor += converted;
        result[source_idx].is_bucket_linked = true;
    }

    // Pass 3: Cashflow-tagged amounts per bucket.
    // Aggregate cashflows where event_data.bucket_id is set, grouped by bucket and source
    // currency. This lets the bucket card show how much of its balance comes from tagged
    // cashflow entries (e.g. CSV-imported rows).
    {
        let mut stmt = conn.prepare(
            "SELECT
               ed.bucket_id AS bucket_account_id,
               a.currency_id,
               c.minor_units,
               SUM(ed.amount_minor) AS tagged_total
             FROM event e
             JOIN event_data ed ON ed.id = e.latest_data_id
             JOIN account a ON a.id = e.account_id
             JOIN currency c ON c.id = a.currency_id
             WHERE e.deleted_at IS NULL
               AND ed.bucket_id IS NOT NULL
               AND ed.event_date <= ?1
             GROUP BY ed.bucket_id, a.currency_id",
        )?;

        let tagged_rows: Vec<(i64, i64, i64, i64)> = stmt
            .query_map(params![selected_datetime], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for (bucket_account_id, currency_id, minor_units, tagged_total) in tagged_rows {
            let bucket_idx = match account_index_map.get(&bucket_account_id) {
                Some(&idx) => idx,
                None => continue,
            };

            let converted = if currency_id == consolidation.id {
                tagged_total
            } else {
                match get_fx_rate_for_conversion(
                    conn,
                    consolidation.id,
                    currency_id,
                    snapshot_date,
                )? {
                    Some((mantissa, exponent, is_direct)) => convert_balance(
                        tagged_total,
                        mantissa,
                        exponent,
                        minor_units,
                        consolidation.minor_units,
                        is_direct,
                    ),
                    None => {
                        result[bucket_idx].fx_rate_missing = true;
                        convert_balance(
                            tagged_total,
                            1,
                            0,
                            minor_units,
                            consolidation.minor_units,
                            false,
                        )
                    }
                }
            };

            // TODO: This double-counts if the same source account is also contributing via
            // bucket_event_link (Pass 2). The UI wizard should prevent this scenario.
            result[bucket_idx].converted_balance_minor += converted;
            result[bucket_idx].cashflow_tagged_minor += converted;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::create_account;
    use crate::features::currency::repository::set_fx_rate_manual;

    fn mk_account(conn: &Connection) -> i64 {
        create_account(conn, "Test Account", 1, "account", None, None, None)
            .expect("create account failed")
    }

    #[test]
    fn snapshot_with_no_events_returns_zero() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].account_id, account_id);
        assert_eq!(snapshot[0].balance_minor, 0);
    }

    #[test]
    fn snapshot_reflects_balance_update() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 5000, "2026-03-01", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 5000);
    }

    #[test]
    fn snapshot_ignores_future_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 5000, "2026-06-01", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 0);
    }

    #[test]
    fn snapshot_uses_latest_event_by_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 3000, "2026-01-01", None).unwrap();
        create_balance_update(&conn, account_id, 7000, "2026-02-01", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 7000);
    }

    #[test]
    fn snapshot_ignores_soft_deleted_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let event_id = create_balance_update(&conn, account_id, 5000, "2026-03-01", None).unwrap();
        delete_event(&conn, event_id).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 0);
    }

    #[test]
    fn update_event_creates_new_data_row() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let event_id = create_balance_update(&conn, account_id, 5000, "2026-03-01", None).unwrap();
        update_event(&conn, event_id, 9999, "2026-03-01", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 9999);
    }

    #[test]
    fn update_event_rejects_deleted_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let event_id = create_balance_update(&conn, account_id, 5000, "2026-03-01", None).unwrap();
        delete_event(&conn, event_id).unwrap();
        let result = update_event(&conn, event_id, 9999, "2026-03-01", None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Cannot update a deleted event"));
    }

    #[test]
    fn update_event_rejects_nonexistent_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let result = update_event(&conn, 999, 9999, "2026-03-01", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Event not found"));
    }

    #[test]
    fn list_events_returns_all_non_deleted() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 1000, "2026-01-01", None).unwrap();
        create_balance_update(&conn, account_id, 2000, "2026-02-01", None).unwrap();
        let result = list_events(&conn, ListEventsQuery::default()).unwrap();
        assert_eq!(result.events.len(), 2);
    }

    #[test]
    fn list_events_filters_by_account() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(&conn, "Second", 1, "account", None, None, None).unwrap();
        create_balance_update(&conn, acc1, 1000, "2026-01-01", None).unwrap();
        create_balance_update(&conn, acc2, 2000, "2026-02-01", None).unwrap();

        let result_acc1 = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(acc1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result_acc1.events.len(), 1);
        assert_eq!(result_acc1.events[0].account_id, acc1);

        let result_acc2 = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(acc2),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result_acc2.events.len(), 1);
        assert_eq!(result_acc2.events[0].account_id, acc2);
    }

    #[test]
    fn list_events_filters_by_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 1000, "2026-01-15", None).unwrap();
        create_balance_update(&conn, account_id, 2000, "2026-03-15", None).unwrap();
        let result = list_events(
            &conn,
            ListEventsQuery {
                before_date: Some("2026-02-01T23:59:59".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].amount_minor, 1000);
    }

    #[test]
    fn create_bucket_appears_in_snapshot() {
        let conn = initialize_in_memory().expect("DB init failed");
        let bucket_id = create_account(
            &conn,
            "Emergency Fund",
            1,
            "bucket",
            Some(20000),
            None,
            None,
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        let bucket = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();
        assert_eq!(bucket.account_type, "bucket");
        assert_eq!(bucket.balance_minor, 20000);
    }

    #[test]
    fn snapshot_returns_account_type() {
        let conn = initialize_in_memory().expect("DB init failed");
        mk_account(&conn);
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].account_type, "account");
    }

    #[test]
    fn bucket_balance_update_works() {
        let conn = initialize_in_memory().expect("DB init failed");
        let bucket_id =
            create_account(&conn, "Savings Bucket", 1, "bucket", None, None, None).unwrap();
        create_balance_update(&conn, bucket_id, 15000, "2026-03-01", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        let bucket = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();
        assert_eq!(bucket.balance_minor, 15000);
    }

    #[test]
    fn list_events_includes_account_type() {
        let conn = initialize_in_memory().expect("DB init failed");
        let bucket_id =
            create_account(&conn, "Test Bucket", 1, "bucket", None, None, None).unwrap();
        create_balance_update(&conn, bucket_id, 5000, "2026-03-01", None).unwrap();
        let result = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(bucket_id),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].account_type, "bucket");

        let empty_account_id =
            create_account(&conn, "Empty Account", 1, "account", None, None, None).unwrap();
        let result_empty = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(empty_account_id),
                ..Default::default()
            },
        )
        .unwrap();
        // Account with no balance updates has no events
        assert_eq!(result_empty.events.len(), 0);
    }

    #[test]
    fn snapshot_orders_accounts_before_buckets() {
        let conn = initialize_in_memory().expect("DB init failed");
        create_account(&conn, "Zebra Bucket", 1, "bucket", None, None, None).unwrap();
        create_account(&conn, "Alpha Account", 1, "account", None, None, None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        // accounts come first (alphabetically 'account' < 'bucket'), then buckets
        let types: Vec<&str> = snapshot.iter().map(|r| r.account_type.as_str()).collect();
        let first_bucket_idx = types.iter().position(|t| *t == "bucket");
        let last_account_idx = types.iter().rposition(|t| *t == "account");
        if let (Some(fb), Some(la)) = (first_bucket_idx, last_account_idx) {
            assert!(la < fb, "All accounts should come before all buckets");
        }
    }

    #[test]
    fn snapshot_includes_currency_fields() {
        let conn = initialize_in_memory().expect("DB init failed");
        mk_account(&conn);
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        assert_eq!(snapshot[0].currency_code, "EUR");
        assert_eq!(snapshot[0].currency_minor_units, 2);
        // EUR is the consolidation currency so converted == balance and no rate missing
        assert_eq!(
            snapshot[0].converted_balance_minor,
            snapshot[0].balance_minor
        );
        assert!(!snapshot[0].fx_rate_missing);
    }

    #[test]
    fn snapshot_foreign_currency_no_rate_uses_1_to_1_fallback() {
        let conn = initialize_in_memory().expect("DB init failed");
        // USD is seeded in migration 004
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let acc = create_account(&conn, "USD Account", usd, "account", None, None, None).unwrap();
        create_balance_update(&conn, acc, 108420, "2026-03-01", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        let row = snapshot.iter().find(|r| r.account_id == acc).unwrap();
        assert_eq!(row.balance_minor, 108420);
        // No FX rate → 1:1 fallback → converted = same value (minor_units both 2)
        assert_eq!(row.converted_balance_minor, 108420);
        assert!(row.fx_rate_missing);
    }

    #[test]
    fn snapshot_foreign_currency_with_rate_converts_correctly() {
        let conn = initialize_in_memory().expect("DB init failed");
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let acc = create_account(&conn, "USD Account", usd, "account", None, None, None).unwrap();
        create_balance_update(&conn, acc, 108420, "2026-03-01", None).unwrap();
        // Store rate: 1 EUR = 1.0842 USD (mantissa=10842, exponent=-4)
        set_fx_rate_manual(&conn, eur, usd, "2026-03-01", 10842, -4, false).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        let row = snapshot.iter().find(|r| r.account_id == acc).unwrap();
        assert_eq!(row.balance_minor, 108420);
        assert_eq!(row.converted_balance_minor, 100000); // 1084.20 USD → 1000.00 EUR
        assert!(!row.fx_rate_missing);
    }

    #[test]
    fn list_events_includes_currency_fields() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 5000, "2026-03-01", None).unwrap();
        let result = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(account_id),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.events[0].currency_code, "EUR");
        assert_eq!(result.events[0].currency_minor_units, 2);
    }

    #[test]
    fn list_events_returns_correct_total_count_with_limit() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 1000, "2026-01-01", None).unwrap();
        create_balance_update(&conn, account_id, 2000, "2026-02-01", None).unwrap();
        create_balance_update(&conn, account_id, 3000, "2026-03-01", None).unwrap();
        create_balance_update(&conn, account_id, 4000, "2026-04-01", None).unwrap();
        create_balance_update(&conn, account_id, 5000, "2026-05-01", None).unwrap();
        let result = list_events(
            &conn,
            ListEventsQuery {
                limit: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.total_count, 5);
    }

    #[test]
    fn snapshot_cashflow_after_balance_update_is_summed() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 5000, "2026-03-01T10:00:00", None).unwrap();
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id,
                amount_minor: 200,
                event_date: "2026-03-01T14:30:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-31T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 5200);
    }

    #[test]
    fn snapshot_cashflow_before_balance_update_not_counted() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id,
                amount_minor: 500,
                event_date: "2026-01-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        create_balance_update(&conn, account_id, 5000, "2026-03-01T00:00:00", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-12-31T23:59:59").unwrap();
        // The cashflow at 2026-01-01 is before the anchor; only the anchor balance counts.
        assert_eq!(snapshot[0].balance_minor, 5000);
    }

    #[test]
    fn snapshot_with_no_balance_update_sums_all_cashflows() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id,
                amount_minor: 1000,
                event_date: "2026-01-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id,
                amount_minor: 500,
                event_date: "2026-02-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-12-31T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 1500);
    }

    #[test]
    fn create_transfer_creates_linked_pair() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(&conn, "Second Account", 1, "account", None, None, None).unwrap();
        let (source_id, counterpart_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -10000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc2),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let source_linked: Option<i64> = conn
            .query_row(
                "SELECT linked_event_id FROM event WHERE id = ?1",
                params![source_id],
                |r| r.get(0),
            )
            .unwrap();
        let counterpart_linked: Option<i64> = conn
            .query_row(
                "SELECT linked_event_id FROM event WHERE id = ?1",
                params![counterpart_id],
                |r| r.get(0),
            )
            .unwrap();

        assert_eq!(source_linked, Some(counterpart_id));
        assert_eq!(counterpart_linked, Some(source_id));

        let snapshot1 = get_accounts_snapshot(&conn, "2026-12-31T23:59:59").unwrap();
        let row1 = snapshot1.iter().find(|r| r.account_id == acc1).unwrap();
        let row2 = snapshot1.iter().find(|r| r.account_id == acc2).unwrap();
        assert_eq!(row1.balance_minor, -10000);
        assert_eq!(row2.balance_minor, 10000);
    }

    #[test]
    fn delete_transfer_also_deletes_counterpart() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(&conn, "Second Account", 1, "account", None, None, None).unwrap();
        let (source_id, counterpart_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -10000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc2),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let deleted = delete_event(&conn, source_id).unwrap();
        assert_eq!(deleted.len(), 2);
        assert!(deleted.contains(&source_id));
        assert!(deleted.contains(&counterpart_id));

        let source_deleted_at: Option<String> = conn
            .query_row(
                "SELECT deleted_at FROM event WHERE id = ?1",
                params![source_id],
                |r| r.get(0),
            )
            .unwrap();
        let counterpart_deleted_at: Option<String> = conn
            .query_row(
                "SELECT deleted_at FROM event WHERE id = ?1",
                params![counterpart_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(source_deleted_at.is_some());
        assert!(counterpart_deleted_at.is_some());
    }

    #[test]
    fn snapshot_transfer_affects_both_accounts() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc_a = mk_account(&conn);
        let acc_b = create_account(&conn, "Account B", 1, "account", None, None, None).unwrap();
        create_balance_update(&conn, acc_a, 10000, "2026-01-10T09:00:00", None).unwrap();
        create_balance_update(&conn, acc_b, 5000, "2026-01-10T09:00:00", None).unwrap();
        create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc_a,
                amount_minor: -3000,
                event_date: "2026-01-15T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc_b),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-12-31T23:59:59").unwrap();
        let row_a = snapshot.iter().find(|r| r.account_id == acc_a).unwrap();
        let row_b = snapshot.iter().find(|r| r.account_id == acc_b).unwrap();
        assert_eq!(row_a.balance_minor, 7000);
        assert_eq!(row_b.balance_minor, 8000);
    }

    #[test]
    fn delete_transfer_restores_snapshot_balances() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc_a = mk_account(&conn);
        let acc_b = create_account(&conn, "Account B", 1, "account", None, None, None).unwrap();
        create_balance_update(&conn, acc_a, 10000, "2026-01-10T09:00:00", None).unwrap();
        create_balance_update(&conn, acc_b, 5000, "2026-01-10T09:00:00", None).unwrap();
        let (source_id, _) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc_a,
                amount_minor: -3000,
                event_date: "2026-01-15T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc_b),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        delete_event(&conn, source_id).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-12-31T23:59:59").unwrap();
        let row_a = snapshot.iter().find(|r| r.account_id == acc_a).unwrap();
        let row_b = snapshot.iter().find(|r| r.account_id == acc_b).unwrap();
        assert_eq!(row_a.balance_minor, 10000);
        assert_eq!(row_b.balance_minor, 5000);
    }

    #[test]
    fn snapshot_ignores_soft_deleted_cashflows() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 10000, "2026-03-01T10:00:00", None).unwrap();
        let cashflow_id = create_cashflow(
            &conn,
            &CashflowEntry {
                account_id,
                amount_minor: -2000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        delete_event(&conn, cashflow_id).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-31T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 10000);
    }

    #[test]
    fn update_transfer_same_currency() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(&conn, "Second Account", 1, "account", None, None, None).unwrap();

        let (from_id, to_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -5000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: Some("original note".to_string()),
                counterpart_account_id: Some(acc2),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        update_transfer(
            &conn,
            UpdateTransferParams {
                from_event_id: from_id,
                to_event_id: to_id,
                from_date: "2026-04-01T12:00:00".into(),
                to_date: "2026-04-01T12:00:00".into(),
                from_amount_minor: -8000,
                to_amount_minor: 8000,
                note: Some("updated note".into()),
                original_currency_id: None,
                original_amount_minor_for_from_leg: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let from_updated = get_event_by_id(&conn, from_id).unwrap().unwrap();
        assert_eq!(from_updated.amount_minor, -8000);
        assert_eq!(from_updated.event_date, "2026-04-01T12:00:00");
        assert_eq!(from_updated.note.as_deref(), Some("updated note"));
        assert_eq!(from_updated.counterpart_account_id, Some(acc2));

        let to_updated = get_event_by_id(&conn, to_id).unwrap().unwrap();
        assert_eq!(to_updated.amount_minor, 8000);
        assert_eq!(to_updated.event_date, "2026-04-01T12:00:00");
        assert_eq!(to_updated.note.as_deref(), Some("updated note"));

        let from_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event_data WHERE event_id = ?1",
                params![from_id],
                |r| r.get(0),
            )
            .unwrap();
        let to_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event_data WHERE event_id = ?1",
                params![to_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(from_count, 2);
        assert_eq!(to_count, 2);
    }

    #[test]
    fn update_transfer_cross_currency() {
        let conn = initialize_in_memory().expect("DB init failed");
        let usd_id: i64 = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get(0)
            })
            .unwrap();

        let acc_eur = mk_account(&conn);
        let acc_usd =
            create_account(&conn, "USD Account", usd_id, "account", None, None, None).unwrap();

        let (from_id, to_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc_eur,
                amount_minor: -10000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc_usd),
                bucket_id: None,
                original_currency_id: Some(usd_id),
                original_amount_minor: Some(10845),
                fx_rate_mantissa: Some(10845),
                fx_rate_exponent: Some(-4),
            },
        )
        .unwrap();

        let new_to_amount: i64 = 11000;

        update_transfer(
            &conn,
            UpdateTransferParams {
                from_event_id: from_id,
                to_event_id: to_id,
                from_date: "2026-04-01T12:00:00".into(),
                to_date: "2026-04-01T12:00:00".into(),
                from_amount_minor: -10200,
                to_amount_minor: new_to_amount,
                note: Some("cross-currency updated".into()),
                original_currency_id: Some(usd_id),
                original_amount_minor_for_from_leg: Some(new_to_amount),
                fx_rate_mantissa: Some(10784),
                fx_rate_exponent: Some(-4),
            },
        )
        .unwrap();

        let from_updated = get_event_by_id(&conn, from_id).unwrap().unwrap();
        assert_eq!(from_updated.original_currency_id, Some(usd_id));
        assert_eq!(from_updated.original_amount_minor, Some(new_to_amount));
        assert_eq!(from_updated.fx_rate_mantissa, Some(10784));
        assert_eq!(from_updated.fx_rate_exponent, Some(-4));

        let to_updated = get_event_by_id(&conn, to_id).unwrap().unwrap();
        assert_eq!(to_updated.original_currency_id, None);
        assert_eq!(to_updated.original_amount_minor, None);
        assert_eq!(to_updated.fx_rate_mantissa, None);
        assert_eq!(to_updated.fx_rate_exponent, None);
    }

    #[test]
    fn update_transfer_validation_not_linked() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(&conn, "Second Account", 1, "account", None, None, None).unwrap();
        let acc3 = create_account(&conn, "Third Account", 1, "account", None, None, None).unwrap();

        let (pair1_from, _) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -1000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc2),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let (pair2_from, _) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -2000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc3),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        // pair1_from is linked to pair1_to, not pair2_from — validation should reject this
        let result = update_transfer(
            &conn,
            UpdateTransferParams {
                from_event_id: pair1_from,
                to_event_id: pair2_from,
                from_date: "2026-04-01T12:00:00".into(),
                to_date: "2026-04-01T12:00:00".into(),
                from_amount_minor: -1000,
                to_amount_minor: 1000,
                note: None,
                original_currency_id: None,
                original_amount_minor_for_from_leg: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "VALIDATION");
    }

    #[test]
    fn update_transfer_soft_deleted_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(&conn, "Second Account", 1, "account", None, None, None).unwrap();

        let (from_id, to_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -5000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc2),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let now = crate::shared::local_now();
        conn.execute(
            "UPDATE event SET deleted_at = ?1 WHERE id = ?2",
            params![now, to_id],
        )
        .unwrap();

        let result = update_transfer(
            &conn,
            UpdateTransferParams {
                from_event_id: from_id,
                to_event_id: to_id,
                from_date: "2026-04-01T12:00:00".into(),
                to_date: "2026-04-01T12:00:00".into(),
                from_amount_minor: -5000,
                to_amount_minor: 5000,
                note: None,
                original_currency_id: None,
                original_amount_minor_for_from_leg: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "VALIDATION");
    }

    #[test]
    fn get_event_by_id_returns_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let event_id = create_balance_update(&conn, account_id, 7500, "2026-03-01", None).unwrap();

        let maybe_event = get_event_by_id(&conn, event_id).unwrap();
        assert!(maybe_event.is_some());
        let event = maybe_event.unwrap();
        assert_eq!(event.id, event_id);
        assert_eq!(event.event_type, "balance_update");
        assert_eq!(event.amount_minor, 7500);

        let none = get_event_by_id(&conn, 999999).unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn cashflow_with_bucket_id_contributes_to_bucket_snapshot() {
        let conn = initialize_in_memory().expect("DB init failed");

        let source_id = mk_account(&conn);
        let bucket_id =
            create_account(&conn, "Tagged Bucket", 1, "bucket", None, None, None).unwrap();

        // Cashflow on source account tagged to bucket
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id: source_id,
                amount_minor: 3000,
                event_date: "2026-06-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: Some(bucket_id),
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let snapshot = get_accounts_snapshot(&conn, "2026-12-31T23:59:59").unwrap();
        let bucket_row = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();

        // EUR is the consolidation currency → direct 1:1 conversion
        assert_eq!(bucket_row.cashflow_tagged_minor, 3000);
        assert_eq!(bucket_row.converted_balance_minor, 3000);
    }

    #[test]
    fn cashflow_after_snapshot_date_not_counted_in_bucket() {
        let conn = initialize_in_memory().expect("DB init failed");

        let source_id = mk_account(&conn);
        let bucket_id =
            create_account(&conn, "Tagged Bucket", 1, "bucket", None, None, None).unwrap();

        // Cashflow dated AFTER the snapshot date
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id: source_id,
                amount_minor: 5000,
                event_date: "2026-12-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: Some(bucket_id),
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        // Snapshot is for a date BEFORE the cashflow
        let snapshot = get_accounts_snapshot(&conn, "2026-06-30T23:59:59").unwrap();
        let bucket_row = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();

        assert_eq!(bucket_row.cashflow_tagged_minor, 0);
        assert_eq!(bucket_row.converted_balance_minor, 0);
    }

    #[test]
    fn list_events_filters_by_bucket_id() {
        let conn = initialize_in_memory().expect("DB init failed");

        let source_id = mk_account(&conn);
        let bucket_id =
            create_account(&conn, "Some Bucket", 1, "bucket", None, None, None).unwrap();

        // Cashflow on source account tagged to bucket
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id: source_id,
                amount_minor: 2500,
                event_date: "2026-04-01T10:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: Some(bucket_id),
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        // Another cashflow on source account with no bucket tag
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id: source_id,
                amount_minor: 1000,
                event_date: "2026-04-02T10:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        // Filter by bucket_id only — should return only the tagged cashflow even though
        // event.account_id is the source account (not the bucket)
        let result = list_events(
            &conn,
            ListEventsQuery {
                bucket_ids: Some(vec![bucket_id]),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].account_id, source_id);
        assert_eq!(result.events[0].bucket_id, Some(bucket_id));
        assert_eq!(result.events[0].amount_minor, 2500);
    }
}
