use crate::error::AppError;
use crate::models::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};

/// Execute `f` inside a SAVEPOINT. Rolls back on error, releases on success.
/// Works with &Connection (no &mut needed).
fn with_savepoint<T, F>(conn: &Connection, f: F) -> rusqlite::Result<T>
where
    F: FnOnce() -> rusqlite::Result<T>,
{
    conn.execute_batch("SAVEPOINT repo_sp")?;
    match f() {
        Ok(val) => {
            conn.execute_batch("RELEASE SAVEPOINT repo_sp")?;
            Ok(val)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT repo_sp");
            let _ = conn.execute_batch("RELEASE SAVEPOINT repo_sp");
            Err(e)
        }
    }
}

/// Same as `with_savepoint` but for closures that return `Result<T, AppError>`.
fn with_savepoint_app<T, F>(conn: &Connection, f: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError>,
{
    conn.execute_batch("SAVEPOINT repo_sp")
        .map_err(AppError::from)?;
    match f() {
        Ok(val) => {
            conn.execute_batch("RELEASE SAVEPOINT repo_sp")
                .map_err(AppError::from)?;
            Ok(val)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT repo_sp");
            let _ = conn.execute_batch("RELEASE SAVEPOINT repo_sp");
            Err(e)
        }
    }
}

pub fn create_account(
    conn: &Connection,
    name: &str,
    currency_id: i64,
    account_type: &str,
    initial_balance_minor: Option<i64>,
    price_per_unit: Option<&str>,
) -> Result<i64, AppError> {
    with_savepoint_app(conn, || {
        let next_sort_order: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM account WHERE account_type = ?1",
            params![account_type],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO account (name, currency_id, account_type, sort_order) VALUES (?1, ?2, ?3, ?4)",
            params![name, currency_id, account_type, next_sort_order],
        )?;
        let account_id = conn.last_insert_rowid();

        if let Some(amount) = initial_balance_minor {
            let now = local_now();
            create_balance_update_inner(conn, account_id, amount, &now, None)?;
        }

        if let Some(price) = price_per_unit {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            store_asset_price(conn, account_id, price, &today)?;
        }

        Ok(account_id)
    })
}

pub fn update_sort_order(conn: &Connection, updates: &[(i64, i64)]) -> rusqlite::Result<()> {
    with_savepoint(conn, || {
        for &(account_id, sort_order) in updates {
            let rows = conn.execute(
                "UPDATE account SET sort_order = ?1 WHERE id = ?2",
                params![sort_order, account_id],
            )?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
        }
        Ok(())
    })
}

pub fn update_account(conn: &Connection, account_id: i64, name: &str) -> rusqlite::Result<()> {
    let rows = conn.execute(
        "UPDATE account SET name = ?1 WHERE id = ?2",
        params![name, account_id],
    )?;
    if rows == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn delete_account(conn: &Connection, account_id: i64) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        // Save currency_id before deleting (needed for orphaned custom unit cleanup).
        let currency_id: i64 = conn
            .query_row(
                "SELECT currency_id FROM account WHERE id = ?1",
                params![account_id],
                |row| row.get(0),
            )
            .map_err(AppError::from)?;

        // Delete ALL allocation rows for pairs where the latest row (by effective_date DESC,
        // id DESC) has amount_minor = 0 — these are truly unlinked. Using only
        // `amount_minor = 0` would miss the earlier positive-amount rows for the same pair,
        // which would then be seen as active and block deletion.
        conn.execute(
            "DELETE FROM bucket_allocation
             WHERE source_account_id = ?1
               AND bucket_id IN (
                 SELECT ba.bucket_id
                 FROM bucket_allocation ba
                 WHERE ba.source_account_id = ?1
                   AND ba.id = (
                     SELECT id FROM bucket_allocation ba2
                     WHERE ba2.bucket_id = ba.bucket_id
                       AND ba2.source_account_id = ba.source_account_id
                     ORDER BY ba2.effective_date DESC, ba2.id DESC
                     LIMIT 1
                   )
                   AND ba.amount_minor = 0
               )",
            params![account_id],
        )
        .map_err(AppError::from)?;

        // Check whether any active allocations remain — i.e. pairs whose latest row has
        // amount_minor != 0. If so, return a descriptive error. Uses the same
        // latest-row-per-pair logic as list_bucket_allocations.
        let mut check_stmt = conn
            .prepare(
                "SELECT DISTINCT a.name
                 FROM bucket_allocation ba
                 JOIN account a ON a.id = ba.bucket_id
                 WHERE ba.source_account_id = ?1
                   AND ba.id = (
                     SELECT id FROM bucket_allocation ba2
                     WHERE ba2.bucket_id = ba.bucket_id
                       AND ba2.source_account_id = ba.source_account_id
                     ORDER BY ba2.effective_date DESC, ba2.id DESC
                     LIMIT 1
                   )
                   AND ba.amount_minor != 0",
            )
            .map_err(AppError::from)?;
        let active_bucket_names: Vec<String> = check_stmt
            .query_map(params![account_id], |row| row.get(0))
            .map_err(AppError::from)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(AppError::from)?;
        if !active_bucket_names.is_empty() {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: format!(
                    "Cannot delete account: it has active allocations in buckets: {}. Unlink them first.",
                    active_bucket_names.join(", ")
                ),
            });
        }

        // Delete all events for this account.
        // ON DELETE CASCADE on event_data.event_id removes event_data rows.
        // DEFERRABLE FK on event.latest_data_id is checked at savepoint release.
        conn.execute(
            "DELETE FROM event WHERE account_id = ?1",
            params![account_id],
        )
        .map_err(AppError::from)?;

        match conn.execute("DELETE FROM account WHERE id = ?1", params![account_id]) {
            Ok(0) => return Err(AppError::from(rusqlite::Error::QueryReturnedNoRows)),
            Ok(_) => {}
            Err(e) => return Err(AppError::from(e)),
        }

        // Step 23: orphaned custom unit cleanup.
        // If the deleted account's currency is a custom unit and no other account
        // references it, delete its fx_rate rows and then the currency itself.
        let is_custom: i64 = conn
            .query_row(
                "SELECT COALESCE((SELECT is_custom FROM currency WHERE id = ?1), 0)",
                params![currency_id],
                |row| row.get(0),
            )
            .map_err(AppError::from)?;

        if is_custom != 0 {
            let other_refs: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM account WHERE currency_id = ?1",
                    params![currency_id],
                    |row| row.get(0),
                )
                .map_err(AppError::from)?;

            if other_refs == 0 {
                conn.execute(
                    "DELETE FROM fx_rate WHERE from_currency_id = ?1 OR to_currency_id = ?1",
                    params![currency_id],
                )
                .map_err(AppError::from)?;
                conn.execute("DELETE FROM currency WHERE id = ?1", params![currency_id])
                    .map_err(AppError::from)?;
            }
        }

        Ok(())
    })
}

fn create_balance_update_inner(
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
            ids.push(event_id);
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
        "INSERT INTO event_data (event_id, amount_minor, event_date, note) VALUES (?1, ?2, ?3, ?4)",
        params![event_id, amount_minor, event_date, note],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn delete_event(conn: &Connection, event_id: i64) -> rusqlite::Result<()> {
    let now = local_now();
    let rows = conn.execute(
        "UPDATE event SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, event_id],
    )?;
    if rows == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

type SnapshotRawRow = (i64, String, String, i64, String, i64, i64, i64);

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
           c.id AS currency_id,
           c.code AS currency_code,
           c.minor_units AS currency_minor_units,
           c.is_custom AS currency_is_custom,
           COALESCE((
             SELECT ed.amount_minor
             FROM event e
             JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.account_id = a.id
               AND e.deleted_at IS NULL
               AND ed.event_date <= ?1
             ORDER BY ed.event_date DESC, e.created_at DESC
             LIMIT 1
           ), 0) AS balance_minor
         FROM account a
         JOIN currency c ON c.id = a.currency_id
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
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut result = Vec::with_capacity(row_data.len());
    for (
        account_id,
        account_name,
        account_type,
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
                Some((mantissa, exponent)) => {
                    let converted = convert_balance(
                        balance_minor,
                        mantissa,
                        exponent,
                        currency_minor_units,
                        consolidation.minor_units,
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
                    );
                    (converted, true)
                }
            }
        };

        result.push(SnapshotRow {
            account_id,
            account_name,
            account_type,
            balance_minor,
            currency_code,
            currency_minor_units,
            is_custom: currency_is_custom != 0,
            converted_balance_minor,
            fx_rate_missing,
            allocated_total_minor: 0,
            linked_allocations_balance_minor: 0,
            over_allocation_buckets: vec![],
            linked_allocations: vec![],
            linked_allocations_from_assets_minor: 0,
            is_linked_to_asset: false,
            linked_asset_ids: vec![],
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

    // Second pass: populate allocation data for buckets and accounts.
    for row in &mut result {
        if row.account_type == "bucket" {
            let allocations = list_bucket_allocations(conn, row.account_id, snapshot_date)?;
            let mut linked_sum: i64 = 0;
            let mut asset_sum: i64 = 0;
            for alloc in &allocations {
                let converted = if alloc.source_currency_id == consolidation.id {
                    alloc.amount_minor
                } else {
                    match get_fx_rate_for_conversion(
                        conn,
                        consolidation.id,
                        alloc.source_currency_id,
                        snapshot_date,
                    )? {
                        Some((mantissa, exponent)) => convert_balance(
                            alloc.amount_minor,
                            mantissa,
                            exponent,
                            alloc.source_currency_minor_units,
                            consolidation.minor_units,
                        ),
                        None => {
                            row.fx_rate_missing = true;
                            convert_balance(
                                alloc.amount_minor,
                                1,
                                0,
                                alloc.source_currency_minor_units,
                                consolidation.minor_units,
                            )
                        }
                    }
                };
                linked_sum += converted;
                if alloc.source_account_type == "asset"
                    || linked_account_ids_set.contains(&alloc.source_account_id)
                {
                    asset_sum += converted;
                }
            }
            row.linked_allocations_balance_minor = linked_sum;
            row.linked_allocations_from_assets_minor = asset_sum;
            row.converted_balance_minor += linked_sum;
            row.linked_allocations = allocations;
        } else if row.account_type == "account" {
            let total_allocated = get_account_allocated_total(conn, row.account_id, snapshot_date)?;
            row.allocated_total_minor = total_allocated;
            if total_allocated > 0 && total_allocated > row.balance_minor {
                let mut stmt = conn.prepare(
                    "SELECT ba.bucket_id, a.name AS bucket_name, ba.amount_minor
                     FROM bucket_allocation ba
                     JOIN account a ON a.id = ba.bucket_id
                     WHERE ba.source_account_id = ?1
                       AND ba.amount_minor != 0
                       AND ba.id = (
                           SELECT id FROM bucket_allocation ba2
                           WHERE ba2.source_account_id = ?1
                             AND ba2.bucket_id = ba.bucket_id
                             AND ba2.effective_date <= ?2
                           ORDER BY ba2.effective_date DESC, ba2.id DESC
                           LIMIT 1
                       )",
                )?;
                let buckets: Vec<AllocationDetail> = stmt
                    .query_map(params![row.account_id, snapshot_date], |r| {
                        Ok(AllocationDetail {
                            bucket_id: r.get(0)?,
                            bucket_name: r.get(1)?,
                            amount_minor: r.get(2)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                row.over_allocation_buckets = buckets;
            }
        }
    }

    Ok(result)
}

pub fn list_events(
    conn: &Connection,
    account_id: Option<i64>,
    before_date: Option<&str>,
) -> rusqlite::Result<Vec<EventWithData>> {
    let sql = "
        SELECT
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
          c.minor_units AS currency_minor_units
        FROM event e
        JOIN account a ON a.id = e.account_id
        JOIN currency c ON c.id = a.currency_id
        JOIN event_data ed ON ed.id = e.latest_data_id
        WHERE e.deleted_at IS NULL
          AND (?1 IS NULL OR e.account_id = ?1)
          AND (?2 IS NULL OR ed.event_date <= ?2)
        ORDER BY ed.event_date DESC, e.created_at DESC
    ";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![account_id, before_date], |row| {
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
        })
    })?;

    rows.collect()
}

fn local_now() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn list_currencies(
    conn: &Connection,
    include_custom: Option<bool>,
) -> rusqlite::Result<Vec<Currency>> {
    let sql = if include_custom.unwrap_or(true) {
        "SELECT id, code, name, minor_units, is_custom FROM currency ORDER BY code"
    } else {
        "SELECT id, code, name, minor_units, is_custom FROM currency WHERE is_custom = 0 ORDER BY code"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(Currency {
            id: row.get(0)?,
            code: row.get(1)?,
            name: row.get(2)?,
            minor_units: row.get(3)?,
            is_custom: row.get::<_, i64>(4)? != 0,
        })
    })?;
    rows.collect()
}

pub fn get_consolidation_currency(conn: &Connection) -> rusqlite::Result<Currency> {
    conn.query_row(
        "SELECT c.id, c.code, c.name, c.minor_units, c.is_custom
         FROM currency c
         JOIN app_setting s ON s.value = c.code
         WHERE s.key = 'consolidation_currency_code'",
        [],
        |row| {
            Ok(Currency {
                id: row.get(0)?,
                code: row.get(1)?,
                name: row.get(2)?,
                minor_units: row.get(3)?,
                is_custom: row.get::<_, i64>(4)? != 0,
            })
        },
    )
}

pub fn set_consolidation_currency(conn: &Connection, currency_id: i64) -> rusqlite::Result<()> {
    with_savepoint(conn, || {
        conn.execute(
            "UPDATE app_setting
             SET value = (SELECT code FROM currency WHERE id = ?1)
             WHERE key = 'consolidation_currency_code'",
            params![currency_id],
        )?;
        // Invalidate all auto-fetched rates — they were computed from the old consolidation.
        conn.execute("DELETE FROM fx_rate WHERE is_manual = 0", [])?;
        Ok(())
    })
}

pub fn set_fx_rate_manual(
    conn: &Connection,
    from_currency_id: i64,
    to_currency_id: i64,
    date: &str,
    rate_mantissa: i64,
    rate_exponent: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO fx_rate (date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent, is_manual)
         VALUES (?1, ?2, ?3, ?4, ?5, 1)
         ON CONFLICT (date, from_currency_id, to_currency_id)
         DO UPDATE SET
           rate_mantissa = excluded.rate_mantissa,
           rate_exponent = excluded.rate_exponent,
           is_manual = 1,
           fetched_at = strftime('%Y-%m-%dT%H:%M:%f','now')",
        params![date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent],
    )?;
    Ok(())
}

pub fn list_fx_rates(
    conn: &Connection,
    date_filter: Option<&str>,
) -> rusqlite::Result<Vec<FxRateRow>> {
    let mut stmt = conn.prepare(
        "SELECT
           fx.id,
           fx.date,
           cf.code AS from_currency_code,
           ct.code AS to_currency_code,
           fx.rate_mantissa,
           fx.rate_exponent,
           fx.is_manual,
           fx.fetched_at
         FROM fx_rate fx
         JOIN currency cf ON cf.id = fx.from_currency_id
         JOIN currency ct ON ct.id = fx.to_currency_id
         WHERE (?1 IS NULL OR fx.date = ?1)
         ORDER BY fx.date DESC, cf.code, ct.code",
    )?;
    let rows = stmt.query_map(params![date_filter], |row| {
        Ok(FxRateRow {
            id: row.get(0)?,
            date: row.get(1)?,
            from_currency_code: row.get(2)?,
            to_currency_code: row.get(3)?,
            rate_mantissa: row.get(4)?,
            rate_exponent: row.get(5)?,
            is_manual: row.get::<_, i64>(6)? != 0,
            fetched_at: row.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn get_fx_rate_for_conversion(
    conn: &Connection,
    from_currency_id: i64,
    to_currency_id: i64,
    date: &str,
) -> rusqlite::Result<Option<(i64, i64)>> {
    conn.query_row(
        "SELECT rate_mantissa, rate_exponent
         FROM fx_rate
         WHERE from_currency_id = ?1
           AND to_currency_id = ?2
           AND date <= ?3
         ORDER BY date DESC
         LIMIT 1",
        params![from_currency_id, to_currency_id, date],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )
    .optional()
}

/// Batch-upsert auto-fetched FX rates.
/// Skips rows where `is_manual = 1` (manual rates are preserved and not overwritten).
pub fn upsert_fx_rates(
    conn: &Connection,
    rates: &[(String, i64, i64, i64, i64)], // (date, from_currency_id, to_currency_id, mantissa, exponent)
) -> rusqlite::Result<()> {
    with_savepoint(conn, || {
        for (date, from_id, to_id, mantissa, exponent) in rates {
            conn.execute(
                "INSERT INTO fx_rate
                   (date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent, is_manual)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0)
                 ON CONFLICT (date, from_currency_id, to_currency_id) DO UPDATE SET
                   rate_mantissa = excluded.rate_mantissa,
                   rate_exponent = excluded.rate_exponent,
                   fetched_at    = strftime('%Y-%m-%dT%H:%M:%f','now')
                 WHERE is_manual = 0",
                params![date, from_id, to_id, mantissa, exponent],
            )?;
        }
        Ok(())
    })
}

/// Read a single app_setting value by key. Returns None if the key does not exist.
pub fn get_app_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_setting WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

/// Upsert an app_setting key-value pair (insert or replace existing value).
pub fn set_app_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Return (code, id) for each distinct currency used by accounts, excluding the
/// consolidation currency. These are the currencies for which cross rates are needed.
pub fn get_active_foreign_currencies(
    conn: &Connection,
    consolidation_id: i64,
) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT c.code, c.id
         FROM account a
         JOIN currency c ON c.id = a.currency_id
         WHERE a.currency_id != ?1
           AND c.is_custom = 0",
    )?;
    let rows = stmt.query_map(params![consolidation_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    rows.collect()
}

/// Return the most recently stored fx_rate date for a given (from, to) currency pair.
/// Returns None if no rates have been stored for that pair.
pub fn get_latest_fx_rate_date(
    conn: &Connection,
    from_currency_id: i64,
    to_currency_id: i64,
) -> rusqlite::Result<Option<String>> {
    // MAX() on an empty set returns NULL — query_row always gets one row.
    conn.query_row(
        "SELECT MAX(date) FROM fx_rate WHERE from_currency_id = ?1 AND to_currency_id = ?2",
        params![from_currency_id, to_currency_id],
        |row| row.get::<_, Option<String>>(0),
    )
}

/// Return dates that have balance-update events for non-consolidation-currency accounts
/// but lack a corresponding fx_rate row.  Used for ledger-driven backfill.
pub fn get_dates_needing_fx_rates(
    conn: &Connection,
    consolidation_id: i64,
) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT substr(ed.event_date, 1, 10) AS d
         FROM event e
         JOIN event_data ed ON ed.id = e.latest_data_id
         JOIN account a     ON a.id  = e.account_id
         WHERE a.currency_id != ?1
           AND e.deleted_at IS NULL
           AND e.latest_data_id IS NOT NULL
           AND NOT EXISTS (
             SELECT 1 FROM fx_rate fx
             WHERE fx.date             = substr(ed.event_date, 1, 10)
               AND fx.from_currency_id = ?1
               AND fx.to_currency_id   = a.currency_id
           )
         ORDER BY d",
    )?;
    let rows = stmt.query_map(params![consolidation_id], |row| row.get(0))?;
    rows.collect()
}

/// Return `true` if every currency in `active_currencies` has a stored fx_rate row for the
/// given `date` and `consolidation_id`. Returns `true` immediately if the slice is empty.
pub fn has_all_fx_rates_for_date(
    conn: &Connection,
    consolidation_id: i64,
    active_currencies: &[(String, i64)],
    date: &str,
) -> rusqlite::Result<bool> {
    if active_currencies.is_empty() {
        return Ok(true);
    }
    for (_, currency_id) in active_currencies {
        let exists: i64 = conn.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM fx_rate
               WHERE date = ?1
                 AND from_currency_id = ?2
                 AND to_currency_id = ?3
             )",
            params![date, consolidation_id, currency_id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Convert `balance_minor` (in source currency) to destination currency minor units.
///
/// `rate` is stored as: 1 destination unit = `rate_mantissa × 10^rate_exponent` source units.
/// Formula (corrected for minor-unit mismatch):
///   `exp_adj = -rate_exponent + dest_minor_units - source_minor_units`
///   if exp_adj >= 0: `converted = (balance × 10^exp_adj + mantissa/2) / mantissa`
///   if exp_adj < 0: `converted = (balance + mantissa×10^|exp_adj|/2) / (mantissa×10^|exp_adj|)`
/// All intermediates use i128 to prevent overflow.
pub fn convert_balance(
    balance_minor: i64,
    rate_mantissa: i64,
    rate_exponent: i64,
    source_minor_units: i64,
    dest_minor_units: i64,
) -> i64 {
    let exp_adj = -rate_exponent + dest_minor_units - source_minor_units;
    let balance = balance_minor as i128;
    let mantissa = rate_mantissa as i128;

    if exp_adj >= 0 {
        let factor = 10_i128.pow(exp_adj as u32);
        (((balance * factor) + mantissa / 2) / mantissa) as i64
    } else {
        let factor = 10_i128.pow((-exp_adj) as u32);
        let denominator = mantissa * factor;
        ((balance + denominator / 2) / denominator) as i64
    }
}

// ---------------------------------------------------------------------------
// Bucket allocation functions (Step 5)
// ---------------------------------------------------------------------------

/// Return the account_type of the account with the given `id`, or `None` if not found.
pub fn get_account_type(conn: &Connection, account_id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT account_type FROM account WHERE id = ?1",
        params![account_id],
        |row| row.get(0),
    )
    .optional()
}

/// Return the latest allocation amount from `source_account_id` to `bucket_id`
/// at or before `effective_date`. Returns 0 if no allocation exists.
pub fn get_existing_allocation_to_bucket(
    conn: &Connection,
    source_account_id: i64,
    bucket_id: i64,
    effective_date: &str,
) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT amount_minor FROM bucket_allocation
             WHERE source_account_id = ?1
               AND bucket_id = ?2
               AND effective_date <= ?3
             ORDER BY effective_date DESC, id DESC
             LIMIT 1",
        params![source_account_id, bucket_id, effective_date],
        |row| row.get(0),
    )
    .optional()
    .map(|opt| opt.unwrap_or(0))
}

pub fn create_bucket_allocation(
    conn: &Connection,
    bucket_id: i64,
    source_account_id: i64,
    amount_minor: i64,
    effective_date: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO bucket_allocation (bucket_id, source_account_id, amount_minor, effective_date)
         VALUES (?1, ?2, ?3, ?4)",
        params![bucket_id, source_account_id, amount_minor, effective_date],
    )?;
    Ok(conn.last_insert_rowid())
}

/// For each distinct `source_account_id` linked to this bucket, return the latest
/// allocation row where `effective_date <= as_of_date`. Excludes zero-amount rows
/// (amount_minor = 0 represents an unlink event).
pub fn list_bucket_allocations(
    conn: &Connection,
    bucket_id: i64,
    as_of_date: &str,
) -> rusqlite::Result<Vec<BucketAllocation>> {
    let mut stmt = conn.prepare(
        "SELECT
           ba.id,
           ba.bucket_id,
           ba.source_account_id,
           a.name  AS source_account_name,
           a.account_type AS source_account_type,
           c.id    AS source_currency_id,
           c.code  AS source_currency_code,
           c.minor_units AS source_currency_minor_units,
           ba.amount_minor,
           ba.effective_date
         FROM bucket_allocation ba
         JOIN account  a ON a.id = ba.source_account_id
         JOIN currency c ON c.id = a.currency_id
         WHERE ba.bucket_id = ?1
           AND ba.id = (
               SELECT id FROM bucket_allocation
               WHERE bucket_id = ?1
                 AND source_account_id = ba.source_account_id
                 AND effective_date <= ?2
               ORDER BY effective_date DESC, id DESC
               LIMIT 1
           )
           AND ba.amount_minor != 0",
    )?;
    let rows = stmt.query_map(params![bucket_id, as_of_date], |row| {
        Ok(BucketAllocation {
            id: row.get(0)?,
            bucket_id: row.get(1)?,
            source_account_id: row.get(2)?,
            source_account_name: row.get(3)?,
            source_account_type: row.get(4)?,
            source_currency_id: row.get(5)?,
            source_currency_code: row.get(6)?,
            source_currency_minor_units: row.get(7)?,
            amount_minor: row.get(8)?,
            effective_date: row.get(9)?,
        })
    })?;
    rows.collect()
}

/// Sum the latest allocation amount from `source_account_id` across all buckets
/// where `effective_date <= as_of_date`. Zero-amount (unlinked) rows are excluded.
pub fn get_account_allocated_total(
    conn: &Connection,
    source_account_id: i64,
    as_of_date: &str,
) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COALESCE(SUM(ba.amount_minor), 0)
         FROM bucket_allocation ba
         WHERE ba.source_account_id = ?1
           AND ba.amount_minor != 0
           AND ba.id = (
               SELECT id FROM bucket_allocation ba2
               WHERE ba2.source_account_id = ?1
                 AND ba2.bucket_id = ba.bucket_id
                 AND ba2.effective_date <= ?2
               ORDER BY ba2.effective_date DESC, ba2.id DESC
               LIMIT 1
           )",
        params![source_account_id, as_of_date],
        |row| row.get(0),
    )
}

/// Return the balance of `account_id` as of `selected_datetime` (ISO 8601 datetime).
/// Applies the same snapshot algorithm as `get_accounts_snapshot`: latest non-deleted
/// event_data where event_date <= selected_datetime. Returns 0 if no events found.
pub fn get_account_balance_at_date(
    conn: &Connection,
    account_id: i64,
    selected_datetime: &str,
) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COALESCE((
           SELECT ed.amount_minor
           FROM event e
           JOIN event_data ed ON ed.id = e.latest_data_id
           WHERE e.account_id = ?1
             AND e.deleted_at IS NULL
             AND ed.event_date <= ?2
           ORDER BY ed.event_date DESC, e.created_at DESC
           LIMIT 1
         ), 0)",
        params![account_id, selected_datetime],
        |row| row.get(0),
    )
}

/// Check whether `source_account_id` is over-allocated as of `as_of_date` (YYYY-MM-DD).
/// Returns `Some(OverAllocationWarning)` if total allocations exceed the account balance,
/// or `None` if the account is within its balance.
pub fn check_over_allocation(
    conn: &Connection,
    source_account_id: i64,
    as_of_date: &str,
) -> rusqlite::Result<Option<OverAllocationWarning>> {
    let selected_datetime = format!("{}T23:59:59", as_of_date);
    let balance = get_account_balance_at_date(conn, source_account_id, &selected_datetime)?;
    let total_allocated = get_account_allocated_total(conn, source_account_id, as_of_date)?;

    if total_allocated <= balance {
        return Ok(None);
    }

    let (account_name, currency_code, currency_minor_units): (String, String, i64) = conn
        .query_row(
            "SELECT a.name, c.code, c.minor_units
             FROM account a
             JOIN currency c ON c.id = a.currency_id
             WHERE a.id = ?1",
            params![source_account_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

    let mut stmt = conn.prepare(
        "SELECT ba.bucket_id, a.name AS bucket_name, ba.amount_minor
         FROM bucket_allocation ba
         JOIN account a ON a.id = ba.bucket_id
         WHERE ba.source_account_id = ?1
           AND ba.amount_minor != 0
           AND ba.id = (
               SELECT id FROM bucket_allocation ba2
               WHERE ba2.source_account_id = ?1
                 AND ba2.bucket_id = ba.bucket_id
                 AND ba2.effective_date <= ?2
               ORDER BY ba2.effective_date DESC, ba2.id DESC
               LIMIT 1
           )",
    )?;
    let allocations: Vec<AllocationDetail> = stmt
        .query_map(params![source_account_id, as_of_date], |row| {
            Ok(AllocationDetail {
                bucket_id: row.get(0)?,
                bucket_name: row.get(1)?,
                amount_minor: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Some(OverAllocationWarning {
        source_account_id,
        source_account_name: account_name,
        currency_code,
        currency_minor_units,
        balance_minor: balance,
        total_allocated_minor: total_allocated,
        over_allocation_minor: total_allocated - balance,
        allocations,
    }))
}

// ---------------------------------------------------------------------------
// Custom unit functions (Step 19)
// ---------------------------------------------------------------------------

/// Parse a decimal price string P and return the inverse rate (1/P) as (mantissa, exponent).
/// Uses 12-digit precision arithmetic to avoid floating-point loss.
fn invert_price_str(price_str: &str) -> Result<(i64, i64), AppError> {
    let (m_p, e_p) = crate::oxr::parse_decimal_str(price_str)?;
    if m_p == 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Price must be greater than zero".into(),
        });
    }
    const PRECISION: i64 = 12;
    let precision_factor = 10_i128.pow(PRECISION as u32);
    let mut mantissa = precision_factor / (m_p as i128);
    let mut exponent = -e_p - PRECISION;

    if mantissa == 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Computed rate is zero (price too large)".into(),
        });
    }

    // Normalize: strip trailing zeros.
    while mantissa != 0 && mantissa % 10 == 0 {
        mantissa /= 10;
        exponent += 1;
    }

    if mantissa > i64::MAX as i128 || mantissa < i64::MIN as i128 {
        return Err(AppError {
            code: "OVERFLOW".into(),
            message: "Computed rate mantissa overflows i64".into(),
        });
    }

    Ok((mantissa as i64, exponent))
}

/// Convert a price-per-unit string to an FX rate and upsert it.
/// Stores rate from `consolidation → asset_currency` on `date`.
/// Used by both `update_asset_value` and `create_account` (with initial price).
pub fn store_asset_price(
    conn: &Connection,
    account_id: i64,
    price_str: &str,
    date: &str,
) -> Result<(), AppError> {
    let asset_currency_id: i64 = conn
        .query_row(
            "SELECT currency_id FROM account WHERE id = ?1",
            params![account_id],
            |row| row.get(0),
        )
        .map_err(AppError::from)?;

    let consolidation = get_consolidation_currency(conn).map_err(AppError::from)?;
    let (rate_mantissa, rate_exponent) = invert_price_str(price_str)?;

    set_fx_rate_manual(
        conn,
        consolidation.id,
        asset_currency_id,
        date,
        rate_mantissa,
        rate_exponent,
    )
    .map_err(AppError::from)?;

    Ok(())
}

/// Create a custom unit (currency with `is_custom = 1`).
/// `code` and `name` are both set to `name`; validates uniqueness and minor_units range.
pub fn create_custom_unit(
    conn: &Connection,
    name: &str,
    minor_units: i64,
) -> Result<i64, AppError> {
    let exists: i64 = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM currency WHERE code = ?1)",
            params![name],
            |row| row.get(0),
        )
        .map_err(AppError::from)?;
    if exists != 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: format!("A currency with code '{}' already exists", name),
        });
    }

    conn.execute(
        "INSERT INTO currency (code, name, minor_units, is_custom) VALUES (?1, ?2, ?3, 1)",
        params![name, name, minor_units],
    )
    .map_err(AppError::from)?;

    Ok(conn.last_insert_rowid())
}

/// List all custom units (currencies where `is_custom = 1`), ordered by code.
pub fn list_custom_units(conn: &Connection) -> rusqlite::Result<Vec<Currency>> {
    let mut stmt = conn.prepare(
        "SELECT id, code, name, minor_units, is_custom FROM currency WHERE is_custom = 1 ORDER BY code",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Currency {
            id: row.get(0)?,
            code: row.get(1)?,
            name: row.get(2)?,
            minor_units: row.get(3)?,
            is_custom: row.get::<_, i64>(4)? != 0,
        })
    })?;
    rows.collect()
}

/// Rename a custom unit. Updates both `code` and `name` columns.
/// Returns `NOT_FOUND` if the currency doesn't exist, `VALIDATION` if it is not custom
/// or the new name conflicts with an existing code.
pub fn update_custom_unit(conn: &Connection, currency_id: i64, name: &str) -> Result<(), AppError> {
    let is_custom: Option<i64> = conn
        .query_row(
            "SELECT is_custom FROM currency WHERE id = ?1",
            params![currency_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(AppError::from)?;

    match is_custom {
        None => {
            return Err(AppError {
                code: "NOT_FOUND".into(),
                message: "Currency not found".into(),
            })
        }
        Some(0) => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Cannot rename a built-in currency".into(),
            })
        }
        _ => {}
    }

    let conflict: i64 = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM currency WHERE code = ?1 AND id != ?2)",
            params![name, currency_id],
            |row| row.get(0),
        )
        .map_err(AppError::from)?;
    if conflict != 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: format!("A currency with code '{}' already exists", name),
        });
    }

    conn.execute(
        "UPDATE currency SET code = ?1, name = ?2 WHERE id = ?3",
        params![name, name, currency_id],
    )
    .map_err(AppError::from)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Asset value update (Step 20)
// ---------------------------------------------------------------------------

/// Record a new asset value: optionally update the balance snapshot and/or the
/// price-per-unit FX rate. At least one of `amount_minor` or `price_per_unit` must
/// be provided. The account must be of type 'asset'.
pub fn update_asset_value(
    conn: &Connection,
    account_id: i64,
    amount_minor: Option<i64>,
    price_per_unit: Option<&str>,
    event_date: &str,
    note: Option<&str>,
) -> Result<(), AppError> {
    let account_type: Option<String> = conn
        .query_row(
            "SELECT account_type FROM account WHERE id = ?1",
            params![account_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(AppError::from)?;

    match account_type.as_deref() {
        None => {
            return Err(AppError {
                code: "NOT_FOUND".into(),
                message: "Account not found".into(),
            })
        }
        Some("asset") => {}
        Some(_) => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "update_asset_value can only be used with asset accounts".into(),
            })
        }
    }

    with_savepoint_app(conn, || {
        if let Some(amount) = amount_minor {
            create_balance_update_inner(conn, account_id, amount, event_date, note)
                .map_err(AppError::from)?;
        }

        if let Some(price_str) = price_per_unit {
            store_asset_price(conn, account_id, price_str, event_date)?;
        }

        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Account–Asset Linking functions
// ---------------------------------------------------------------------------

/// Return all account_asset_link rows, optionally filtered by account_id.
/// Joins with account table (twice) to return account/asset names.
pub fn list_account_asset_links(
    conn: &Connection,
    account_id: Option<i64>,
) -> rusqlite::Result<Vec<AccountAssetLink>> {
    let mut stmt = conn.prepare(
        "SELECT
           aal.id,
           aal.account_id,
           acc.name AS account_name,
           aal.asset_id,
           ast.name AS asset_name
         FROM account_asset_link aal
         JOIN account acc ON acc.id = aal.account_id
         JOIN account ast ON ast.id = aal.asset_id
         WHERE (?1 IS NULL OR aal.account_id = ?1)
         ORDER BY aal.id",
    )?;
    let rows = stmt.query_map(params![account_id], |row| {
        Ok(AccountAssetLink {
            id: row.get(0)?,
            account_id: row.get(1)?,
            account_name: row.get(2)?,
            asset_id: row.get(3)?,
            asset_name: row.get(4)?,
        })
    })?;
    rows.collect()
}

/// Replace all asset links for `account_id` with the supplied `asset_ids`.
/// Validates that `account_id` is of type 'account' and each asset_id is of type 'asset'.
pub fn set_account_asset_links(
    conn: &Connection,
    account_id: i64,
    asset_ids: &[i64],
) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        // Validate that account_id refers to an account-type row.
        let account_type: Option<String> = get_account_type(conn, account_id)?;
        match account_type.as_deref() {
            Some("account") => {}
            Some(_) => {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: "account_id must refer to an account-type account".into(),
                })
            }
            None => {
                return Err(AppError {
                    code: "NOT_FOUND".into(),
                    message: "Account not found".into(),
                })
            }
        }

        // Validate that each asset_id refers to an asset-type row.
        for &aid in asset_ids {
            let asset_type: Option<String> = get_account_type(conn, aid)?;
            match asset_type.as_deref() {
                Some("asset") => {}
                Some(_) => {
                    return Err(AppError {
                        code: "VALIDATION".into(),
                        message: format!("asset_id {} must refer to an asset-type account", aid),
                    })
                }
                None => {
                    return Err(AppError {
                        code: "NOT_FOUND".into(),
                        message: format!("Asset {} not found", aid),
                    })
                }
            }
        }

        // Delete existing links for this account.
        conn.execute(
            "DELETE FROM account_asset_link WHERE account_id = ?1",
            params![account_id],
        )
        .map_err(AppError::from)?;

        // Insert new links.
        for &aid in asset_ids {
            conn.execute(
                "INSERT INTO account_asset_link (account_id, asset_id) VALUES (?1, ?2)",
                params![account_id, aid],
            )
            .map_err(AppError::from)?;
        }

        Ok(())
    })
}

/// Fetch all account_asset_link rows and return three derived structures:
///
/// 1. Set of linked account IDs
/// 2. Map of account_id → Vec<asset_id>
/// 3. Map of asset_id → Vec<account_id>
///
/// Used by the snapshot computation to avoid N+1 queries.
#[allow(clippy::type_complexity)]
pub fn get_all_account_asset_link_ids(
    conn: &Connection,
) -> rusqlite::Result<(HashSet<i64>, HashMap<i64, Vec<i64>>, HashMap<i64, Vec<i64>>)> {
    let mut stmt = conn.prepare(
        "SELECT account_id, asset_id FROM account_asset_link ORDER BY account_id, asset_id",
    )?;
    let rows: Vec<(i64, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut linked_set: HashSet<i64> = HashSet::new();
    let mut account_to_assets: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut asset_to_accounts: HashMap<i64, Vec<i64>> = HashMap::new();

    for (account_id, asset_id) in rows {
        linked_set.insert(account_id);
        account_to_assets
            .entry(account_id)
            .or_default()
            .push(asset_id);
        asset_to_accounts
            .entry(asset_id)
            .or_default()
            .push(account_id);
    }

    Ok((linked_set, account_to_assets, asset_to_accounts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;

    /// Create a plain EUR account for use as a test fixture.
    fn mk_account(conn: &Connection) -> i64 {
        create_account(conn, "Test Account", 1, "account", None, None)
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
    fn delete_account_cascades_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 5000, "2026-03-01", None).unwrap();
        delete_account(&conn, account_id).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        assert!(snapshot.iter().all(|r| r.account_id != account_id));
        let events = list_events(&conn, Some(account_id), None).unwrap();
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn delete_account_succeeds_when_no_active_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let result = delete_account(&conn, account_id);
        assert!(result.is_ok());
    }

    #[test]
    fn delete_unit_asset_cleans_up_orphan_unit() {
        let conn = initialize_in_memory().expect("DB init failed");

        // Create a custom unit currency (e.g. a stock ticker)
        let unit_id = create_custom_unit(&conn, "TSLA", 4).unwrap();

        // Create an asset account denominated in this custom unit
        let account_id = create_account(&conn, "TSLA Asset", unit_id, "asset", None, None).unwrap();

        // Store a balance event for the asset
        create_balance_update(&conn, account_id, 10_000, "2026-03-11", None).unwrap();

        // Store an fx_rate for the custom unit (EUR → TSLA)
        let eur_id: i64 = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |row| {
                row.get(0)
            })
            .unwrap();
        set_fx_rate_manual(&conn, eur_id, unit_id, "2026-03-11", 1, -2).unwrap();

        // Delete the account
        delete_account(&conn, account_id).unwrap();

        // Account must be gone from snapshot
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        assert!(snapshot.iter().all(|r| r.account_id != account_id));

        // Events must be gone
        let events = list_events(&conn, Some(account_id), None).unwrap();
        assert_eq!(events.len(), 0);

        // fx_rate rows for the custom unit must be cleaned up
        let fx_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM fx_rate WHERE from_currency_id = ?1 OR to_currency_id = ?1",
                rusqlite::params![unit_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fx_count, 0, "fx_rate rows should have been deleted");

        // The custom currency row itself must be gone
        let curr_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM currency WHERE id = ?1",
                rusqlite::params![unit_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(curr_count, 0, "custom currency should have been deleted");
    }

    #[test]
    fn create_account_with_initial_balance() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = create_account(&conn, "Savings", 1, "account", Some(10000), None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        let row = snapshot
            .iter()
            .find(|r| r.account_id == account_id)
            .unwrap();
        assert_eq!(row.balance_minor, 10000);
    }

    #[test]
    fn list_events_returns_all_non_deleted() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 1000, "2026-01-01", None).unwrap();
        create_balance_update(&conn, account_id, 2000, "2026-02-01", None).unwrap();
        let events = list_events(&conn, None, None).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn list_events_filters_by_account() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(&conn, "Second", 1, "account", None, None).unwrap();
        create_balance_update(&conn, acc1, 1000, "2026-01-01", None).unwrap();
        create_balance_update(&conn, acc2, 2000, "2026-02-01", None).unwrap();

        let events_acc1 = list_events(&conn, Some(acc1), None).unwrap();
        assert_eq!(events_acc1.len(), 1);
        assert_eq!(events_acc1[0].account_id, acc1);

        let events_acc2 = list_events(&conn, Some(acc2), None).unwrap();
        assert_eq!(events_acc2.len(), 1);
        assert_eq!(events_acc2[0].account_id, acc2);
    }

    #[test]
    fn list_events_filters_by_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 1000, "2026-01-15", None).unwrap();
        create_balance_update(&conn, account_id, 2000, "2026-03-15", None).unwrap();
        let events = list_events(&conn, None, Some("2026-02-01T23:59:59")).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].amount_minor, 1000);
    }

    #[test]
    fn create_bucket_appears_in_snapshot() {
        let conn = initialize_in_memory().expect("DB init failed");
        let bucket_id =
            create_account(&conn, "Emergency Fund", 1, "bucket", Some(20000), None).unwrap();
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
        let bucket_id = create_account(&conn, "Savings Bucket", 1, "bucket", None, None).unwrap();
        create_balance_update(&conn, bucket_id, 15000, "2026-03-01", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        let bucket = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();
        assert_eq!(bucket.balance_minor, 15000);
    }

    #[test]
    fn list_events_includes_account_type() {
        let conn = initialize_in_memory().expect("DB init failed");
        let bucket_id = create_account(&conn, "Test Bucket", 1, "bucket", None, None).unwrap();
        create_balance_update(&conn, bucket_id, 5000, "2026-03-01", None).unwrap();
        let events = list_events(&conn, Some(bucket_id), None).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].account_type, "bucket");

        let empty_account_id =
            create_account(&conn, "Empty Account", 1, "account", None, None).unwrap();
        let events_empty = list_events(&conn, Some(empty_account_id), None).unwrap();
        // Account with no balance updates has no events
        assert_eq!(events_empty.len(), 0);
    }

    #[test]
    fn snapshot_orders_accounts_before_buckets() {
        let conn = initialize_in_memory().expect("DB init failed");
        create_account(&conn, "Zebra Bucket", 1, "bucket", None, None).unwrap();
        create_account(&conn, "Alpha Account", 1, "account", None, None).unwrap();
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
        let acc = create_account(&conn, "USD Account", usd, "account", None, None).unwrap();
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
        let acc = create_account(&conn, "USD Account", usd, "account", None, None).unwrap();
        create_balance_update(&conn, acc, 108420, "2026-03-01", None).unwrap();
        // Store rate: 1 EUR = 1.0842 USD (mantissa=10842, exponent=-4)
        set_fx_rate_manual(&conn, eur, usd, "2026-03-01", 10842, -4).unwrap();
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
        let events = list_events(&conn, Some(account_id), None).unwrap();
        assert_eq!(events[0].currency_code, "EUR");
        assert_eq!(events[0].currency_minor_units, 2);
    }

    #[test]
    fn get_consolidation_currency_returns_eur() {
        let conn = initialize_in_memory().expect("DB init failed");
        let c = get_consolidation_currency(&conn).unwrap();
        assert_eq!(c.code, "EUR");
        assert_eq!(c.minor_units, 2);
    }

    #[test]
    fn set_consolidation_currency_updates_and_clears_auto_rates() {
        let conn = initialize_in_memory().expect("DB init failed");
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();

        // Insert an auto-fetched rate and a manual rate
        conn.execute(
            "INSERT INTO fx_rate (date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent, is_manual) VALUES ('2026-03-01', ?1, ?2, 10842, -4, 0)",
            params![eur, usd],
        ).unwrap();
        conn.execute(
            "INSERT INTO fx_rate (date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent, is_manual) VALUES ('2026-03-01', ?1, ?2, 10842, -4, 1)",
            params![usd, eur],
        ).unwrap();

        set_consolidation_currency(&conn, usd).unwrap();

        let c = get_consolidation_currency(&conn).unwrap();
        assert_eq!(c.code, "USD");

        // Auto-fetched rate deleted, manual rate preserved
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM fx_rate WHERE is_manual = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);

        let manual_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM fx_rate WHERE is_manual = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(manual_count, 1);
    }

    #[test]
    fn get_fx_rate_for_conversion_returns_most_recent_on_or_before_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();

        set_fx_rate_manual(&conn, eur, usd, "2026-01-01", 10800, -4).unwrap();
        set_fx_rate_manual(&conn, eur, usd, "2026-02-01", 10842, -4).unwrap();
        set_fx_rate_manual(&conn, eur, usd, "2026-03-15", 10900, -4).unwrap();

        // On 2026-03-01, most recent on-or-before is 2026-02-01
        let rate = get_fx_rate_for_conversion(&conn, eur, usd, "2026-03-01").unwrap();
        assert_eq!(rate, Some((10842, -4)));

        // Before any rates → None
        let none = get_fx_rate_for_conversion(&conn, eur, usd, "2025-12-31").unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn upsert_fx_rates_creates_new_rows() {
        let conn = initialize_in_memory().expect("DB init failed");
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();

        let rates = vec![("2026-03-01".to_string(), eur, usd, 10842_i64, -4_i64)];
        upsert_fx_rates(&conn, &rates).unwrap();

        let rate = get_fx_rate_for_conversion(&conn, eur, usd, "2026-03-01").unwrap();
        assert_eq!(rate, Some((10842, -4)));
    }

    #[test]
    fn upsert_fx_rates_overwrites_existing_auto_rows() {
        let conn = initialize_in_memory().expect("DB init failed");
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();

        let initial = vec![("2026-03-01".to_string(), eur, usd, 10842_i64, -4_i64)];
        upsert_fx_rates(&conn, &initial).unwrap();

        let updated = vec![("2026-03-01".to_string(), eur, usd, 10900_i64, -4_i64)];
        upsert_fx_rates(&conn, &updated).unwrap();

        let rate = get_fx_rate_for_conversion(&conn, eur, usd, "2026-03-01").unwrap();
        assert_eq!(rate, Some((10900, -4)));
    }

    #[test]
    fn upsert_fx_rates_preserves_manual_rows() {
        let conn = initialize_in_memory().expect("DB init failed");
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();

        // Insert a manual rate
        set_fx_rate_manual(&conn, eur, usd, "2026-03-01", 10842, -4).unwrap();

        // Attempt to overwrite with an auto-fetched rate
        let rates = vec![("2026-03-01".to_string(), eur, usd, 99999_i64, -4_i64)];
        upsert_fx_rates(&conn, &rates).unwrap();

        // Manual rate should be unchanged
        let rate = get_fx_rate_for_conversion(&conn, eur, usd, "2026-03-01").unwrap();
        assert_eq!(rate, Some((10842, -4)));

        let is_manual: i64 = conn
            .query_row(
                "SELECT is_manual FROM fx_rate WHERE from_currency_id = ?1 AND to_currency_id = ?2 AND date = '2026-03-01'",
                params![eur, usd],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(is_manual, 1);
    }

    // ---------------------------------------------------------------------------
    // Bucket allocation tests (Step 10b)
    // ---------------------------------------------------------------------------

    #[test]
    fn test_create_and_list_bucket_allocations() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id = create_account(&conn, "Emergency Fund", 1, "bucket", None, None).unwrap();
        create_bucket_allocation(&conn, bucket_id, source_id, 5000, "2024-01-01").unwrap();

        let allocs = list_bucket_allocations(&conn, bucket_id, "2024-12-31").unwrap();
        assert_eq!(allocs.len(), 1);
        assert_eq!(allocs[0].bucket_id, bucket_id);
        assert_eq!(allocs[0].source_account_id, source_id);
        assert_eq!(allocs[0].amount_minor, 5000);
        assert_eq!(allocs[0].source_currency_code, "EUR");
        assert_eq!(allocs[0].effective_date, "2024-01-01");
    }

    #[test]
    fn test_allocation_respects_effective_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id = create_account(&conn, "Vacation Fund", 1, "bucket", None, None).unwrap();
        create_bucket_allocation(&conn, bucket_id, source_id, 5000, "2024-01-01").unwrap();
        create_bucket_allocation(&conn, bucket_id, source_id, 8000, "2024-06-01").unwrap();

        // At 2024-03-01: only the first allocation is effective.
        let early = list_bucket_allocations(&conn, bucket_id, "2024-03-01").unwrap();
        assert_eq!(early.len(), 1);
        assert_eq!(early[0].amount_minor, 5000);

        // At 2024-07-01: the second allocation supersedes the first.
        let late = list_bucket_allocations(&conn, bucket_id, "2024-07-01").unwrap();
        assert_eq!(late.len(), 1);
        assert_eq!(late[0].amount_minor, 8000);
    }

    #[test]
    fn test_unlink_allocation_via_zero_amount() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id = create_account(&conn, "Car Fund", 1, "bucket", None, None).unwrap();
        create_bucket_allocation(&conn, bucket_id, source_id, 5000, "2024-01-01").unwrap();
        // Unlink: zero-amount allocation at a later date.
        create_bucket_allocation(&conn, bucket_id, source_id, 0, "2024-06-01").unwrap();

        // Before unlink date: original allocation is still visible.
        let before = list_bucket_allocations(&conn, bucket_id, "2024-03-01").unwrap();
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].amount_minor, 5000);

        // After unlink date: zero-amount row is excluded → empty.
        let after = list_bucket_allocations(&conn, bucket_id, "2024-07-01").unwrap();
        assert_eq!(after.len(), 0);
    }

    #[test]
    fn test_get_account_allocated_total_sums_across_buckets() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket1 = create_account(&conn, "Bucket A", 1, "bucket", None, None).unwrap();
        let bucket2 = create_account(&conn, "Bucket B", 1, "bucket", None, None).unwrap();
        create_bucket_allocation(&conn, bucket1, source_id, 3000, "2024-01-01").unwrap();
        create_bucket_allocation(&conn, bucket2, source_id, 2000, "2024-01-01").unwrap();

        let total = get_account_allocated_total(&conn, source_id, "2024-12-31").unwrap();
        assert_eq!(total, 5000);
    }

    #[test]
    fn test_snapshot_includes_allocation_in_bucket_balance() {
        let conn = initialize_in_memory().expect("DB init failed");
        // EUR is the consolidation currency; source account and bucket are both EUR.
        let source_id = mk_account(&conn);
        let bucket_id =
            create_account(&conn, "Allocation Bucket", 1, "bucket", None, None).unwrap();
        create_balance_update(&conn, source_id, 10000, "2024-01-01", None).unwrap();
        create_bucket_allocation(&conn, bucket_id, source_id, 4000, "2024-01-01").unwrap();

        let snapshot = get_accounts_snapshot(&conn, "2024-12-31T23:59:59").unwrap();
        let bucket = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();

        // Bucket has no manual balance events, so base balance = 0.
        // linked_allocations_balance_minor should be 4000.
        assert_eq!(bucket.linked_allocations_balance_minor, 4000);
        // converted_balance_minor = base (0) + linked (4000) = 4000.
        assert_eq!(bucket.converted_balance_minor, 4000);
    }

    #[test]
    fn test_snapshot_includes_allocated_total_for_accounts() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket1 = create_account(&conn, "Fund A", 1, "bucket", None, None).unwrap();
        let bucket2 = create_account(&conn, "Fund B", 1, "bucket", None, None).unwrap();
        create_balance_update(&conn, source_id, 20000, "2024-01-01", None).unwrap();
        create_bucket_allocation(&conn, bucket1, source_id, 3000, "2024-01-01").unwrap();
        create_bucket_allocation(&conn, bucket2, source_id, 5000, "2024-01-01").unwrap();

        let snapshot = get_accounts_snapshot(&conn, "2024-12-31T23:59:59").unwrap();
        let account = snapshot.iter().find(|r| r.account_id == source_id).unwrap();
        assert_eq!(account.allocated_total_minor, 8000);
    }

    #[test]
    fn test_over_allocation_check_detects_excess() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id =
            create_account(&conn, "Over-Alloc Bucket", 1, "bucket", None, None).unwrap();
        // Account starts at 10000.
        create_balance_update(&conn, source_id, 10000, "2024-01-01", None).unwrap();
        // Allocate 10000 — exactly matching balance.
        create_bucket_allocation(&conn, bucket_id, source_id, 10000, "2024-01-01").unwrap();
        // Later, balance drops to 5000.
        create_balance_update(&conn, source_id, 5000, "2024-06-01", None).unwrap();

        // As of 2024-12-31: balance=5000, allocated=10000 → over by 5000.
        let warning = check_over_allocation(&conn, source_id, "2024-12-31").unwrap();
        assert!(warning.is_some());
        let w = warning.unwrap();
        assert_eq!(w.source_account_id, source_id);
        assert_eq!(w.balance_minor, 5000);
        assert_eq!(w.total_allocated_minor, 10000);
        assert_eq!(w.over_allocation_minor, 5000);
        assert_eq!(w.allocations.len(), 1);
    }

    #[test]
    fn test_over_allocation_check_returns_none_when_ok() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id = create_account(&conn, "Safe Bucket", 1, "bucket", None, None).unwrap();
        create_balance_update(&conn, source_id, 10000, "2024-01-01", None).unwrap();
        create_bucket_allocation(&conn, bucket_id, source_id, 5000, "2024-01-01").unwrap();

        let warning = check_over_allocation(&conn, source_id, "2024-12-31").unwrap();
        assert!(warning.is_none());
    }

    #[test]
    fn test_delete_account_blocked_by_allocation() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id =
            create_account(&conn, "Emergency Reserve", 1, "bucket", None, None).unwrap();
        create_balance_update(&conn, source_id, 10000, "2024-01-01", None).unwrap();
        create_bucket_allocation(&conn, bucket_id, source_id, 5000, "2024-01-01").unwrap();

        // Attempting to delete the source account should fail.
        let result = delete_account(&conn, source_id);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("Emergency Reserve"),
            "Error message should name the bucket: {}",
            err.message
        );
    }

    #[test]
    fn test_delete_bucket_cascades_allocations() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id = create_account(&conn, "Cascade Bucket", 1, "bucket", None, None).unwrap();
        create_balance_update(&conn, source_id, 10000, "2024-01-01", None).unwrap();
        create_bucket_allocation(&conn, bucket_id, source_id, 5000, "2024-01-01").unwrap();

        // Deleting the bucket should cascade-remove allocation rows.
        delete_account(&conn, bucket_id).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bucket_allocation WHERE bucket_id = ?1",
                params![bucket_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "bucket_allocation rows should be removed by CASCADE"
        );
    }

    #[test]
    fn delete_account_succeeds_after_unlinking_from_bucket() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id = create_account(&conn, "Savings Pot", 1, "bucket", None, None).unwrap();
        create_balance_update(&conn, source_id, 10000, "2024-01-01", None).unwrap();

        // Link: positive-amount allocation row
        create_bucket_allocation(&conn, bucket_id, source_id, 5000, "2024-01-01").unwrap();
        // Unlink: zero-amount row supersedes the link
        create_bucket_allocation(&conn, bucket_id, source_id, 0, "2024-06-01").unwrap();

        // After unlinking, delete should succeed
        let result = delete_account(&conn, source_id);
        assert!(
            result.is_ok(),
            "delete_account failed after unlinking: {:?}",
            result.unwrap_err()
        );

        // All allocation rows for this source must be gone
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bucket_allocation WHERE source_account_id = ?1",
                params![source_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "all allocation rows should have been deleted");

        // The account itself must be gone
        let snap = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        assert!(snap.iter().all(|r| r.account_id != source_id));
    }

    #[test]
    fn delete_account_fails_when_still_linked_to_bucket() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id = create_account(&conn, "Active Reserve", 1, "bucket", None, None).unwrap();
        create_balance_update(&conn, source_id, 10000, "2024-01-01", None).unwrap();

        // Link without unlinking
        create_bucket_allocation(&conn, bucket_id, source_id, 5000, "2024-01-01").unwrap();

        let result = delete_account(&conn, source_id);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("Active Reserve"),
            "Error message should name the bucket: {}",
            err.message
        );
    }

    #[test]
    fn create_account_assigns_sequential_sort_order() {
        let conn = initialize_in_memory().expect("DB init failed");
        // Use bucket type: seed DB has no buckets, so first gets sort_order=0.
        let id1 = create_account(&conn, "First Bucket", 1, "bucket", None, None).unwrap();
        let id2 = create_account(&conn, "Second Bucket", 1, "bucket", None, None).unwrap();
        let so1: i64 = conn
            .query_row(
                "SELECT sort_order FROM account WHERE id = ?1",
                params![id1],
                |row| row.get(0),
            )
            .unwrap();
        let so2: i64 = conn
            .query_row(
                "SELECT sort_order FROM account WHERE id = ?1",
                params![id2],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(so1, 0);
        assert_eq!(so2, 1);
    }

    #[test]
    fn update_sort_order_changes_snapshot_order() {
        let conn = initialize_in_memory().expect("DB init failed");
        let alpha_id = create_account(&conn, "Alpha", 1, "bucket", None, None).unwrap();
        let beta_id = create_account(&conn, "Beta", 1, "bucket", None, None).unwrap();
        // Alpha gets sort_order=0, Beta gets sort_order=1 — swap them.
        update_sort_order(&conn, &[(beta_id, 0), (alpha_id, 1)]).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        let beta_pos = snapshot
            .iter()
            .position(|r| r.account_id == beta_id)
            .unwrap();
        let alpha_pos = snapshot
            .iter()
            .position(|r| r.account_id == alpha_id)
            .unwrap();
        assert!(
            beta_pos < alpha_pos,
            "Beta should come before Alpha after swap"
        );
    }

    #[test]
    fn update_sort_order_rejects_invalid_id() {
        let conn = initialize_in_memory().expect("DB init failed");
        let result = update_sort_order(&conn, &[(9999, 0)]);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------------------
    // Phase 3 — Asset-to-bucket linking (Step 35)
    // ---------------------------------------------------------------------------

    #[test]
    fn test_linked_allocations_from_assets_minor_populates_correctly() {
        let conn = initialize_in_memory().expect("DB init failed");
        // Create a regular account, an asset, and a bucket — all EUR.
        let account_id = mk_account(&conn);
        let asset_id = create_account(&conn, "Test Asset", 1, "asset", Some(50000), None).unwrap();
        let bucket_id = create_account(&conn, "Test Bucket", 1, "bucket", None, None).unwrap();
        // Give the account a balance.
        create_balance_update(&conn, account_id, 20000, "2024-01-01", None).unwrap();
        // Allocate 3000 from account and 5000 from asset to the same bucket.
        create_bucket_allocation(&conn, bucket_id, account_id, 3000, "2024-01-01").unwrap();
        create_bucket_allocation(&conn, bucket_id, asset_id, 5000, "2024-01-01").unwrap();

        let snapshot = get_accounts_snapshot(&conn, "2024-12-31T23:59:59").unwrap();
        let bucket = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();

        // Only the asset-sourced allocation appears in linked_allocations_from_assets_minor.
        assert_eq!(bucket.linked_allocations_from_assets_minor, 5000);
        // Total linked balance = account(3000) + asset(5000) = 8000.
        assert_eq!(bucket.linked_allocations_balance_minor, 8000);
    }

    // ---------------------------------------------------------------------------
    // Account–Asset Linking tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_set_and_list_account_asset_links() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let asset_id = create_account(&conn, "House", 1, "asset", Some(40_000_000), None).unwrap();

        // Link account to asset.
        set_account_asset_links(&conn, account_id, &[asset_id]).unwrap();

        let links = list_account_asset_links(&conn, Some(account_id)).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].account_id, account_id);
        assert_eq!(links[0].asset_id, asset_id);

        // Clear the link.
        set_account_asset_links(&conn, account_id, &[]).unwrap();
        let links_empty = list_account_asset_links(&conn, Some(account_id)).unwrap();
        assert_eq!(links_empty.len(), 0);
    }

    #[test]
    fn test_set_account_asset_links_validates_account_type() {
        let conn = initialize_in_memory().expect("DB init failed");
        let bucket_id = create_account(&conn, "Bucket", 1, "bucket", None, None).unwrap();
        let asset_id = create_account(&conn, "House", 1, "asset", Some(40_000_000), None).unwrap();

        // Attempting to link a bucket as the account side should fail.
        let result = set_account_asset_links(&conn, bucket_id, &[asset_id]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "VALIDATION");
    }

    #[test]
    fn test_set_account_asset_links_validates_asset_type() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let other_account_id =
            create_account(&conn, "Other Account", 1, "account", None, None).unwrap();

        // Attempting to link account→account should fail validation.
        let result = set_account_asset_links(&conn, account_id, &[other_account_id]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "VALIDATION");
    }

    #[test]
    fn test_snapshot_is_linked_to_asset_flag() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let asset_id = create_account(&conn, "House", 1, "asset", Some(40_000_000), None).unwrap();

        // Before linking: flag is false.
        let snap_before = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        let acc_before = snap_before
            .iter()
            .find(|r| r.account_id == account_id)
            .unwrap();
        assert!(!acc_before.is_linked_to_asset);

        // After linking: flag is true and linked_asset_ids contains the asset.
        set_account_asset_links(&conn, account_id, &[asset_id]).unwrap();
        let snap_after = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        let acc_after = snap_after
            .iter()
            .find(|r| r.account_id == account_id)
            .unwrap();
        assert!(acc_after.is_linked_to_asset);
        assert_eq!(acc_after.linked_asset_ids, vec![asset_id]);

        // Asset row should have account_id in its linked_asset_ids.
        let asset_row = snap_after
            .iter()
            .find(|r| r.account_id == asset_id)
            .unwrap();
        assert_eq!(asset_row.linked_asset_ids, vec![account_id]);
    }

    #[test]
    fn test_snapshot_linked_allocations_includes_linked_accounts() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let asset_id = create_account(&conn, "House", 1, "asset", Some(40_000_000), None).unwrap();
        let bucket_id = create_account(&conn, "Bucket", 1, "bucket", None, None).unwrap();

        create_balance_update(&conn, account_id, 20_000, "2024-01-01", None).unwrap();
        // Link the account to the asset.
        set_account_asset_links(&conn, account_id, &[asset_id]).unwrap();
        // Allocate from the linked account to the bucket.
        create_bucket_allocation(&conn, bucket_id, account_id, 5_000, "2024-01-01").unwrap();

        let snap = get_accounts_snapshot(&conn, "2024-12-31T23:59:59").unwrap();
        let bucket = snap.iter().find(|r| r.account_id == bucket_id).unwrap();
        // Linked account treated same as asset — counted in linked_allocations_from_assets_minor.
        assert_eq!(bucket.linked_allocations_from_assets_minor, 5_000);
    }

    #[test]
    fn test_delete_asset_cascades_links() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let asset_id = create_account(&conn, "House", 1, "asset", Some(40_000_000), None).unwrap();
        set_account_asset_links(&conn, account_id, &[asset_id]).unwrap();

        // Verify link exists.
        let links_before = list_account_asset_links(&conn, Some(account_id)).unwrap();
        assert_eq!(links_before.len(), 1);

        // Delete the asset.
        delete_account(&conn, asset_id).unwrap();

        // Link should be gone via CASCADE.
        let links_after = list_account_asset_links(&conn, Some(account_id)).unwrap();
        assert_eq!(links_after.len(), 0);
    }

    #[test]
    fn test_delete_account_cascades_links() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let asset_id = create_account(&conn, "House", 1, "asset", Some(40_000_000), None).unwrap();
        set_account_asset_links(&conn, account_id, &[asset_id]).unwrap();

        // Delete the account.
        delete_account(&conn, account_id).unwrap();

        // Link should be gone via CASCADE.
        let links: Vec<AccountAssetLink> = conn
            .prepare("SELECT id, account_id, 'x', asset_id, 'y' FROM account_asset_link WHERE asset_id = ?1")
            .unwrap()
            .query_map(params![asset_id], |row| {
                Ok(AccountAssetLink {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    account_name: row.get(2)?,
                    asset_id: row.get(3)?,
                    asset_name: row.get(4)?,
                })
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(links.len(), 0);
    }
}

#[cfg(test)]
mod convert_balance_tests {
    use super::convert_balance;

    // Test 1: 1:1 identity — mantissa=1, exponent=0, same minor_units
    #[test]
    fn identity_same_minor_units() {
        assert_eq!(convert_balance(100_000, 1, 0, 2, 2), 100_000);
    }

    // Test 2: EUR→USD standard (same minor_units=2)
    // Rate: 1 EUR = 1.0842 USD (mantissa=10842, exponent=-4)
    // Balance: 108420 USD cents (1084.20 USD) → 100000 EUR cents (1000.00 EUR)
    #[test]
    fn eur_to_usd_same_minor_units() {
        assert_eq!(convert_balance(108420, 10842, -4, 2, 2), 100_000);
    }

    // Test 3: EUR→JPY (source has minor_units=0, dest minor_units=2)
    // Rate: 1 EUR = 157.23 JPY (mantissa=15723, exponent=-2)
    // Balance: 15723 JPY (no minor units) → 10000 EUR cents (100.00 EUR)
    #[test]
    fn eur_to_jpy_different_minor_units() {
        // exp_adj = 2 + 2 - 0 = 4; converted = 15723 * 10000 / 15723 = 10000
        assert_eq!(convert_balance(15723, 15723, -2, 0, 2), 10_000);
    }

    // Test 4: EUR→BTC (source has minor_units=8)
    // Rate: 1 EUR = 0.0000161 BTC (mantissa=161, exponent=-7)
    // Balance: 50_000_000 satoshi (0.5 BTC) → ~3_105_590 EUR cents (~31 055.90 EUR)
    #[test]
    fn eur_to_btc_different_minor_units() {
        // exp_adj = 7 + 2 - 8 = 1; converted = 50_000_000 * 10 / 161 ≈ 3_105_590
        assert_eq!(convert_balance(50_000_000, 161, -7, 8, 2), 3_105_590);
    }

    // Test 5: zero balance always yields zero
    #[test]
    fn zero_balance() {
        assert_eq!(convert_balance(0, 10842, -4, 2, 2), 0);
    }

    // Test 6: negative balance (exact case to avoid rounding ambiguity)
    // mantissa=1 means 1:1 rate → negative passthrough
    #[test]
    fn negative_balance_identity_rate() {
        assert_eq!(convert_balance(-50_000, 1, 0, 2, 2), -50_000);
    }

    // Test 7: large amount — verify i128 intermediates don't overflow
    // Balance: 1_000_000_000_000 USD cents (10 billion USD), rate 1 EUR = 1.0842 USD
    #[test]
    fn large_amount_no_overflow() {
        let result = convert_balance(1_000_000_000_000, 10842, -4, 2, 2);
        // 10^12 * 10^4 / 10842 ≈ 922_528_128_581
        assert!(result > 900_000_000_000);
        assert!(result < 1_000_000_000_000);
    }

    // Test 8: exp_adj < 0 (hyperinflation — 1 EUR = 36 500 VES, minor_units=2 both)
    // mantissa=365, exponent=2 → rate = 36500
    // Balance: 3_650_000 VES minor (36 500.00 VES) → 100 EUR cents (1.00 EUR)
    #[test]
    fn exp_adj_negative_hyperinflation() {
        // exp_adj = -2 + 2 - 2 = -2; denom = 365*100 = 36500
        // (3_650_000 + 18250) / 36500 = 100
        assert_eq!(convert_balance(3_650_000, 365, 2, 2, 2), 100);
    }

    // Test 9: rounding — half-up for positive remainder
    // 3 units to convert when rate is 2:1 → 1 (floor) or 2 (round)?
    // 3 USD minor * factor / 2 → 3/2 = 1.5 → rounds to 2 (half-up)
    #[test]
    fn rounding_half_up() {
        // 3 USD minor (minor_units=2), rate 1 EUR = 2 USD (mantissa=2, exponent=0)
        // exp_adj = 0; (3 * 1 + 1) / 2 = 4/2 = 2
        assert_eq!(convert_balance(3, 2, 0, 2, 2), 2);
    }
}
