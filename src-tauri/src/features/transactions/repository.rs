use crate::features::assets::repository::get_all_account_asset_link_ids;
use crate::features::buckets::repository::{get_account_allocated_total, list_bucket_allocations};
use crate::features::buckets::AllocationDetail;
use crate::features::currency::repository::{
    get_consolidation_currency, get_fx_rate_for_conversion,
};
use crate::shared::{convert_balance, local_now, with_savepoint};
use rusqlite::{params, Connection, OptionalExtension};

use super::models::{EventWithData, SnapshotRow};

type SnapshotRawRow = (i64, String, String, i64, String, i64, i64, i64);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::create_account;
    use crate::features::currency::repository::set_fx_rate_manual;

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
}
