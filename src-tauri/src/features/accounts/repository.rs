use crate::error::AppError;
use crate::shared::{
    is_duplicate_iban_error, local_now, validate_iban, with_savepoint, with_savepoint_app,
};
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Default)]
pub struct CreateAccountParams {
    pub name: String,
    pub currency_id: i64,
    pub account_type: String,
    pub initial_balance_minor: Option<i64>,
    pub price_per_unit: Option<String>,
    pub iban: Option<String>,
    pub person_id: Option<i64>,
    pub purchase_price_minor: Option<i64>,
    pub purchase_date: Option<String>,
    pub depreciation_period_months: Option<i64>,
}

pub struct UpdateAccountParams {
    pub account_id: i64,
    pub name: String,
    pub iban: Option<String>,
    pub person_id: Option<i64>,
    pub purchase_price_minor: Option<i64>,
    pub purchase_date: Option<String>,
    pub depreciation_period_months: Option<i64>,
}

pub struct UpdateSortOrderParams {
    pub updates: Vec<(i64, i64)>,
}

pub fn create_account(conn: &Connection, params: CreateAccountParams) -> Result<i64, AppError> {
    let normalised_iban: Option<String> = if let Some(raw) = params.iban.as_deref() {
        if raw.is_empty() {
            None
        } else {
            Some(validate_iban(raw)?)
        }
    } else {
        None
    };
    with_savepoint_app(conn, || {
        let next_sort_order: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM account WHERE account_type = ?1",
            params![params.account_type.as_str()],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO account (name, currency_id, account_type, sort_order, iban, person_id, purchase_price_minor, purchase_date, depreciation_period_months) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![params.name.as_str(), params.currency_id, params.account_type.as_str(), next_sort_order, normalised_iban, params.person_id, params.purchase_price_minor, params.purchase_date.as_deref(), params.depreciation_period_months],
        ).map_err(|e| {
            if is_duplicate_iban_error(&e) {
                AppError {
                    code: "DUPLICATE_IBAN".into(),
                    message: "This IBAN is already in use by another account or partner.".into(),
                }
            } else {
                AppError::from(e)
            }
        })?;
        let account_id = conn.last_insert_rowid();

        if let Some(amount) = params.initial_balance_minor {
            let now = local_now();
            crate::features::transactions::repository::create_balance_update_inner(
                conn,
                crate::features::transactions::repository::CreateBalanceUpdateParams {
                    account_id,
                    amount_minor: amount,
                    event_date: now,
                    note: None,
                },
            )?;
        }

        if let Some(price) = params.price_per_unit.as_deref() {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            crate::features::assets::repository::store_asset_price(
                conn, account_id, price, &today,
            )?;
        }

        Ok(account_id)
    })
}

pub fn update_sort_order(conn: &Connection, params: UpdateSortOrderParams) -> rusqlite::Result<()> {
    with_savepoint(conn, || {
        for &(account_id, sort_order) in &params.updates {
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

pub fn update_account(conn: &Connection, params: UpdateAccountParams) -> Result<(), AppError> {
    if params.iban.is_some() {
        let account_type = get_account_type(conn, params.account_id)
            .map_err(AppError::from)?
            .unwrap_or_default();
        if account_type != "account" {
            return Err(AppError {
                code: "VALIDATION_ERROR".into(),
                message: "IBAN can only be set on accounts.".into(),
            });
        }
    }
    match params.iban.as_deref() {
        Some(raw) if !raw.is_empty() => {
            let normalised = validate_iban(raw)?;
            let rows = conn
                .execute(
                    "UPDATE account SET name = ?1, iban = ?2, person_id = ?3, purchase_price_minor = ?4, purchase_date = ?5, depreciation_period_months = ?6 WHERE id = ?7",
                    params![
                        params.name.as_str(),
                        normalised,
                        params.person_id,
                        params.purchase_price_minor,
                        params.purchase_date.as_deref(),
                        params.depreciation_period_months,
                        params.account_id
                    ],
                )
                .map_err(|e| {
                    if is_duplicate_iban_error(&e) {
                        AppError {
                            code: "DUPLICATE_IBAN".into(),
                            message: "This IBAN is already in use by another account or partner."
                                .into(),
                        }
                    } else {
                        AppError::from(e)
                    }
                })?;
            if rows == 0 {
                return Err(AppError::from(rusqlite::Error::QueryReturnedNoRows));
            }
        }
        Some(_empty) => {
            // Empty string — clear the IBAN
            let rows = conn
                .execute(
                    "UPDATE account SET name = ?1, iban = NULL, person_id = ?2, purchase_price_minor = ?3, purchase_date = ?4, depreciation_period_months = ?5 WHERE id = ?6",
                    params![params.name.as_str(), params.person_id, params.purchase_price_minor, params.purchase_date.as_deref(), params.depreciation_period_months, params.account_id],
                )
                .map_err(AppError::from)?;
            if rows == 0 {
                return Err(AppError::from(rusqlite::Error::QueryReturnedNoRows));
            }
        }
        None => {
            // No IBAN update
            let rows = conn
                .execute(
                    "UPDATE account SET name = ?1, person_id = ?2, purchase_price_minor = ?3, purchase_date = ?4, depreciation_period_months = ?5 WHERE id = ?6",
                    params![params.name.as_str(), params.person_id, params.purchase_price_minor, params.purchase_date.as_deref(), params.depreciation_period_months, params.account_id],
                )
                .map_err(AppError::from)?;
            if rows == 0 {
                return Err(AppError::from(rusqlite::Error::QueryReturnedNoRows));
            }
        }
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

        // Check if this account is currently actively linked to any bucket.
        // "Currently linked" means it appears in the latest non-deleted balance_update
        // event for at least one bucket.
        let mut check_stmt = conn
            .prepare(
                "SELECT DISTINCT a.name
                 FROM bucket_event_link bel
                 JOIN event      e   ON e.id  = bel.event_id
                 JOIN event_data ed  ON ed.id = e.latest_data_id
                 JOIN account    a   ON a.id  = e.account_id
                 WHERE bel.source_account_id = ?1
                   AND e.deleted_at          IS NULL
                   AND e.event_type          = 'balance_update'
                   AND e.id = (
                       SELECT e2.id
                       FROM   event e2
                       JOIN   event_data ed2 ON ed2.id = e2.latest_data_id
                       WHERE  e2.account_id  = e.account_id
                         AND  e2.event_type  = 'balance_update'
                         AND  e2.deleted_at  IS NULL
                       ORDER BY ed2.event_date DESC, e2.created_at DESC
                       LIMIT 1
                   )",
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
                    "Cannot delete account: it is currently linked to buckets: {}. Unlink it first.",
                    active_bucket_names.join(", ")
                ),
            });
        }

        // Remove any historical bucket_event_link rows referencing this account
        // (from past events where it was once a source). These are cleaned before
        // hard-deleting the account to avoid the RESTRICT FK violation.
        conn.execute(
            "DELETE FROM bucket_event_link WHERE source_account_id = ?1",
            params![account_id],
        )
        .map_err(AppError::from)?;

        // A transfer leg on another account keeps its amount as a plain cashflow, so
        // that account's balance history does not change.
        conn.execute(
            "UPDATE event SET event_type = 'cashflow', linked_event_id = NULL
             WHERE account_id != ?1
               AND linked_event_id IN (SELECT id FROM event WHERE account_id = ?1)",
            params![account_id],
        )
        .map_err(AppError::from)?;

        conn.execute(
            "DELETE FROM taxable_cashflow_link
             WHERE cashflow_event_id IN (SELECT id FROM event WHERE account_id = ?1)
                OR taxable_event_id  IN (SELECT id FROM event WHERE account_id = ?1)",
            params![account_id],
        )
        .map_err(AppError::from)?;

        // An asset's depreciation events live on the person's default expense account.
        crate::features::transactions::repository::depreciation::delete_system_events_for_asset(
            conn, account_id,
        )?;

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

/// Return the currency id of the account with the given `id`.
pub fn get_account_currency_id(conn: &Connection, account_id: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT currency_id FROM account WHERE id = ?1",
        params![account_id],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::assets::repository::{create_custom_unit, CreateCustomUnitParams};
    use crate::features::buckets::repository::{set_bucket_event_links, SetBucketEventLinksParams};
    use crate::features::currency::repository::{set_fx_rate_manual, SetFxRateManualParams};
    use crate::features::persons::repository::resolve_default_taxable_account;
    use crate::features::transactions::repository::depreciation::generate_depreciation_events;
    use crate::features::transactions::repository::taxable_links::{
        link_cashflows_to_taxable, LinkCashflowsParams,
    };
    use crate::features::transactions::repository::{
        create_balance_update, create_cashflow, create_taxable_event, create_transfer,
        get_accounts_snapshot, list_events, CashflowEntry, CreateBalanceUpdateParams,
        CreateTaxableEventParams, GetSnapshotParams, ListEventsQuery,
    };

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

    #[test]
    fn delete_account_cascades_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 5000,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        delete_account(&conn, account_id).unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(snapshot.iter().all(|r| r.account_id != account_id));
        let result = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(account_id),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.events.len(), 0);
    }

    fn mk_person_account(conn: &Connection, name: &str, account_type: &str) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: name.to_owned(),
                currency_id: 1,
                account_type: account_type.to_owned(),
                person_id: Some(1),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn balance_at(conn: &Connection, account_id: i64, datetime: &str) -> i64 {
        get_accounts_snapshot(
            conn,
            GetSnapshotParams {
                selected_datetime: datetime.to_owned(),
                ..Default::default()
            },
        )
        .unwrap()
        .into_iter()
        .find(|r| r.account_id == account_id)
        .expect("account missing from snapshot")
        .balance_minor
    }

    #[test]
    fn delete_account_turns_other_transfer_leg_into_cashflow() {
        let conn = initialize_in_memory().expect("DB init failed");
        let deleted_id = mk_person_account(&conn, "Checking", "account");
        let other_id = mk_person_account(&conn, "Savings", "account");
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: other_id,
                amount_minor: 10_000,
                event_date: "2026-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let (_, counterpart_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: deleted_id,
                amount_minor: -3_000,
                event_date: "2026-02-01T12:00:00".to_owned(),
                note: None,
                counterpart_account_id: Some(other_id),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        let balance_before = balance_at(&conn, other_id, "2026-12-31T23:59:59");
        assert_eq!(balance_before, 13_000);

        delete_account(&conn, deleted_id).expect("delete_account failed");

        let (event_type, linked_event_id): (String, Option<i64>) = conn
            .query_row(
                "SELECT event_type, linked_event_id FROM event WHERE id = ?1",
                params![counterpart_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(event_type, "cashflow");
        assert_eq!(linked_event_id, None);
        assert_eq!(
            balance_at(&conn, other_id, "2026-12-31T23:59:59"),
            balance_before
        );
    }

    #[test]
    fn delete_account_removes_taxable_links_to_its_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_person_account(&conn, "Checking", "account");
        let cashflow_id = create_cashflow(
            &conn,
            &CashflowEntry {
                account_id,
                amount_minor: 5_000,
                event_date: "2026-02-01T12:00:00".to_owned(),
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
        let revenue_account_id = resolve_default_taxable_account(&conn, 1, "revenue").unwrap();
        let revenue_id = create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id: revenue_account_id,
                event_type: "revenue".to_owned(),
                amount_minor: 5_000,
                event_date: "2026-02-01T00:00:00".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        link_cashflows_to_taxable(
            &conn,
            LinkCashflowsParams {
                taxable_event_id: revenue_id,
                cashflow_event_ids: vec![cashflow_id],
            },
        )
        .unwrap();

        delete_account(&conn, account_id).expect("delete_account failed");

        let link_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM taxable_cashflow_link WHERE taxable_event_id = ?1",
                params![revenue_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(link_count, 0);
        let revenue_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event WHERE id = ?1 AND deleted_at IS NULL",
                params![revenue_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(revenue_count, 1, "the revenue itself must survive");
    }

    #[test]
    fn delete_asset_removes_its_depreciation_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Laptop".to_owned(),
                currency_id: 1,
                account_type: "asset".to_owned(),
                person_id: Some(1),
                purchase_price_minor: Some(120_000),
                purchase_date: Some("2024-01-01".to_owned()),
                depreciation_period_months: Some(12),
                ..Default::default()
            },
        )
        .unwrap();
        generate_depreciation_events(&conn).unwrap();
        let count_depreciation = |conn: &Connection| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM event WHERE linked_asset_id = ?1",
                params![asset_id],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(count_depreciation(&conn), 12);

        delete_account(&conn, asset_id).expect("delete_account failed");

        assert_eq!(count_depreciation(&conn), 0);
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
        let unit_id = create_custom_unit(
            &conn,
            CreateCustomUnitParams {
                name: "TSLA".to_owned(),
                minor_units: 4,
            },
        )
        .unwrap();

        // Create an asset account denominated in this custom unit
        let account_id = create_account(
            &conn,
            CreateAccountParams {
                name: "TSLA Asset".to_owned(),
                currency_id: unit_id,
                account_type: "asset".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        // Store a balance event for the asset
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 10_000,
                event_date: "2026-03-11".to_owned(),
                note: None,
            },
        )
        .unwrap();

        // Store an fx_rate for the custom unit (EUR → TSLA)
        let eur_id: i64 = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |row| {
                row.get(0)
            })
            .unwrap();
        set_fx_rate_manual(
            &conn,
            SetFxRateManualParams {
                from_currency_id: eur_id,
                to_currency_id: unit_id,
                date: "2026-03-11".to_owned(),
                rate_mantissa: 1,
                rate_exponent: -2,
                is_direct: true,
            },
        )
        .unwrap();

        // Delete the account
        delete_account(&conn, account_id).unwrap();

        // Account must be gone from snapshot
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(snapshot.iter().all(|r| r.account_id != account_id));

        // Events must be gone
        let result = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(account_id),
                ..Default::default()
            },
        )
        .unwrap();
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
        let account_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Savings".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                initial_balance_minor: Some(10000),
                ..Default::default()
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let row = snapshot
            .iter()
            .find(|r| r.account_id == account_id)
            .unwrap();
        assert_eq!(row.balance_minor, 10000);
    }

    #[test]
    fn test_delete_account_blocked_by_bucket_link() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Emergency Reserve".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: source_id,
                amount_minor: 10000,
                event_date: "2024-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        // Create a bucket balance update event that links the source account.
        let event_id = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: bucket_id,
                amount_minor: 0,
                event_date: "2024-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        set_bucket_event_links(
            &conn,
            SetBucketEventLinksParams {
                event_id,
                account_ids: vec![source_id],
            },
        )
        .unwrap();

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
    fn test_delete_bucket_cascades_links() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Cascade Bucket".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: source_id,
                amount_minor: 10000,
                event_date: "2024-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        // Create a bucket balance update event that links the source account.
        let event_id = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: bucket_id,
                amount_minor: 0,
                event_date: "2024-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        set_bucket_event_links(
            &conn,
            SetBucketEventLinksParams {
                event_id,
                account_ids: vec![source_id],
            },
        )
        .unwrap();

        // Deleting the bucket hard-deletes its events, which cascade to bucket_event_link rows.
        delete_account(&conn, bucket_id).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bucket_event_link WHERE event_id = ?1",
                params![event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "bucket_event_link rows should be removed by CASCADE on event deletion"
        );
    }

    #[test]
    fn delete_account_succeeds_after_unlinking_from_bucket() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Savings Pot".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: source_id,
                amount_minor: 10000,
                event_date: "2024-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();

        // Event 1: link the source account.
        let event1 = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: bucket_id,
                amount_minor: 0,
                event_date: "2024-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        set_bucket_event_links(
            &conn,
            SetBucketEventLinksParams {
                event_id: event1,
                account_ids: vec![source_id],
            },
        )
        .unwrap();

        // Event 2 (newer): no links — this is the "unlink".
        let event2 = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: bucket_id,
                amount_minor: 0,
                event_date: "2024-06-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        set_bucket_event_links(
            &conn,
            SetBucketEventLinksParams {
                event_id: event2,
                account_ids: vec![],
            },
        )
        .unwrap();

        // After unlinking via a newer event, delete should succeed.
        let result = delete_account(&conn, source_id);
        assert!(
            result.is_ok(),
            "delete_account failed after unlinking: {:?}",
            result.unwrap_err()
        );

        // All bucket_event_link rows for this source must be gone.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bucket_event_link WHERE source_account_id = ?1",
                params![source_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "all link rows should have been deleted");

        // The account itself must be gone.
        let snap = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(snap.iter().all(|r| r.account_id != source_id));
    }

    #[test]
    fn delete_account_fails_when_still_linked_to_bucket() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn);
        let bucket_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Active Reserve".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: source_id,
                amount_minor: 10000,
                event_date: "2024-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();

        // Link account without creating a newer event to unlink.
        let event_id = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: bucket_id,
                amount_minor: 0,
                event_date: "2024-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        set_bucket_event_links(
            &conn,
            SetBucketEventLinksParams {
                event_id,
                account_ids: vec![source_id],
            },
        )
        .unwrap();

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
        let id1 = create_account(
            &conn,
            CreateAccountParams {
                name: "First Bucket".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let id2 = create_account(
            &conn,
            CreateAccountParams {
                name: "Second Bucket".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
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
        let alpha_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Alpha".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let beta_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Beta".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        // Alpha gets sort_order=0, Beta gets sort_order=1 — swap them.
        update_sort_order(
            &conn,
            UpdateSortOrderParams {
                updates: vec![(beta_id, 0), (alpha_id, 1)],
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
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
        let result = update_sort_order(
            &conn,
            UpdateSortOrderParams {
                updates: vec![(9999, 0)],
            },
        );
        assert!(result.is_err());
    }
}
