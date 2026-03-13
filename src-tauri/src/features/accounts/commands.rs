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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountInput {
    pub account_id: i64,
    pub name: String,
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
    let conn = state.conn()?;
    let id = repository::create_account(
        &conn,
        input.name.trim(),
        input.currency_id,
        account_type,
        input.initial_balance_minor,
        input.price_per_unit.as_deref(),
    )?;

    // Link to assets if provided and this is a regular account.
    if account_type == "account" {
        if let Some(asset_ids) = &input.linked_asset_ids {
            if !asset_ids.is_empty() {
                crate::features::assets::repository::set_account_asset_links(&conn, id, asset_ids)?;
            }
        }
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
    let conn = state.conn()?;
    repository::update_account(&conn, input.account_id, input.name.trim())?;
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
    repository::update_sort_order(&conn, &pairs)?;
    Ok(())
}
