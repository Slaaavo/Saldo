use crate::error::AppError;
use crate::features::persons::models::PersonRow;
use crate::shared::with_savepoint_app;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

use super::repository;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePersonInput {
    pub name: String,
    pub person_type: String,
    pub vat_payer: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePersonInput {
    pub person_id: i64,
    pub name: String,
    pub person_type: String,
    pub vat_payer: bool,
}

fn validate_person_input(name: &str, person_type: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Person name is required.".into(),
        });
    }
    if !matches!(person_type, "physical" | "legal") {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "person_type must be 'physical' or 'legal'.".into(),
        });
    }
    Ok(())
}

#[tauri::command]
pub fn create_person(
    state: State<'_, AppState>,
    input: CreatePersonInput,
) -> Result<i64, AppError> {
    validate_person_input(&input.name, &input.person_type)?;
    let conn = state.conn()?;
    repository::create_person(
        &conn,
        repository::CreatePersonParams {
            name: input.name.trim().to_owned(),
            person_type: input.person_type,
            vat_payer: input.vat_payer,
        },
    )
}

#[tauri::command]
pub fn list_persons(state: State<'_, AppState>) -> Result<Vec<PersonRow>, AppError> {
    let conn = state.conn()?;
    repository::list_persons(&conn).map_err(AppError::from)
}

#[tauri::command]
pub fn update_person(state: State<'_, AppState>, input: UpdatePersonInput) -> Result<(), AppError> {
    validate_person_input(&input.name, &input.person_type)?;
    let conn = state.conn()?;
    repository::update_person(
        &conn,
        repository::UpdatePersonParams {
            person_id: input.person_id,
            name: input.name.trim().to_owned(),
            person_type: input.person_type,
            vat_payer: input.vat_payer,
        },
    )
}

#[tauri::command]
pub fn delete_person(state: State<'_, AppState>, person_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    with_savepoint_app(&conn, || {
        let account_ids =
            repository::get_person_accounts(&conn, person_id).map_err(AppError::from)?;
        for account_id in account_ids {
            crate::features::accounts::repository::delete_account(&conn, account_id)?;
        }
        repository::delete_person_row(&conn, person_id)
    })
}
