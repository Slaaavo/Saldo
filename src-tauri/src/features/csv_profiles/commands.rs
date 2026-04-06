use crate::error::AppError;
use crate::features::csv_profiles::models::{ImportProfileRow, RuleInput};
use crate::AppState;
use tauri::State;

use super::repository;

#[tauri::command]
pub fn list_import_profiles(state: State<'_, AppState>) -> Result<Vec<ImportProfileRow>, AppError> {
    let conn = state.conn()?;
    repository::list_import_profiles(&conn)
}

#[tauri::command]
pub fn create_import_profile(
    state: State<'_, AppState>,
    name: String,
    column_mapping_json: String,
    rules: Vec<RuleInput>,
) -> Result<i64, AppError> {
    if name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Profile name is required.".into(),
        });
    }
    let conn = state.conn()?;
    repository::create_import_profile(
        &conn,
        repository::CreateImportProfileParams {
            name: name.trim().to_owned(),
            column_mapping_json,
            rules,
        },
    )
}

#[tauri::command]
pub fn update_import_profile(
    state: State<'_, AppState>,
    profile_id: i64,
    name: String,
    column_mapping_json: String,
    rules: Vec<RuleInput>,
) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Profile name is required.".into(),
        });
    }
    let conn = state.conn()?;
    repository::update_import_profile(
        &conn,
        repository::UpdateImportProfileParams {
            profile_id,
            name: name.trim().to_owned(),
            column_mapping_json,
            rules,
        },
    )
}

#[tauri::command]
pub fn delete_import_profile(state: State<'_, AppState>, profile_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::delete_import_profile(&conn, profile_id)
}

#[tauri::command]
pub fn get_preferred_profile(
    state: State<'_, AppState>,
    account_id: i64,
) -> Result<Option<ImportProfileRow>, AppError> {
    let conn = state.conn()?;
    repository::get_preferred_profile(&conn, account_id)
}

#[tauri::command]
pub fn set_preferred_profile(
    state: State<'_, AppState>,
    account_id: i64,
    profile_id: Option<i64>,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::set_preferred_profile(
        &conn,
        repository::SetPreferredProfileParams {
            account_id,
            profile_id,
        },
    )
}
