use crate::error::AppError;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

use super::models::AccountAssetLink;
use super::repository;
use crate::features::currency::Currency;

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
    let id = repository::create_custom_unit(&conn, &name, input.minor_units)?;
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
    repository::update_custom_unit(&conn, input.currency_id, &name)?;
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
        input.account_id,
        input.amount_minor,
        input.price_per_unit.as_deref(),
        &input.event_date,
        input.note.as_deref(),
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
