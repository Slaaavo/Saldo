use crate::error::AppError;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::business_use::{check_bps, AssetUseTerms};
use super::models::AccountAssetLink;
use super::purchase::{split_purchase, PurchaseSplit, PurchaseVat};
use super::repository;
use crate::features::currency::Currency;
use crate::shared::div_round;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewAssetPurchaseInput {
    pub purchase_vat: PurchaseVat,
    pub business_use_pct_bps: i64,
    pub depreciation_period_months: Option<i64>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssetPurchasePreview {
    #[serde(flatten)]
    pub split: PurchaseSplit,
    pub monthly_minor: Option<i64>,
    pub monthly_deductible_minor: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCustomUnitInput {
    pub name: String,
    pub minor_units: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCustomUnitInput {
    pub currency_id: i64,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAssetValueInput {
    pub account_id: i64,
    pub amount_minor: Option<i64>,
    pub price_per_unit: Option<String>,
    pub event_date: String,
    pub note: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAccountAssetLinksInput {
    pub account_id: i64,
    pub asset_ids: Vec<i64>,
}

#[tauri::command]
pub fn get_asset_use(
    state: State<'_, AppState>,
    account_id: i64,
) -> Result<AssetUseTerms, AppError> {
    let conn = state.conn()?;
    repository::get_use_terms(&conn, account_id)
}

#[tauri::command]
pub fn preview_asset_purchase(
    input: PreviewAssetPurchaseInput,
) -> Result<AssetPurchasePreview, AppError> {
    input.purchase_vat.validate()?;
    check_bps("Business use", input.business_use_pct_bps)?;
    let split = split_purchase(&input.purchase_vat, input.business_use_pct_bps);
    let monthly_minor = input
        .depreciation_period_months
        .filter(|&months| months > 0)
        .map(|months| div_round(split.depreciating_minor, months));

    Ok(AssetPurchasePreview {
        split,
        monthly_minor,
        monthly_deductible_minor: monthly_minor
            .map(|monthly| monthly * input.business_use_pct_bps / 10000),
    })
}

#[tauri::command]
pub fn create_custom_unit(
    state: State<'_, AppState>,
    input: CreateCustomUnitInput,
) -> Result<i64, AppError> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Unit name is required".into(),
        });
    }
    if input.minor_units < 0 || input.minor_units > 8 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "minor_units must be between 0 and 8".into(),
        });
    }
    let conn = state.conn()?;
    let id = repository::create_custom_unit(
        &conn,
        repository::CreateCustomUnitParams {
            name,
            minor_units: input.minor_units,
        },
    )?;
    Ok(id)
}

#[tauri::command]
pub fn list_custom_units(state: State<'_, AppState>) -> Result<Vec<Currency>, AppError> {
    let conn = state.conn()?;
    let units = repository::list_custom_units(&conn)?;
    Ok(units)
}

#[tauri::command]
pub fn update_custom_unit(
    state: State<'_, AppState>,
    input: UpdateCustomUnitInput,
) -> Result<(), AppError> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Unit name is required".into(),
        });
    }
    let conn = state.conn()?;
    repository::update_custom_unit(
        &conn,
        repository::UpdateCustomUnitParams {
            currency_id: input.currency_id,
            name,
        },
    )?;
    Ok(())
}

#[tauri::command]
pub fn update_asset_value(
    state: State<'_, AppState>,
    input: UpdateAssetValueInput,
) -> Result<(), AppError> {
    if input.amount_minor.is_none() && input.price_per_unit.is_none() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "At least one of amount_minor or price_per_unit is required".into(),
        });
    }
    crate::shared::validate_event_date(&input.event_date)?;
    let conn = state.conn()?;
    repository::update_asset_value(
        &conn,
        repository::UpdateAssetValueParams {
            account_id: input.account_id,
            amount_minor: input.amount_minor,
            price_per_unit: input.price_per_unit,
            event_date: input.event_date,
            note: input.note,
        },
    )?;
    Ok(())
}

#[tauri::command]
pub fn list_account_asset_links(
    state: State<'_, AppState>,
    account_id: Option<i64>,
) -> Result<Vec<AccountAssetLink>, AppError> {
    let conn = state.conn()?;
    let links = repository::list_account_asset_links(&conn, account_id)?;
    Ok(links)
}

#[tauri::command]
pub fn set_account_asset_links(
    state: State<'_, AppState>,
    input: SetAccountAssetLinksInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::set_account_asset_links(&conn, input.account_id, &input.asset_ids)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kona_input(months: Option<i64>) -> PreviewAssetPurchaseInput {
        PreviewAssetPurchaseInput {
            purchase_vat: PurchaseVat {
                gross_minor: 1_560_000,
                vat_rate_bps: 2300,
                vat_reclaimable_pct_bps: 5000,
            },
            business_use_pct_bps: 8000,
            depreciation_period_months: months,
        }
    }

    #[test]
    fn the_preview_shows_the_kona_summary() {
        assert_eq!(
            preview_asset_purchase(kona_input(Some(24))).unwrap(),
            AssetPurchasePreview {
                split: PurchaseSplit {
                    depreciating_minor: 1_268_292,
                    reclaimed_vat_minor: 145_854,
                    lost_vat_entry_minor: 145_854,
                    start_value_minor: 1_560_000,
                },
                monthly_minor: Some(52_846),
                monthly_deductible_minor: Some(42_276),
            }
        );
    }

    #[test]
    fn a_preview_with_no_period_has_no_monthly_amount() {
        let preview = preview_asset_purchase(kona_input(None)).unwrap();
        assert_eq!(preview.monthly_minor, None);
        assert_eq!(preview.split.depreciating_minor, 1_268_292);
    }

    #[test]
    fn a_preview_refuses_a_rate_outside_0_to_100_percent() {
        let mut input = kona_input(Some(24));
        input.purchase_vat.vat_rate_bps = -10000;
        assert!(preview_asset_purchase(input).is_err());
    }
}
