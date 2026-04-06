use crate::error::AppError;
use crate::features::partner_accounts::models::PartnerAccountRow;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

use super::repository;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePartnerAccountInput {
    pub name: String,
    pub iban: String,
    pub currency_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePartnerAccountInput {
    pub account_id: i64,
    pub name: String,
    pub iban: String,
}

#[tauri::command]
pub fn create_partner_account(
    state: State<'_, AppState>,
    input: CreatePartnerAccountInput,
) -> Result<i64, AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Partner name is required".into(),
        });
    }
    if input.iban.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "IBAN is required".into(),
        });
    }
    let conn = state.conn()?;
    repository::create_partner_account(
        &conn,
        repository::CreatePartnerAccountParams {
            name: input.name.trim().to_owned(),
            iban: input.iban.trim().to_owned(),
            currency_id: input.currency_id,
        },
    )
}

#[tauri::command]
pub fn list_partner_accounts(
    state: State<'_, AppState>,
) -> Result<Vec<PartnerAccountRow>, AppError> {
    let conn = state.conn()?;
    repository::list_partner_accounts(&conn).map_err(AppError::from)
}

#[tauri::command]
pub fn update_partner_account(
    state: State<'_, AppState>,
    input: UpdatePartnerAccountInput,
) -> Result<(), AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Partner name is required".into(),
        });
    }
    if input.iban.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "IBAN is required".into(),
        });
    }
    let conn = state.conn()?;
    repository::update_partner_account(
        &conn,
        repository::UpdatePartnerAccountParams {
            account_id: input.account_id,
            name: input.name.trim().to_owned(),
            iban: input.iban.trim().to_owned(),
        },
    )
}

#[tauri::command]
pub fn delete_partner_account(state: State<'_, AppState>, account_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::delete_partner_account(&conn, account_id)
}
