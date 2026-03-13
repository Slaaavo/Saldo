use crate::error::AppError;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};

use super::models::AccountAssetLink;
use crate::features::currency::Currency;

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
pub(crate) fn store_asset_price(
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

    let consolidation = crate::features::currency::repository::get_consolidation_currency(conn)
        .map_err(AppError::from)?;
    let (rate_mantissa, rate_exponent) = invert_price_str(price_str)?;

    crate::features::currency::repository::set_fx_rate_manual(
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

    crate::shared::with_savepoint_app(conn, || {
        if let Some(amount) = amount_minor {
            crate::features::transactions::repository::create_balance_update_inner(
                conn, account_id, amount, event_date, note,
            )
            .map_err(AppError::from)?;
        }

        if let Some(price_str) = price_per_unit {
            store_asset_price(conn, account_id, price_str, event_date)?;
        }

        Ok(())
    })
}

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
    crate::shared::with_savepoint_app(conn, || {
        // Validate that account_id refers to an account-type row.
        let account_type: Option<String> =
            crate::features::accounts::repository::get_account_type(conn, account_id)?;
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
            let asset_type: Option<String> =
                crate::features::accounts::repository::get_account_type(conn, aid)?;
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
pub(crate) fn get_all_account_asset_link_ids(
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
    use crate::features::accounts::repository::create_account;
    use crate::features::buckets::repository::create_bucket_allocation;
    use crate::features::transactions::repository::{create_balance_update, get_accounts_snapshot};

    fn mk_account(conn: &Connection) -> i64 {
        create_account(conn, "Test Account", 1, "account", None, None)
            .expect("create account failed")
    }

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
        crate::features::accounts::repository::delete_account(&conn, asset_id).unwrap();

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
        crate::features::accounts::repository::delete_account(&conn, account_id).unwrap();

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
