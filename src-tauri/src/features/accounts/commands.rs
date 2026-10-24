use crate::error::AppError;
use crate::features::currency::repository::get_consolidation_currency;
use crate::shared::{is_iso_date, with_savepoint_app};
use crate::AppState;
use rusqlite::Connection;
use serde::Deserialize;
use tauri::State;

use super::repository;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccountInput {
    pub name: String,
    pub currency_id: i64,
    pub account_type: Option<String>,
    pub initial_balance_minor: Option<i64>,
    pub price_per_unit: Option<String>,
    pub linked_asset_ids: Option<Vec<i64>>,
    pub iban: Option<String>,
    pub person_id: Option<i64>,
    pub purchase_price_minor: Option<i64>,
    pub purchase_date: Option<String>,
    pub depreciation_period_months: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountInput {
    pub account_id: i64,
    pub name: String,
    pub iban: Option<String>,
    pub person_id: Option<i64>,
    pub purchase_price_minor: Option<i64>,
    pub purchase_date: Option<String>,
    pub depreciation_period_months: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortOrderEntry {
    pub account_id: i64,
    pub sort_order: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSortOrderInput {
    pub entries: Vec<SortOrderEntry>,
}

#[tauri::command]
pub fn create_account(
    state: State<'_, AppState>,
    input: CreateAccountInput,
) -> Result<i64, AppError> {
    let conn = state.conn()?;
    create_account_with_conn(&conn, input)
}

fn create_account_with_conn(conn: &Connection, input: CreateAccountInput) -> Result<i64, AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Account name is required".into(),
        });
    }
    let account_type = input.account_type.as_deref().unwrap_or("account");
    if !matches!(account_type, "account" | "bucket" | "asset") {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "account_type must be 'account', 'bucket', or 'asset'".into(),
        });
    }
    if input.price_per_unit.is_some() && account_type != "asset" {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "price_per_unit can only be used with asset accounts".into(),
        });
    }
    validate_depreciation_fields_only_on_assets(
        account_type,
        input.purchase_price_minor,
        input.purchase_date.as_deref(),
        input.depreciation_period_months,
    )?;
    // Normalize: treat Some(0) as no depreciation (None)
    let depreciation_period_months = validate_and_normalize_depreciation(
        input.depreciation_period_months,
        input.purchase_price_minor,
        input.purchase_date.as_deref(),
        is_consolidation_currency(conn, input.currency_id)?,
    )?;
    // IBAN is only valid for regular accounts
    if input.iban.as_ref().is_some_and(|v| !v.is_empty()) && account_type != "account" {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "IBAN can only be set on account-type accounts".into(),
        });
    }
    if input.person_id.is_none() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "person_id is required for non-partner accounts".into(),
        });
    }
    with_savepoint_app(conn, || {
        let id = repository::create_account(
            conn,
            repository::CreateAccountParams {
                name: input.name.trim().to_owned(),
                currency_id: input.currency_id,
                account_type: account_type.to_owned(),
                initial_balance_minor: input.initial_balance_minor,
                price_per_unit: input.price_per_unit.clone(),
                iban: input.iban.clone(),
                person_id: input.person_id,
                purchase_price_minor: input.purchase_price_minor,
                purchase_date: input.purchase_date.clone(),
                depreciation_period_months,
            },
        )?;

        // Link to assets if provided and this is a regular account.
        if account_type == "account" {
            if let Some(asset_ids) = &input.linked_asset_ids {
                if !asset_ids.is_empty() {
                    crate::features::assets::repository::set_account_asset_links(
                        conn, id, asset_ids,
                    )?;
                }
            }
        }

        // Generate depreciation events immediately for new asset accounts.
        if account_type == "asset" {
            crate::features::transactions::repository::recalculate_depreciation_for_asset(
                conn, id,
            )?;
        }

        Ok(id)
    })
}

#[tauri::command]
pub fn update_account(
    state: State<'_, AppState>,
    input: UpdateAccountInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    update_account_with_conn(&conn, input)
}

fn update_account_with_conn(conn: &Connection, input: UpdateAccountInput) -> Result<(), AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Account name is required".into(),
        });
    }
    let account_type =
        repository::get_account_type(conn, input.account_id)?.ok_or_else(|| AppError {
            code: "NOT_FOUND".into(),
            message: "Account not found".into(),
        })?;
    if account_type != "partner" && input.person_id.is_none() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "person_id is required for non-partner accounts".into(),
        });
    }
    validate_depreciation_fields_only_on_assets(
        &account_type,
        input.purchase_price_minor,
        input.purchase_date.as_deref(),
        input.depreciation_period_months,
    )?;
    let account_currency_id = repository::get_account_currency_id(conn, input.account_id)?;
    let depreciation_period_months = validate_and_normalize_depreciation(
        input.depreciation_period_months,
        input.purchase_price_minor,
        input.purchase_date.as_deref(),
        is_consolidation_currency(conn, account_currency_id)?,
    )?;
    with_savepoint_app(conn, || {
        repository::update_account(
            conn,
            repository::UpdateAccountParams {
                account_id: input.account_id,
                name: input.name.trim().to_owned(),
                iban: input.iban.clone(),
                person_id: input.person_id,
                purchase_price_minor: input.purchase_price_minor,
                purchase_date: input.purchase_date.clone(),
                depreciation_period_months,
            },
        )?;

        // If this is an asset account, recalculate depreciation events so they stay
        // in sync with any metadata change (price, date, period — or clearing them).
        if account_type == "asset" {
            crate::features::transactions::repository::recalculate_depreciation_for_asset(
                conn,
                input.account_id,
            )?;
        }

        Ok(())
    })
}

#[tauri::command]
pub fn delete_account(state: State<'_, AppState>, account_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::delete_account(&conn, account_id)?;
    Ok(())
}

#[tauri::command]
pub fn update_sort_order(
    state: State<'_, AppState>,
    input: UpdateSortOrderInput,
) -> Result<(), AppError> {
    if input.entries.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "entries must not be empty".into(),
        });
    }
    let conn = state.conn()?;
    let pairs: Vec<(i64, i64)> = input
        .entries
        .iter()
        .map(|e| (e.account_id, e.sort_order))
        .collect();
    repository::update_sort_order(&conn, repository::UpdateSortOrderParams { updates: pairs })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn is_consolidation_currency(conn: &Connection, currency_id: i64) -> Result<bool, AppError> {
    Ok(get_consolidation_currency(conn)?.id == currency_id)
}

/// Depreciation fields only make sense on asset accounts.
fn validate_depreciation_fields_only_on_assets(
    account_type: &str,
    purchase_price_minor: Option<i64>,
    purchase_date: Option<&str>,
    depreciation_period_months: Option<i64>,
) -> Result<(), AppError> {
    let has_depreciation = purchase_price_minor.is_some()
        || purchase_date.is_some()
        || depreciation_period_months.is_some();
    if has_depreciation && account_type != "asset" {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Depreciation fields can only be set on asset accounts".into(),
        });
    }
    Ok(())
}

/// Normalises `depreciation_period_months` (`Some(0)` → `None`) and validates
/// that when a positive period is set the purchase price and date are also provided.
/// Depreciation fields are only accepted for an asset denominated in the consolidation
/// currency, and the purchase date must be a zero-padded `YYYY-MM-DD`.
fn validate_and_normalize_depreciation(
    depreciation_period_months: Option<i64>,
    purchase_price_minor: Option<i64>,
    purchase_date: Option<&str>,
    in_consolidation_currency: bool,
) -> Result<Option<i64>, AppError> {
    // Some(0) means "no depreciation" — treat as None so the DB stores NULL.
    let months = depreciation_period_months.and_then(|m| if m == 0 { None } else { Some(m) });
    if let Some(n) = months {
        if n < 0 {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Depreciation period cannot be negative".into(),
            });
        }
    }
    if purchase_date.is_some_and(|date| !is_iso_date(date)) {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Purchase date must be a valid date in YYYY-MM-DD format".into(),
        });
    }
    let has_depreciation_data =
        months.is_some() || purchase_price_minor.is_some() || purchase_date.is_some();
    if has_depreciation_data && !in_consolidation_currency {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Depreciation is only available for assets in the consolidation currency"
                .into(),
        });
    }
    if months.is_some() {
        if purchase_price_minor.is_none() {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Purchase price is required when depreciation period is set".into(),
            });
        }
        if purchase_date.is_none() {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Purchase date is required when depreciation period is set".into(),
            });
        }
    }
    Ok(months)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::partner_accounts::repository::{
        create_partner_account, CreatePartnerAccountParams,
    };

    const EUR_ID: i64 = 1;

    #[test]
    fn depreciation_zero_normalizes_to_none() {
        let result = validate_and_normalize_depreciation(Some(0), None, None, true).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn depreciation_positive_requires_price() {
        let err = validate_and_normalize_depreciation(Some(12), None, Some("2026-01-01"), true)
            .unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("price"));
    }

    #[test]
    fn depreciation_positive_requires_date() {
        let err =
            validate_and_normalize_depreciation(Some(12), Some(100_000), None, true).unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("date"));
    }

    #[test]
    fn depreciation_positive_with_all_fields_is_valid() {
        let result =
            validate_and_normalize_depreciation(Some(12), Some(100_000), Some("2026-01-01"), true)
                .unwrap();
        assert_eq!(result, Some(12));
    }

    #[test]
    fn depreciation_none_is_valid_without_price_or_date() {
        let result = validate_and_normalize_depreciation(None, None, None, true).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn update_account_rejects_negative_depreciation_period() {
        let err = validate_and_normalize_depreciation(Some(-3), None, None, true).unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("negative"));
    }

    #[test]
    fn depreciation_outside_consolidation_currency_is_rejected() {
        let err =
            validate_and_normalize_depreciation(Some(12), Some(100_000), Some("2026-01-01"), false)
                .unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("consolidation currency"));
    }

    #[test]
    fn any_depreciation_field_outside_consolidation_currency_is_rejected() {
        for (price, date) in [(Some(100_000), None), (None, Some("2026-01-01"))] {
            let err = validate_and_normalize_depreciation(None, price, date, false).unwrap_err();
            assert_eq!(err.code, "VALIDATION");
        }
    }

    #[test]
    fn no_depreciation_fields_are_valid_outside_consolidation_currency() {
        let result = validate_and_normalize_depreciation(Some(0), None, None, false).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn purchase_date_must_be_zero_padded_iso_date() {
        for date in [
            "2026-1-1",
            "2026-01-1",
            "26-01-01",
            "2026-01-01T00:00:00",
            "2026/01/01",
            "2026-02-30",
            "",
        ] {
            let err =
                validate_and_normalize_depreciation(Some(12), Some(100_000), Some(date), true)
                    .unwrap_err();
            assert_eq!(err.code, "VALIDATION", "date {date:?}");
            assert!(err.message.contains("YYYY-MM-DD"), "date {date:?}");
        }
    }

    fn default_person_id(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM person WHERE is_default = 1", [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    fn usd_id(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM currency WHERE code = 'USD'", [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    fn create_input(conn: &Connection, account_type: &str, currency_id: i64) -> CreateAccountInput {
        CreateAccountInput {
            name: "Test".to_owned(),
            currency_id,
            account_type: Some(account_type.to_owned()),
            initial_balance_minor: None,
            price_per_unit: None,
            linked_asset_ids: None,
            iban: None,
            person_id: Some(default_person_id(conn)),
            purchase_price_minor: None,
            purchase_date: None,
            depreciation_period_months: None,
        }
    }

    fn with_depreciation(mut input: CreateAccountInput) -> CreateAccountInput {
        input.purchase_price_minor = Some(120_000);
        input.purchase_date = Some("2026-01-15".to_owned());
        input.depreciation_period_months = Some(12);
        input
    }

    fn update_input(conn: &Connection, account_id: i64) -> UpdateAccountInput {
        UpdateAccountInput {
            account_id,
            name: "Renamed".to_owned(),
            iban: None,
            person_id: Some(default_person_id(conn)),
            purchase_price_minor: None,
            purchase_date: None,
            depreciation_period_months: None,
        }
    }

    fn with_update_depreciation(mut input: UpdateAccountInput) -> UpdateAccountInput {
        input.purchase_price_minor = Some(120_000);
        input.purchase_date = Some("2026-01-15".to_owned());
        input.depreciation_period_months = Some(12);
        input
    }

    fn stored_person_id(conn: &Connection, account_id: i64) -> Option<i64> {
        conn.query_row(
            "SELECT person_id FROM account WHERE id = ?1",
            [account_id],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn create_account_requires_person_id() {
        let conn = initialize_in_memory().expect("DB init failed");
        let mut input = create_input(&conn, "account", EUR_ID);
        input.person_id = None;

        let err = create_account_with_conn(&conn, input).unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("person_id"));
    }

    #[test]
    fn create_account_rejects_depreciation_fields_on_non_asset() {
        let conn = initialize_in_memory().expect("DB init failed");
        let input = with_depreciation(create_input(&conn, "account", EUR_ID));

        let err = create_account_with_conn(&conn, input).unwrap_err();

        assert_eq!(err.code, "VALIDATION");
    }

    #[test]
    fn create_asset_rejects_depreciation_outside_consolidation_currency() {
        let conn = initialize_in_memory().expect("DB init failed");
        let input = with_depreciation(create_input(&conn, "asset", usd_id(&conn)));

        let err = create_account_with_conn(&conn, input).unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("consolidation currency"));
    }

    #[test]
    fn create_asset_without_depreciation_is_allowed_in_any_currency() {
        let conn = initialize_in_memory().expect("DB init failed");

        let id = create_account_with_conn(&conn, create_input(&conn, "asset", usd_id(&conn)));

        assert!(id.is_ok());
    }

    #[test]
    fn create_asset_accepts_depreciation_in_consolidation_currency() {
        let conn = initialize_in_memory().expect("DB init failed");
        let input = with_depreciation(create_input(&conn, "asset", EUR_ID));

        let id = create_account_with_conn(&conn, input).unwrap();

        let months: Option<i64> = conn
            .query_row(
                "SELECT depreciation_period_months FROM account WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(months, Some(12));
    }

    #[test]
    fn create_asset_rejects_short_purchase_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let mut input = with_depreciation(create_input(&conn, "asset", EUR_ID));
        input.purchase_date = Some("2026-1-5".to_owned());

        let err = create_account_with_conn(&conn, input).unwrap_err();

        assert_eq!(err.code, "VALIDATION");
    }

    #[test]
    fn update_account_requires_person_id_for_non_partner_accounts() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_account_with_conn(&conn, create_input(&conn, "account", EUR_ID)).unwrap();
        let mut input = update_input(&conn, id);
        input.person_id = None;

        let err = update_account_with_conn(&conn, input).unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("person_id"));
        assert_eq!(stored_person_id(&conn, id), Some(default_person_id(&conn)));
    }

    #[test]
    fn update_account_does_not_require_person_id_for_partner_accounts() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_partner_account(
            &conn,
            CreatePartnerAccountParams {
                name: "Partner".to_owned(),
                iban: "SK3112000000198742637541".to_owned(),
                currency_id: EUR_ID,
            },
        )
        .unwrap();
        let mut input = update_input(&conn, id);
        input.person_id = None;

        update_account_with_conn(&conn, input).unwrap();

        assert_eq!(stored_person_id(&conn, id), None);
    }

    #[test]
    fn update_account_rejects_depreciation_fields_on_non_asset() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_account_with_conn(&conn, create_input(&conn, "account", EUR_ID)).unwrap();

        let err =
            update_account_with_conn(&conn, with_update_depreciation(update_input(&conn, id)))
                .unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("asset accounts"));
    }

    #[test]
    fn update_asset_rejects_depreciation_outside_consolidation_currency() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id =
            create_account_with_conn(&conn, create_input(&conn, "asset", usd_id(&conn))).unwrap();

        let err =
            update_account_with_conn(&conn, with_update_depreciation(update_input(&conn, id)))
                .unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("consolidation currency"));
    }

    #[test]
    fn update_asset_accepts_depreciation_in_consolidation_currency() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_account_with_conn(&conn, create_input(&conn, "asset", EUR_ID)).unwrap();

        update_account_with_conn(&conn, with_update_depreciation(update_input(&conn, id))).unwrap();

        let months: Option<i64> = conn
            .query_row(
                "SELECT depreciation_period_months FROM account WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(months, Some(12));
    }

    #[test]
    fn update_asset_rejects_short_purchase_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_account_with_conn(&conn, create_input(&conn, "asset", EUR_ID)).unwrap();
        let mut input = with_update_depreciation(update_input(&conn, id));
        input.purchase_date = Some("2026-1-5".to_owned());

        let err = update_account_with_conn(&conn, input).unwrap_err();

        assert_eq!(err.code, "VALIDATION");
    }

    fn account_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM account", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn create_account_with_an_unknown_linked_asset_leaves_no_account() {
        let conn = initialize_in_memory().expect("DB init failed");
        let before = account_count(&conn);
        let mut input = create_input(&conn, "account", EUR_ID);
        input.linked_asset_ids = Some(vec![999]);

        let err = create_account_with_conn(&conn, input).unwrap_err();

        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(account_count(&conn), before);
    }

    #[test]
    fn update_asset_keeps_its_old_metadata_when_depreciation_fails() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_account_with_conn(&conn, create_input(&conn, "asset", EUR_ID)).unwrap();
        conn.execute_batch(
            "CREATE TEMP TRIGGER fail_depreciation BEFORE INSERT ON event
             WHEN NEW.is_system_generated = 1
             BEGIN SELECT RAISE(ABORT, 'depreciation failed'); END;",
        )
        .unwrap();

        update_account_with_conn(&conn, with_update_depreciation(update_input(&conn, id)))
            .unwrap_err();

        let (name, months): (String, Option<i64>) = conn
            .query_row(
                "SELECT name, depreciation_period_months FROM account WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(name, "Test");
        assert_eq!(months, None);
    }

    #[test]
    fn update_account_reports_unknown_account() {
        let conn = initialize_in_memory().expect("DB init failed");

        let err = update_account_with_conn(&conn, update_input(&conn, 9999)).unwrap_err();

        assert_eq!(err.code, "NOT_FOUND");
    }
}
