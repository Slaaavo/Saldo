use crate::error::AppError;
use crate::AppState;
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
    // Depreciation fields only valid for asset accounts
    let has_depreciation = input.purchase_price_minor.is_some()
        || input.purchase_date.is_some()
        || input.depreciation_period_months.is_some();
    if has_depreciation && account_type != "asset" {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Depreciation fields can only be set on asset accounts".into(),
        });
    }
    // Normalize: treat Some(0) as no depreciation (None)
    let depreciation_period_months = validate_and_normalize_depreciation(
        input.depreciation_period_months,
        input.purchase_price_minor,
        input.purchase_date.as_deref(),
    )?;
    // IBAN is only valid for regular accounts
    if input.iban.as_ref().is_some_and(|v| !v.is_empty()) && account_type != "account" {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "IBAN can only be set on account-type accounts".into(),
        });
    }
    if account_type != "partner" && input.person_id.is_none() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "person_id is required for non-partner accounts".into(),
        });
    }
    let conn = state.conn()?;
    let id = repository::create_account(
        &conn,
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
                crate::features::assets::repository::set_account_asset_links(&conn, id, asset_ids)?;
            }
        }
    }

    // Generate depreciation events immediately for new asset accounts.
    if account_type == "asset" {
        crate::features::transactions::repository::recalculate_depreciation_for_asset(&conn, id)?;
    }

    Ok(id)
}

#[tauri::command]
pub fn update_account(
    state: State<'_, AppState>,
    input: UpdateAccountInput,
) -> Result<(), AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Account name is required".into(),
        });
    }
    let depreciation_period_months = validate_and_normalize_depreciation(
        input.depreciation_period_months,
        input.purchase_price_minor,
        input.purchase_date.as_deref(),
    )?;
    let conn = state.conn()?;
    repository::update_account(
        &conn,
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
    let account_type =
        repository::get_account_type(&conn, input.account_id).map_err(AppError::from)?;
    if account_type.as_deref() == Some("asset") {
        crate::features::transactions::repository::recalculate_depreciation_for_asset(
            &conn,
            input.account_id,
        )?;
    }

    Ok(())
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

/// Normalises `depreciation_period_months` (`Some(0)` → `None`) and validates
/// that when a positive period is set the purchase price and date are also provided.
fn validate_and_normalize_depreciation(
    depreciation_period_months: Option<i64>,
    purchase_price_minor: Option<i64>,
    purchase_date: Option<&str>,
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

    #[test]
    fn depreciation_zero_normalizes_to_none() {
        let result = validate_and_normalize_depreciation(Some(0), None, None).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn depreciation_positive_requires_price() {
        let err =
            validate_and_normalize_depreciation(Some(12), None, Some("2026-01-01")).unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("price"));
    }

    #[test]
    fn depreciation_positive_requires_date() {
        let err = validate_and_normalize_depreciation(Some(12), Some(100_000), None).unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("date"));
    }

    #[test]
    fn depreciation_positive_with_all_fields_is_valid() {
        let result =
            validate_and_normalize_depreciation(Some(12), Some(100_000), Some("2026-01-01"))
                .unwrap();
        assert_eq!(result, Some(12));
    }

    #[test]
    fn depreciation_none_is_valid_without_price_or_date() {
        let result = validate_and_normalize_depreciation(None, None, None).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn update_account_rejects_negative_depreciation_period() {
        let err = validate_and_normalize_depreciation(Some(-3), None, None).unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("negative"));
    }
}
