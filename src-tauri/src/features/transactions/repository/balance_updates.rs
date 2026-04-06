use crate::features::buckets::repository::{
    carry_forward_bucket_links, CarryForwardBucketLinksParams,
};
use crate::shared::with_savepoint;
use rusqlite::{params, Connection, OptionalExtension};

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
                carry_forward_bucket_links(
                    conn,
                    CarryForwardBucketLinksParams {
                        bucket_id: account_id,
                        new_event_id: event_id,
                    },
                )?;
            }
            ids.push(event_id);
        }
        Ok(ids)
    })
}
