use crate::error::AppError;
use crate::shared::{local_now, with_savepoint, with_savepoint_app};
use rusqlite::{params, Connection, OptionalExtension};

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
            crate::features::transactions::repository::create_balance_update_inner(
                conn, account_id, amount, &now, None,
            )?;
        }

        if let Some(price) = price_per_unit {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            crate::features::assets::repository::store_asset_price(
                conn, account_id, price, &today,
            )?;
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

/// Return the account_type of the account with the given `id`, or `None` if not found.
pub fn get_account_type(conn: &Connection, account_id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT account_type FROM account WHERE id = ?1",
        params![account_id],
        |row| row.get(0),
    )
    .optional()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::assets::repository::create_custom_unit;
    use crate::features::buckets::repository::create_bucket_allocation;
    use crate::features::currency::repository::set_fx_rate_manual;
    use crate::features::transactions::repository::{
        create_balance_update, get_accounts_snapshot, list_events,
    };

    fn mk_account(conn: &Connection) -> i64 {
        create_account(conn, "Test Account", 1, "account", None, None)
            .expect("create account failed")
    }

    #[test]
    fn delete_account_cascades_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(&conn, account_id, 5000, "2026-03-01", None).unwrap();
        delete_account(&conn, account_id).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        assert!(snapshot.iter().all(|r| r.account_id != account_id));
        let result = list_events(&conn, Some(account_id), None, None, None, None).unwrap();
        assert_eq!(result.events.len(), 0);
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
        let result = list_events(&conn, Some(account_id), None, None, None, None).unwrap();
        assert_eq!(result.events.len(), 0);

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
}
