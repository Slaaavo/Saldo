use rusqlite::{params, Connection};
use std::collections::HashMap;

use super::models::{BucketLink, LinkConflict};
use crate::shared::with_savepoint;

pub fn set_bucket_event_links(
    conn: &Connection,
    event_id: i64,
    account_ids: &[i64],
) -> rusqlite::Result<()> {
    with_savepoint(conn, || {
        conn.execute(
            "DELETE FROM bucket_event_link WHERE event_id = ?1",
            params![event_id],
        )?;
        for &account_id in account_ids {
            conn.execute(
                "INSERT OR IGNORE INTO bucket_event_link (event_id, source_account_id) VALUES (?1, ?2)",
                params![event_id, account_id],
            )?;
        }
        Ok(())
    })
}

pub fn list_links_for_event(conn: &Connection, event_id: i64) -> rusqlite::Result<Vec<BucketLink>> {
    let mut stmt = conn.prepare(
        "SELECT bel.id, bel.event_id, bel.source_account_id,
                a.name         AS source_account_name,
                c.id           AS source_currency_id,
                c.code         AS source_currency_code,
                c.minor_units  AS source_currency_minor_units
         FROM bucket_event_link bel
         JOIN account  a ON a.id = bel.source_account_id
         JOIN currency c ON c.id = a.currency_id
         WHERE bel.event_id = ?1
         ORDER BY bel.id",
    )?;
    let rows = stmt.query_map(params![event_id], |row| {
        Ok(BucketLink {
            id: row.get(0)?,
            event_id: row.get(1)?,
            source_account_id: row.get(2)?,
            source_account_name: row.get(3)?,
            source_currency_id: row.get(4)?,
            source_currency_code: row.get(5)?,
            source_currency_minor_units: row.get(6)?,
        })
    })?;
    rows.collect()
}

pub fn list_latest_links_for_bucket(
    conn: &Connection,
    bucket_account_id: i64,
    as_of_date: &str,
) -> rusqlite::Result<Vec<BucketLink>> {
    let mut stmt = conn.prepare(
        "SELECT bel.id, bel.event_id, bel.source_account_id,
                a.name         AS source_account_name,
                c.id           AS source_currency_id,
                c.code         AS source_currency_code,
                c.minor_units  AS source_currency_minor_units
         FROM bucket_event_link bel
         JOIN event      e  ON e.id  = bel.event_id
         JOIN account    a  ON a.id  = bel.source_account_id
         JOIN currency   c  ON c.id  = a.currency_id
         WHERE e.account_id  = ?1
           AND e.event_type  = 'balance_update'
           AND e.deleted_at  IS NULL
           AND e.id = (
               SELECT e2.id
               FROM   event e2
               JOIN   event_data ed2 ON ed2.id = e2.latest_data_id
               WHERE  e2.account_id   = ?1
                 AND  e2.event_type   = 'balance_update'
                 AND  e2.deleted_at   IS NULL
                 AND  ed2.event_date <= ?2
               ORDER BY ed2.event_date DESC, e2.created_at DESC
               LIMIT 1
           )
         ORDER BY bel.id",
    )?;
    let rows = stmt.query_map(params![bucket_account_id, as_of_date], |row| {
        Ok(BucketLink {
            id: row.get(0)?,
            event_id: row.get(1)?,
            source_account_id: row.get(2)?,
            source_account_name: row.get(3)?,
            source_currency_id: row.get(4)?,
            source_currency_code: row.get(5)?,
            source_currency_minor_units: row.get(6)?,
        })
    })?;
    rows.collect()
}

pub fn list_all_latest_bucket_links(
    conn: &Connection,
    snapshot_date: &str,
) -> rusqlite::Result<Vec<(i64, BucketLink)>> {
    let mut stmt = conn.prepare(
        "SELECT e.account_id AS bucket_id,
                bel.id,
                bel.event_id,
                bel.source_account_id,
                a.name         AS source_account_name,
                c.id           AS source_currency_id,
                c.code         AS source_currency_code,
                c.minor_units  AS source_currency_minor_units
         FROM bucket_event_link bel
         JOIN event        e   ON e.id  = bel.event_id
         JOIN event_data   ed  ON ed.id = e.latest_data_id
         JOIN account      a   ON a.id  = bel.source_account_id
         JOIN currency     c   ON c.id  = a.currency_id
         WHERE e.event_type  = 'balance_update'
           AND e.deleted_at  IS NULL
           AND ed.event_date <= ?1
           AND e.id = (
               SELECT e2.id
               FROM   event e2
               JOIN   event_data ed2 ON ed2.id = e2.latest_data_id
               WHERE  e2.account_id     = e.account_id
                 AND  e2.event_type     = 'balance_update'
                 AND  e2.deleted_at     IS NULL
                 AND  ed2.event_date   <= ?1
               ORDER BY ed2.event_date DESC, e2.created_at DESC
               LIMIT 1
           )
         ORDER BY e.account_id, bel.id",
    )?;
    let rows = stmt.query_map(params![snapshot_date], |row| {
        let bucket_id: i64 = row.get(0)?;
        let link = BucketLink {
            id: row.get(1)?,
            event_id: row.get(2)?,
            source_account_id: row.get(3)?,
            source_account_name: row.get(4)?,
            source_currency_id: row.get(5)?,
            source_currency_code: row.get(6)?,
            source_currency_minor_units: row.get(7)?,
        };
        Ok((bucket_id, link))
    })?;
    rows.collect()
}

pub fn carry_forward_bucket_links(
    conn: &Connection,
    bucket_id: i64,
    new_event_id: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO bucket_event_link (event_id, source_account_id)
         SELECT ?1, bel.source_account_id
         FROM   bucket_event_link bel
         WHERE  bel.event_id = (
             SELECT e.id
             FROM   event e
             JOIN   event_data ed ON ed.id = e.latest_data_id
             WHERE  e.account_id    = ?2
               AND  e.event_type    = 'balance_update'
               AND  e.deleted_at    IS NULL
               AND  e.id           != ?1
             ORDER BY ed.event_date DESC, e.created_at DESC
             LIMIT 1
         )",
        params![new_event_id, bucket_id],
    )?;
    Ok(())
}

pub fn check_single_link_conflict(
    conn: &Connection,
    source_account_id: i64,
    target_bucket_id: i64,
    new_event_id: i64,
    new_event_date: &str,
) -> rusqlite::Result<Option<LinkConflict>> {
    // Query A: all non-deleted balance_update events linking source_account_id
    // across any bucket, excluding the new event being created/updated.
    let mut stmt_a = conn.prepare(
        "SELECT e.account_id AS bucket_id, a.name AS bucket_name, ed.event_date
         FROM   bucket_event_link bel
         JOIN   event      e  ON e.id  = bel.event_id
         JOIN   event_data ed ON ed.id = e.latest_data_id
         JOIN   account    a  ON a.id  = e.account_id
         WHERE  bel.source_account_id = ?1
           AND  e.deleted_at          IS NULL
           AND  e.event_type          = 'balance_update'
           AND  e.id                 != ?2
         ORDER BY ed.event_date",
    )?;
    let a_rows: Vec<(i64, String, String)> = stmt_a
        .query_map(params![source_account_id, new_event_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    if a_rows.is_empty() {
        return Ok(None);
    }

    // Collect unique bucket_ids for Query B: target first, then all from Query A.
    let mut bucket_ids: Vec<i64> = vec![target_bucket_id];
    for (bid, _, _) in &a_rows {
        if !bucket_ids.contains(bid) {
            bucket_ids.push(*bid);
        }
    }

    // Query B: all balance_update events for the relevant buckets (excluding new_event_id).
    // Build dynamic IN clause with positional parameters.
    let placeholders = std::iter::repeat_n("?", bucket_ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let query_b_sql = format!(
        "SELECT e.account_id AS bucket_id, ed.event_date
         FROM   event      e
         JOIN   event_data ed ON ed.id = e.latest_data_id
         WHERE  e.account_id IN ({})
           AND  e.event_type  = 'balance_update'
           AND  e.deleted_at  IS NULL
           AND  e.id         != ?
         ORDER BY e.account_id, ed.event_date ASC, e.created_at ASC",
        placeholders
    );
    // params: all bucket_ids first, then new_event_id at the end.
    let mut b_params: Vec<i64> = bucket_ids.clone();
    b_params.push(new_event_id);

    let mut stmt_b = conn.prepare(&query_b_sql)?;
    let b_rows: Vec<(i64, String)> = stmt_b
        .query_map(rusqlite::params_from_iter(b_params.iter()), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    // Build HashMap<bucket_id, sorted Vec<event_date>> (already sorted ASC by query).
    let mut bucket_timeline: HashMap<i64, Vec<String>> = HashMap::new();
    for (bid, edate) in &b_rows {
        bucket_timeline.entry(*bid).or_default().push(edate.clone());
    }

    // The new event's active range: [new_event_date, target_active_end).
    let infinity = "9999-12-31T23:59:59".to_string();
    let target_active_end: String = bucket_timeline
        .get(&target_bucket_id)
        .and_then(|dates| dates.iter().find(|d| d.as_str() > new_event_date))
        .cloned()
        .unwrap_or_else(|| infinity.clone());

    for (other_bucket_id, other_bucket_name, other_event_date) in &a_rows {
        if *other_bucket_id == target_bucket_id {
            // Same bucket — not a cross-bucket conflict.
            continue;
        }

        // The other link's active range: [other_event_date, other_active_end).
        let other_active_end: String = bucket_timeline
            .get(other_bucket_id)
            .and_then(|dates| {
                dates
                    .iter()
                    .find(|d| d.as_str() > other_event_date.as_str())
            })
            .cloned()
            .unwrap_or_else(|| infinity.clone());

        // Overlap condition: ranges [A, B) and [C, D) overlap iff A < D && C < B.
        if other_event_date.as_str() < target_active_end.as_str()
            && other_active_end.as_str() > new_event_date
        {
            let source_account_name: String = conn.query_row(
                "SELECT name FROM account WHERE id = ?1",
                params![source_account_id],
                |row| row.get(0),
            )?;

            let conflict_date_raw = if other_event_date.as_str() > new_event_date {
                other_event_date.as_str()
            } else {
                new_event_date
            };
            let conflict_date = conflict_date_raw[..10.min(conflict_date_raw.len())].to_string();

            return Ok(Some(LinkConflict {
                source_account_id,
                source_account_name,
                conflict_date,
                other_bucket_id: *other_bucket_id,
                other_bucket_name: other_bucket_name.clone(),
            }));
        }
    }

    Ok(None)
}

pub fn check_link_conflicts(
    conn: &Connection,
    target_bucket_id: i64,
    new_event_id: i64,
    new_event_date: &str,
    proposed_account_ids: &[i64],
) -> rusqlite::Result<Option<LinkConflict>> {
    for &source_account_id in proposed_account_ids {
        if let Some(conflict) = check_single_link_conflict(
            conn,
            source_account_id,
            target_bucket_id,
            new_event_id,
            new_event_date,
        )? {
            return Ok(Some(conflict));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::transactions::repository::create_balance_update;

    fn mk_account(conn: &Connection) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: "Test Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn mk_bucket(conn: &Connection) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: "Test Bucket".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .expect("create bucket failed")
    }

    #[test]
    fn test_no_conflict_for_single_bucket_link() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let bucket_id = mk_bucket(&conn);

        // Create first event for the bucket and link the account.
        let event1 = create_balance_update(&conn, bucket_id, 0, "2025-01-01", None).unwrap();
        set_bucket_event_links(&conn, event1, &[account_id]).unwrap();

        // Create a second event to check: same bucket, same account — not a conflict.
        let event2 = create_balance_update(&conn, bucket_id, 0, "2025-06-01", None).unwrap();
        let result =
            check_single_link_conflict(&conn, account_id, bucket_id, event2, "2025-06-01").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_conflict_when_two_buckets_same_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let bucket1 = mk_bucket(&conn);
        let bucket2 = create_account(
            &conn,
            CreateAccountParams {
                name: "Bucket 2".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        let event_b1 = create_balance_update(&conn, bucket1, 0, "2025-01-01", None).unwrap();
        set_bucket_event_links(&conn, event_b1, &[account_id]).unwrap();

        // Propose linking account to bucket2 on the same date.
        let event_b2 = create_balance_update(&conn, bucket2, 0, "2025-01-01", None).unwrap();
        let result =
            check_single_link_conflict(&conn, account_id, bucket2, event_b2, "2025-01-01").unwrap();
        assert!(result.is_some());
        let conflict = result.unwrap();
        assert_eq!(conflict.source_account_id, account_id);
        assert_eq!(conflict.other_bucket_id, bucket1);
    }

    #[test]
    fn test_no_conflict_when_buckets_use_different_date_ranges() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let bucket1 = mk_bucket(&conn);
        let bucket2 = create_account(
            &conn,
            CreateAccountParams {
                name: "Bucket 2".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        // Link account to bucket1 starting 2025-01-01.
        let event_b1_jan = create_balance_update(&conn, bucket1, 0, "2025-01-01", None).unwrap();
        set_bucket_event_links(&conn, event_b1_jan, &[account_id]).unwrap();

        // Bucket1 gets a new event on 2025-06-01 WITHOUT the account (terminates the link).
        let event_b1_jun = create_balance_update(&conn, bucket1, 0, "2025-06-01", None).unwrap();
        set_bucket_event_links(&conn, event_b1_jun, &[]).unwrap();

        // Propose linking account to bucket2 starting 2025-06-01.
        let event_b2 = create_balance_update(&conn, bucket2, 0, "2025-06-01", None).unwrap();
        let result =
            check_single_link_conflict(&conn, account_id, bucket2, event_b2, "2025-06-01").unwrap();
        assert!(
            result.is_none(),
            "expected no conflict but got {:?}",
            result
        );
    }

    #[test]
    fn test_carry_forward_copies_links() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let bucket_id = mk_bucket(&conn);

        let event1 = create_balance_update(&conn, bucket_id, 0, "2025-01-01", None).unwrap();
        set_bucket_event_links(&conn, event1, &[account_id]).unwrap();

        let event2 = create_balance_update(&conn, bucket_id, 0, "2025-06-01", None).unwrap();
        carry_forward_bucket_links(&conn, bucket_id, event2).unwrap();

        let links = list_links_for_event(&conn, event2).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].source_account_id, account_id);
    }

    #[test]
    fn test_carry_forward_noop_when_no_previous_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let bucket_id = mk_bucket(&conn);

        let event1 = create_balance_update(&conn, bucket_id, 0, "2025-01-01", None).unwrap();
        carry_forward_bucket_links(&conn, bucket_id, event1).unwrap();

        let links = list_links_for_event(&conn, event1).unwrap();
        assert!(links.is_empty());
    }
}
