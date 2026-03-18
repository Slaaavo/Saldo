use crate::error::AppError;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

use super::models::{
    CreateSplitGroupInput, ListEventsResult, SnapshotRow, UpdateSplitGroupDateInput,
};
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
    pub account_ids: Option<Vec<i64>>,
    pub before_date: Option<String>,
    pub from_date: Option<String>,
    pub event_types: Option<Vec<String>>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCashflowInput {
    pub account_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub counterpart_account_id: Option<i64>,
    pub bucket_id: Option<i64>,
    pub original_currency_id: Option<i64>,
    pub original_amount_minor: Option<i64>,
    pub fx_rate_mantissa: Option<i64>,
    pub fx_rate_exponent: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateCashflowsInput {
    pub entries: Vec<CreateCashflowInput>,
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
    if let Some(account_type) =
        crate::features::accounts::repository::get_account_type(&conn, input.account_id)?
    {
        if account_type == "partner" {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Cannot create events on partner accounts".into(),
            });
        }
    }
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
) -> Result<ListEventsResult, AppError> {
    let conn = state.conn()?;
    let result = repository::list_events(
        &conn,
        filter.account_id,
        filter.account_ids.as_deref(),
        filter.before_date.as_deref(),
        filter.from_date.as_deref(),
        filter.event_types.as_deref(),
        filter.limit,
    )?;
    Ok(result)
}

#[tauri::command]
pub fn update_event(state: State<'_, AppState>, input: UpdateEventInput) -> Result<(), AppError> {
    crate::shared::validate_event_date(&input.event_date)?;
    let conn = state.conn()?;
    repository::check_event_split_group_date_conflict(&conn, input.event_id, &input.event_date)?;
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
    for entry in &input.entries {
        if let Some(account_type) =
            crate::features::accounts::repository::get_account_type(&conn, entry.account_id)?
        {
            if account_type == "partner" {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: "Cannot create events on partner accounts".into(),
                });
            }
        }
    }
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

#[tauri::command]
pub fn create_cashflow(
    state: State<'_, AppState>,
    input: CreateCashflowInput,
) -> Result<i64, AppError> {
    crate::shared::validate_event_date(&input.event_date)?;
    let conn = state.conn()?;
    if let Some(account_type) =
        crate::features::accounts::repository::get_account_type(&conn, input.account_id)?
    {
        if account_type == "partner" {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Cannot create events on partner accounts".into(),
            });
        }
    }
    if input.counterpart_account_id == Some(input.account_id) {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Cannot transfer to the same account".into(),
        });
    }
    let counterpart_type = match input.counterpart_account_id {
        Some(cp_id) => crate::features::accounts::repository::get_account_type(&conn, cp_id)?,
        None => None,
    };
    let entry = repository::CashflowEntry {
        account_id: input.account_id,
        amount_minor: input.amount_minor,
        event_date: input.event_date.clone(),
        note: input.note.clone(),
        counterpart_account_id: input.counterpart_account_id,
        bucket_id: input.bucket_id,
        original_currency_id: input.original_currency_id,
        original_amount_minor: input.original_amount_minor,
        fx_rate_mantissa: input.fx_rate_mantissa,
        fx_rate_exponent: input.fx_rate_exponent,
    };
    let event_id = if counterpart_type.as_deref() == Some("account") {
        let (source_id, _) = repository::create_transfer(&conn, &entry)?;
        source_id
    } else {
        repository::create_cashflow(&conn, &entry)?
    };
    Ok(event_id)
}

#[tauri::command]
pub fn bulk_create_cashflows(
    state: State<'_, AppState>,
    input: BulkCreateCashflowsInput,
) -> Result<Vec<i64>, AppError> {
    if input.entries.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "At least one cashflow entry is required".into(),
        });
    }
    for entry in &input.entries {
        crate::shared::validate_event_date(&entry.event_date)?;
    }
    let conn = state.conn()?;
    for entry in &input.entries {
        if let Some(account_type) =
            crate::features::accounts::repository::get_account_type(&conn, entry.account_id)?
        {
            if account_type == "partner" {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: "Cannot create events on partner accounts".into(),
                });
            }
        }
        if entry.counterpart_account_id == Some(entry.account_id) {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Cannot transfer to the same account".into(),
            });
        }
    }
    let entries: Vec<repository::CashflowEntry> = input
        .entries
        .into_iter()
        .map(|e| repository::CashflowEntry {
            account_id: e.account_id,
            amount_minor: e.amount_minor,
            event_date: e.event_date,
            note: e.note,
            counterpart_account_id: e.counterpart_account_id,
            bucket_id: e.bucket_id,
            original_currency_id: e.original_currency_id,
            original_amount_minor: e.original_amount_minor,
            fx_rate_mantissa: e.fx_rate_mantissa,
            fx_rate_exponent: e.fx_rate_exponent,
        })
        .collect();
    let ids = repository::bulk_create_cashflows(&conn, &entries)?;
    Ok(ids)
}

#[tauri::command]
pub fn create_split_group(
    state: State<'_, AppState>,
    input: CreateSplitGroupInput,
) -> Result<i64, AppError> {
    if input.legs.len() < 2 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Split group requires at least 2 legs".into(),
        });
    }
    for leg in &input.legs {
        crate::shared::validate_event_date(&leg.event_date)?;
    }
    let dates: std::collections::HashSet<&str> =
        input.legs.iter().map(|l| l.event_date.as_str()).collect();
    if dates.len() > 1 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "All split group legs must share the same date".into(),
        });
    }
    let conn = state.conn()?;
    if let Some(account_type) =
        crate::features::accounts::repository::get_account_type(&conn, input.account_id)?
    {
        if account_type == "partner" {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Cannot create events on partner accounts".into(),
            });
        }
    }
    for leg in &input.legs {
        if leg.counterpart_account_id == Some(input.account_id) {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Cannot transfer to the same account".into(),
            });
        }
    }
    let split_group_id = repository::create_split_group_with_legs(
        &conn,
        input.account_id,
        input.group_note.as_deref(),
        &input.legs,
    )?;
    Ok(split_group_id)
}

#[tauri::command]
pub fn update_split_group_date(
    state: State<'_, AppState>,
    input: UpdateSplitGroupDateInput,
) -> Result<(), AppError> {
    crate::shared::validate_event_date(&input.new_date)?;
    let conn = state.conn()?;
    repository::update_split_group_date(&conn, input.split_group_id, &input.new_date)?;
    Ok(())
}
