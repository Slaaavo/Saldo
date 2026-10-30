use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension};

use super::models::{BatchSummary, ChangedRow};
use crate::error::AppError;
use crate::features::transactions::SourceRef;

pub(super) struct CreateBatchParams {
    pub label: String,
    pub document_count: i64,
}

pub(super) fn create_batch(conn: &Connection, params: CreateBatchParams) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO entry_batch (label, document_count) VALUES (?1, ?2)",
        params![params.label, params.document_count],
    )?;
    Ok(conn.last_insert_rowid())
}

pub(super) enum Root {
    Event(i64),
    SplitGroup(i64),
    AssetAccount {
        account_id: i64,
        source_ref: SourceRef,
    },
}

pub(super) struct StampParams {
    pub batch_id: i64,
    pub root: Root,
}

pub(super) fn stamp(conn: &Connection, params: StampParams) -> Result<(), AppError> {
    let batch_id = params.batch_id;
    match params.root {
        Root::Event(event_id) => {
            conn.execute(
                "UPDATE event SET batch_id = ?1 WHERE id = ?2",
                params![batch_id, event_id],
            )?;
        }
        Root::SplitGroup(split_group_id) => {
            conn.execute(
                "UPDATE event SET batch_id = ?1 WHERE split_group_id = ?2",
                params![batch_id, split_group_id],
            )?;
        }
        Root::AssetAccount {
            account_id,
            source_ref,
        } => {
            conn.execute(
                "UPDATE account SET batch_id = ?1, source_ref = ?2 WHERE id = ?3",
                params![batch_id, source_ref.as_str(), account_id],
            )?;
            conn.execute(
                "UPDATE event SET batch_id = ?1 WHERE account_id = ?2",
                params![batch_id, account_id],
            )?;
        }
    }
    Ok(())
}

pub(super) fn count_prepaid_shares(
    conn: &Connection,
    parent_event_id: i64,
) -> Result<usize, AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM event WHERE linked_prepaid_event_id = ?1 AND deleted_at IS NULL",
        params![parent_event_id],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

pub(super) fn list_recent_batches(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<BatchSummary>, AppError> {
    let batches = conn
        .prepare(
            "SELECT id, label, document_count, created_at, undone_at
             FROM entry_batch ORDER BY id DESC LIMIT ?1",
        )?
        .query_map(params![limit], |row| {
            Ok(BatchSummary {
                id: row.get(0)?,
                label: row.get(1)?,
                document_count: row.get(2)?,
                created_at: row.get(3)?,
                undone_at: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(batches)
}

pub(super) fn find_batch_undone(
    conn: &Connection,
    batch_id: i64,
) -> Result<Option<bool>, AppError> {
    Ok(conn
        .query_row(
            "SELECT undone_at IS NOT NULL FROM entry_batch WHERE id = ?1",
            params![batch_id],
            |row| row.get(0),
        )
        .optional()?)
}

pub(super) fn mark_undone(conn: &Connection, batch_id: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE entry_batch
         SET undone_at = strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime')
         WHERE id = ?1",
        params![batch_id],
    )?;
    Ok(())
}

pub(super) struct RootEntry {
    pub event_id: i64,
    pub split_group_id: Option<i64>,
    pub deleted: bool,
}

pub(super) fn list_root_entries(
    conn: &Connection,
    batch_id: i64,
) -> Result<Vec<RootEntry>, AppError> {
    let entries = conn
        .prepare(
            "SELECT e.id, e.split_group_id, e.deleted_at IS NOT NULL
             FROM event e
             JOIN account a ON a.id = e.account_id
             WHERE e.batch_id = ?1 AND a.batch_id IS NOT ?1
             ORDER BY e.id",
        )?
        .query_map(params![batch_id], |row| {
            Ok(RootEntry {
                event_id: row.get(0)?,
                split_group_id: row.get(1)?,
                deleted: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(entries)
}

pub(super) fn list_batch_asset_ids(conn: &Connection, batch_id: i64) -> Result<Vec<i64>, AppError> {
    let ids = conn
        .prepare("SELECT id FROM account WHERE batch_id = ?1 ORDER BY id")?
        .query_map(params![batch_id], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

/// Records each asset's fields and each payment link as the batch made them, for undo to
/// compare with. Runs once, after every document of the batch was written.
pub(super) fn record_what_the_batch_made(conn: &Connection, batch_id: i64) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO entry_batch_asset
             (account_id, name, purchase_price_minor, purchase_date, depreciation_period_months)
         SELECT id, name, purchase_price_minor, purchase_date, depreciation_period_months
         FROM account WHERE batch_id = ?1",
        params![batch_id],
    )?;
    conn.execute(
        "INSERT INTO entry_batch_link (batch_id, taxable_event_id, cashflow_event_id)
         SELECT ?1, tcl.taxable_event_id, tcl.cashflow_event_id
         FROM taxable_cashflow_link tcl
         JOIN event e ON e.id = tcl.taxable_event_id
         WHERE e.batch_id = ?1",
        params![batch_id],
    )?;
    Ok(())
}

/// The person and year of every live row the batch wrote, or its undo removes: entries and
/// split legs, prepaid shares, the asset accounts' events and their depreciation.
pub(super) fn list_person_years(
    conn: &Connection,
    batch_id: i64,
) -> Result<BTreeSet<(i64, i64)>, AppError> {
    let years = conn
        .prepare(
            "SELECT DISTINCT a.person_id, CAST(substr(ed.event_date, 1, 4) AS INTEGER)
             FROM event e
             JOIN account a ON a.id = e.account_id
             JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.deleted_at IS NULL AND a.person_id IS NOT NULL
               AND (e.batch_id = ?1
                    OR e.split_group_id IN (SELECT split_group_id FROM event WHERE batch_id = ?1)
                    OR e.linked_prepaid_event_id IN (SELECT id FROM event WHERE batch_id = ?1)
                    OR e.account_id IN (SELECT id FROM account WHERE batch_id = ?1)
                    OR e.linked_asset_id IN (SELECT id FROM account WHERE batch_id = ?1))",
        )?
        .query_map(params![batch_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(years)
}

/// Depreciation rows Saldo regenerates live on the expense account with no batch, so they are
/// never a change.
pub(super) fn list_changed_rows(
    conn: &Connection,
    batch_id: i64,
) -> Result<Vec<ChangedRow>, AppError> {
    let rows = conn
        .prepare(
            "SELECT 0, e.id, NULL, NULL FROM event e
             WHERE e.batch_id = ?1 AND e.deleted_at IS NULL
               AND (SELECT COUNT(*) FROM event_data ed WHERE ed.event_id = e.id) > 1
             UNION ALL
             SELECT 1, e.id, NULL, NULL FROM event e
             WHERE e.deleted_at IS NULL AND e.batch_id IS NULL
               AND e.split_group_id IN (
                   SELECT split_group_id FROM event
                   WHERE batch_id = ?1 AND split_group_id IS NOT NULL
               )
             UNION ALL
             SELECT 2, e.id, a.id, NULL FROM event e
             JOIN account a ON a.id = e.account_id
             WHERE a.batch_id = ?1 AND e.deleted_at IS NULL AND e.batch_id IS NOT ?1
             UNION ALL
             SELECT 3, NULL, a.id, NULL FROM account a
             JOIN entry_batch_asset made ON made.account_id = a.id
             WHERE a.batch_id = ?1
               AND (a.name IS NOT made.name
                    OR a.purchase_price_minor IS NOT made.purchase_price_minor
                    OR a.purchase_date IS NOT made.purchase_date
                    OR a.depreciation_period_months IS NOT made.depreciation_period_months)
             UNION ALL
             SELECT 4, tcl.taxable_event_id, NULL, tcl.cashflow_event_id
             FROM taxable_cashflow_link tcl
             JOIN event e ON e.id = tcl.taxable_event_id
             WHERE e.batch_id = ?1 AND e.deleted_at IS NULL
               AND NOT EXISTS (
                   SELECT 1 FROM entry_batch_link made
                   WHERE made.taxable_event_id = tcl.taxable_event_id
                     AND made.cashflow_event_id = tcl.cashflow_event_id
               )
             UNION ALL
             SELECT 5, made.taxable_event_id, NULL, made.cashflow_event_id
             FROM entry_batch_link made
             JOIN event e ON e.id = made.taxable_event_id
             WHERE made.batch_id = ?1 AND e.deleted_at IS NULL
               AND NOT EXISTS (
                   SELECT 1 FROM taxable_cashflow_link tcl
                   WHERE tcl.taxable_event_id = made.taxable_event_id
                     AND tcl.cashflow_event_id = made.cashflow_event_id
               )
             ORDER BY 1, 2, 3, 4",
        )?
        .query_map(params![batch_id], |row| {
            let event_id = || row.get::<_, i64>(1);
            let account_id = || row.get::<_, i64>(2);
            let payment_id = || row.get::<_, i64>(3);
            Ok(match row.get::<_, i64>(0)? {
                0 => ChangedRow::EditedInSaldo {
                    event_id: event_id()?,
                },
                1 => ChangedRow::LegAddedInSaldo {
                    event_id: event_id()?,
                },
                2 => ChangedRow::EventAddedToAsset {
                    event_id: event_id()?,
                    account_id: account_id()?,
                },
                3 => ChangedRow::AssetEditedInSaldo {
                    account_id: account_id()?,
                },
                4 => ChangedRow::PaymentLinkAddedInSaldo {
                    event_id: event_id()?,
                    payment_id: payment_id()?,
                },
                _ => ChangedRow::PaymentLinkRemovedInSaldo {
                    event_id: event_id()?,
                    payment_id: payment_id()?,
                },
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}
