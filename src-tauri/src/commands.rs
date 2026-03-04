use crate::{error::AppError, models::*, repository, AppState};
use serde::Deserialize;
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBalanceUpdateInput {
    pub account_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccountInput {
    pub name: String,
    pub currency_id: i64,
    pub initial_balance_minor: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountInput {
    pub account_id: i64,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEventInput {
    pub event_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEventsFilter {
    pub account_id: Option<i64>,
}

#[tauri::command]
pub fn create_balance_update(
    state: State<'_, AppState>,
    input: CreateBalanceUpdateInput,
) -> Result<i64, AppError> {
    if input.event_date.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "event_date is required".into(),
        });
    }
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    let event_id = repository::create_balance_update(
        &conn,
        input.account_id,
        input.amount_minor,
        &input.event_date,
        input.note.as_deref(),
    )?;
    Ok(event_id)
}

#[tauri::command]
pub fn get_accounts_snapshot(
    state: State<'_, AppState>,
    date_iso: String,
) -> Result<Vec<SnapshotRow>, AppError> {
    let selected_datetime = if date_iso.len() == 10 {
        format!("{}T23:59:59", date_iso)
    } else {
        date_iso
    };
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    let snapshot = repository::get_accounts_snapshot(&conn, &selected_datetime)?;
    Ok(snapshot)
}

#[tauri::command]
pub fn list_events(
    state: State<'_, AppState>,
    filter: ListEventsFilter,
) -> Result<Vec<EventWithData>, AppError> {
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    let events = repository::list_events(&conn, filter.account_id)?;
    Ok(events)
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
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    let id = repository::create_account(
        &conn,
        input.name.trim(),
        input.currency_id,
        input.initial_balance_minor,
    )?;
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
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    repository::update_account(&conn, input.account_id, input.name.trim())?;
    Ok(())
}

#[tauri::command]
pub fn delete_account(state: State<'_, AppState>, account_id: i64) -> Result<(), AppError> {
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    repository::delete_account(&conn, account_id)?;
    Ok(())
}

#[tauri::command]
pub fn update_event(state: State<'_, AppState>, input: UpdateEventInput) -> Result<(), AppError> {
    if input.event_date.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "event_date is required".into(),
        });
    }
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    repository::update_event(
        &conn,
        input.event_id,
        input.amount_minor,
        &input.event_date,
        input.note.as_deref(),
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_event(state: State<'_, AppState>, event_id: i64) -> Result<(), AppError> {
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    repository::delete_event(&conn, event_id)?;
    Ok(())
}
