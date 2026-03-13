use rusqlite::{params, Connection, OptionalExtension};

use super::models::{AllocationDetail, BucketAllocation, OverAllocationWarning};

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

/// Check whether `source_account_id` is over-allocated as of `as_of_date` (YYYY-MM-DD).
/// Returns `Some(OverAllocationWarning)` if total allocations exceed the account balance,
/// or `None` if the account is within its balance.
pub fn check_over_allocation(
    conn: &Connection,
    source_account_id: i64,
    as_of_date: &str,
) -> rusqlite::Result<Option<OverAllocationWarning>> {
    let selected_datetime = format!("{}T23:59:59", as_of_date);
    let balance = crate::features::accounts::repository::get_account_balance_at_date(
        conn,
        source_account_id,
        &selected_datetime,
    )?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::create_account;
    use crate::features::transactions::repository::{create_balance_update, get_accounts_snapshot};

    fn mk_account(conn: &Connection) -> i64 {
        create_account(conn, "Test Account", 1, "account", None, None)
            .expect("create account failed")
    }

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
}
