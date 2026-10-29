use rusqlite::{params, Connection, OptionalExtension};

use super::models::{BatchSummary, ChangeReason, ChangedRow};
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

/// What a document wrote that carries the batch. Generated rows (prepaid shares, depreciation)
/// are never stamped: undo finds them through their parent.
pub(super) enum Root {
    Event(i64),
    SplitGroup(i64),
    /// The account and its initial balance update; the account also takes the reference.
    AssetAccount {
        account_id: i64,
        source_ref: SourceRef,
    },
}

pub(super) struct StampParams {
    pub batch_id: i64,
    pub root: Root,
}

/// The only writer of `batch_id` and of `account.source_ref`.
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

/// `None` when there is no such batch; otherwise whether it was already undone.
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

/// An entry or split leg the batch recorded, outside its own asset accounts.
pub(super) struct RootEntry {
    pub event_id: i64,
    pub split_group_id: Option<i64>,
    pub deleted: bool,
    pub person_id: i64,
    pub date: String,
}

pub(super) struct BatchAsset {
    pub account_id: i64,
    pub person_id: i64,
    pub purchase_date: Option<String>,
}

/// Events on the batch's own asset accounts go with the account, so they are not listed.
pub(super) fn list_root_entries(
    conn: &Connection,
    batch_id: i64,
) -> Result<Vec<RootEntry>, AppError> {
    let entries = conn
        .prepare(
            "SELECT e.id, e.split_group_id, e.deleted_at IS NOT NULL, a.person_id,
                    substr(ed.event_date, 1, 10)
             FROM event e
             JOIN account a ON a.id = e.account_id
             JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.batch_id = ?1 AND a.batch_id IS NOT ?1
             ORDER BY e.id",
        )?
        .query_map(params![batch_id], |row| {
            Ok(RootEntry {
                event_id: row.get(0)?,
                split_group_id: row.get(1)?,
                deleted: row.get(2)?,
                person_id: row.get(3)?,
                date: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(entries)
}

pub(super) fn list_batch_assets(
    conn: &Connection,
    batch_id: i64,
) -> Result<Vec<BatchAsset>, AppError> {
    let assets = conn
        .prepare(
            "SELECT id, person_id, purchase_date FROM account WHERE batch_id = ?1 ORDER BY id",
        )?
        .query_map(params![batch_id], |row| {
            Ok(BatchAsset {
                account_id: row.get(0)?,
                person_id: row.get(1)?,
                purchase_date: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(assets)
}

/// What the user changed in Saldo since the batch: a batch row edited (it has a second
/// `event_data` version), a leg added to one of its split groups, or an entry added to one of
/// its asset accounts. Depreciation rows are written by Saldo on the person's expense account,
/// so they never count.
pub(super) fn list_changed_rows(
    conn: &Connection,
    batch_id: i64,
) -> Result<Vec<ChangedRow>, AppError> {
    let rows = conn
        .prepare(
            "SELECT e.id, 0 FROM event e
             WHERE e.batch_id = ?1 AND e.deleted_at IS NULL
               AND (SELECT COUNT(*) FROM event_data ed WHERE ed.event_id = e.id) > 1
             UNION ALL
             SELECT e.id, 1 FROM event e
             WHERE e.deleted_at IS NULL AND e.batch_id IS NULL
               AND e.split_group_id IN (
                   SELECT split_group_id FROM event
                   WHERE batch_id = ?1 AND split_group_id IS NOT NULL
               )
             UNION ALL
             SELECT e.id, 2 FROM event e
             JOIN account a ON a.id = e.account_id
             WHERE a.batch_id = ?1 AND e.deleted_at IS NULL AND e.batch_id IS NOT ?1
             ORDER BY 1",
        )?
        .query_map(params![batch_id], |row| {
            let reason = match row.get::<_, i64>(1)? {
                0 => ChangeReason::EditedInSaldo,
                1 => ChangeReason::LegAddedInSaldo,
                _ => ChangeReason::EventAddedToAsset,
            };
            Ok(ChangedRow {
                event_id: row.get(0)?,
                reason,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}
