use crate::error::AppError;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

use super::models::{
    CreateSplitGroupInput, EventWithData, ListEventsResult, SnapshotRow, UpdateSplitGroupDateInput,
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
pub struct UpdateTransferInput {
    pub from_event_id: i64,
    pub to_event_id: i64,
    pub from_date: String,
    pub to_date: String,
    pub amount_minor: i64,
    pub to_amount_minor: i64,
    pub note: Option<String>,
    pub original_currency_id: Option<i64>,
    pub fx_rate_mantissa: Option<i64>,
    pub fx_rate_exponent: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEventsFilter {
    pub account_id: Option<i64>,
    pub account_ids: Option<Vec<i64>>,
    pub bucket_ids: Option<Vec<i64>>,
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
        if account_type == "bucket" {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Use create_bucket_balance_update for bucket accounts".into(),
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
        repository::ListEventsQuery {
            account_id: filter.account_id,
            account_ids: filter.account_ids,
            before_date: filter.before_date,
            from_date: filter.from_date,
            event_types: filter.event_types,
            limit: filter.limit,
            bucket_ids: filter.bucket_ids,
        },
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

#[tauri::command]
pub async fn get_event_by_id(
    state: State<'_, AppState>,
    event_id: i64,
) -> Result<Option<EventWithData>, AppError> {
    let conn = state.conn()?;
    repository::get_event_by_id(&conn, event_id).map_err(AppError::from)
}

#[tauri::command]
pub fn update_transfer(
    state: State<'_, AppState>,
    input: UpdateTransferInput,
) -> Result<(), AppError> {
    crate::shared::validate_event_date(&input.from_date)?;
    crate::shared::validate_event_date(&input.to_date)?;
    if input.from_event_id == input.to_event_id {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "from_event_id and to_event_id must be different".into(),
        });
    }
    if input.amount_minor >= 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "amount_minor must be negative (from-leg is a debit)".into(),
        });
    }
    if input.to_amount_minor <= 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "to_amount_minor must be positive (to-leg is a credit)".into(),
        });
    }
    let fx_fields_all_some = input.original_currency_id.is_some()
        && input.fx_rate_mantissa.is_some()
        && input.fx_rate_exponent.is_some();
    let fx_fields_all_none = input.original_currency_id.is_none()
        && input.fx_rate_mantissa.is_none()
        && input.fx_rate_exponent.is_none();
    if !fx_fields_all_some && !fx_fields_all_none {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "original_currency_id, fx_rate_mantissa, and fx_rate_exponent must all be provided or all be null".into(),
        });
    }
    let original_amount_minor_for_from_leg = if input.original_currency_id.is_some() {
        Some(input.to_amount_minor)
    } else {
        None
    };
    let conn = state.conn()?;
    repository::update_transfer(
        &conn,
        repository::UpdateTransferParams {
            from_event_id: input.from_event_id,
            to_event_id: input.to_event_id,
            from_date: input.from_date,
            to_date: input.to_date,
            from_amount_minor: input.amount_minor,
            to_amount_minor: input.to_amount_minor,
            note: input.note,
            original_currency_id: input.original_currency_id,
            original_amount_minor_for_from_leg,
            fx_rate_mantissa: input.fx_rate_mantissa,
            fx_rate_exponent: input.fx_rate_exponent,
        },
    )?;
    Ok(())
}
