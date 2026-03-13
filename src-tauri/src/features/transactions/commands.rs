use crate::error::AppError;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

use super::models::{EventWithData, SnapshotRow};
use super::repository;

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
    pub before_date: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkBalanceEntry {
    pub account_id: i64,
    pub amount_minor: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateBalanceUpdatesInput {
    pub entries: Vec<BulkBalanceEntry>,
    pub event_date: String,
    pub note: Option<String>,
}

#[tauri::command]
pub fn create_balance_update(
    state: State<'_, AppState>,
    input: CreateBalanceUpdateInput,
) -> Result<i64, AppError> {
    crate::shared::validate_event_date(&input.event_date)?;
    let conn = state.conn()?;
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
    let conn = state.conn()?;
    let snapshot = repository::get_accounts_snapshot(&conn, &selected_datetime)?;
    Ok(snapshot)
}

#[tauri::command]
pub fn list_events(
    state: State<'_, AppState>,
    filter: ListEventsFilter,
) -> Result<Vec<EventWithData>, AppError> {
    let conn = state.conn()?;
    let events = repository::list_events(&conn, filter.account_id, filter.before_date.as_deref())?;
    Ok(events)
}

#[tauri::command]
pub fn update_event(state: State<'_, AppState>, input: UpdateEventInput) -> Result<(), AppError> {
    crate::shared::validate_event_date(&input.event_date)?;
    let conn = state.conn()?;
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
    let conn = state.conn()?;
    repository::delete_event(&conn, event_id)?;
    Ok(())
}

#[tauri::command]
pub fn bulk_create_balance_updates(
    state: State<'_, AppState>,
    input: BulkCreateBalanceUpdatesInput,
) -> Result<Vec<i64>, AppError> {
    crate::shared::validate_event_date(&input.event_date)?;
    if input.entries.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "At least one balance entry is required".into(),
        });
    }
    let conn = state.conn()?;
    let entries: Vec<(i64, i64)> = input
        .entries
        .iter()
        .map(|e| (e.account_id, e.amount_minor))
        .collect();
    let ids = repository::bulk_create_balance_updates(
        &conn,
        &entries,
        &input.event_date,
        input.note.as_deref(),
    )?;
    Ok(ids)
}
