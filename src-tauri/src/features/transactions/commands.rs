use chrono::{Datelike, NaiveDate};

use crate::error::AppError;
use crate::features::persons::repository::{get_person_vat_payer, resolve_default_taxable_account};
use crate::shared::with_savepoint_app;
use crate::AppState;
use rusqlite::Connection;
use serde::Deserialize;
use tauri::State;

use super::models::{
    CreateSplitGroupInput, EventWithData, ListEventsResult, SnapshotRow, UpdateSplitGroupDateInput,
};
use super::repository::{self, GetSnapshotParams};

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
    pub person_id: Option<i64>,
    pub unmatched_only: Option<bool>,
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
        repository::CreateBalanceUpdateParams {
            account_id: input.account_id,
            amount_minor: input.amount_minor,
            event_date: input.event_date,
            note: input.note,
        },
    )?;
    Ok(event_id)
}

#[tauri::command]
pub fn get_accounts_snapshot(
    state: State<'_, AppState>,
    date_iso: String,
    person_id: Option<i64>,
) -> Result<Vec<SnapshotRow>, AppError> {
    let selected_datetime = if date_iso.len() == 10 {
        format!("{}T23:59:59", date_iso)
    } else {
        date_iso
    };
    let conn = state.conn()?;
    let snapshot = repository::get_accounts_snapshot(
        &conn,
        GetSnapshotParams {
            selected_datetime,
            person_id,
        },
    )?;
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
            person_id: filter.person_id,
            unmatched_only: filter.unmatched_only.unwrap_or(false),
        },
    )?;
    Ok(result)
}

#[tauri::command]
pub fn update_event(state: State<'_, AppState>, input: UpdateEventInput) -> Result<(), AppError> {
    crate::shared::validate_event_date(&input.event_date)?;
    let conn = state.conn()?;
    repository::check_event_not_system_generated(&conn, input.event_id)?;
    repository::check_event_split_group_date_conflict(
        &conn,
        repository::CheckEventSplitGroupDateConflictParams {
            event_id: input.event_id,
            new_date: input.event_date.clone(),
        },
    )?;
    repository::update_event(
        &conn,
        repository::UpdateEventParams {
            event_id: input.event_id,
            amount_minor: input.amount_minor,
            event_date: input.event_date,
            note: input.note,
        },
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_event(state: State<'_, AppState>, event_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::check_event_not_system_generated(&conn, event_id)?;

    // If this is a prepaid_expense parent, cascade-delete its system-generated children first.
    let event_type = repository::get_active_event_type(&conn, event_id)?;

    with_savepoint_app(&conn, || {
        if event_type == "prepaid_expense" {
            repository::prepaid_expenses::delete_prepaid_child_events(&conn, event_id)?;
        }
        repository::delete_event(&conn, event_id)?;
        Ok(())
    })?;
    Ok(())
}

#[tauri::command]
pub fn delete_split_group(state: State<'_, AppState>, split_group_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::check_split_group_not_system_generated(&conn, split_group_id)?;
    repository::delete_split_group(&conn, split_group_id).map_err(AppError::from)?;
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
        repository::BulkCreateBalanceUpdatesParams {
            entries,
            event_date: input.event_date,
            note: input.note,
        },
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
        repository::CreateSplitGroupWithLegsParams {
            account_id: input.account_id,
            group_note: input.group_note,
            legs: input.legs,
        },
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
    repository::update_split_group_date(
        &conn,
        repository::UpdateSplitGroupDateParams {
            split_group_id: input.split_group_id,
            new_date: input.new_date,
        },
    )?;
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
    let conn = state.conn()?;
    update_transfer_with_conn(&conn, input)
}

fn update_transfer_with_conn(
    conn: &rusqlite::Connection,
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
    for (event_id, new_date) in [
        (input.from_event_id, &input.from_date),
        (input.to_event_id, &input.to_date),
    ] {
        repository::check_event_split_group_date_conflict(
            conn,
            repository::CheckEventSplitGroupDateConflictParams {
                event_id,
                new_date: new_date.clone(),
            },
        )?;
    }
    repository::update_transfer(
        conn,
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

// ---------------------------------------------------------------------------
// Taxable event commands (revenue / expense)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaxableEventInput {
    pub person_id: i64,
    pub event_type: String,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_until: Option<String>,
    pub reclaimed_vat: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitTaxableLegInput {
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_until: Option<String>,
    pub reclaimed_vat: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaxableSplitGroupInput {
    pub person_id: i64,
    pub event_type: String,
    pub group_note: Option<String>,
    pub legs: Vec<SplitTaxableLegInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaxableEventInput {
    pub event_id: i64,
    pub event_type: String,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_until: Option<String>,
    pub reclaimed_vat: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedSplitLegInput {
    pub event_id: i64,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_until: Option<String>,
    pub reclaimed_vat: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSplitLegInput {
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_until: Option<String>,
    pub reclaimed_vat: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaxableSplitGroupInput {
    pub split_group_id: i64,
    pub event_type: String,
    pub group_note: Option<String>,
    pub updated_legs: Vec<UpdatedSplitLegInput>,
    pub new_legs: Vec<NewSplitLegInput>,
    pub removed_leg_ids: Vec<i64>,
}

fn validate_event_type_taxable(event_type: &str) -> Result<(), AppError> {
    if event_type != "revenue" && event_type != "expense" && event_type != "prepaid_expense" {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "event_type must be 'revenue', 'expense', or 'prepaid_expense'".into(),
        });
    }
    Ok(())
}

/// Prepaid expenses generate children per event, so they cannot be split.
fn validate_split_group_event_type(event_type: &str) -> Result<(), AppError> {
    if event_type != "revenue" && event_type != "expense" {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "A split group's event_type must be 'revenue' or 'expense'".into(),
        });
    }
    Ok(())
}

fn validate_taxable_field_ranges(
    vat_rate_bps: Option<i64>,
    vat_reclaimable_pct_bps: Option<i64>,
    expense_deductible_pct_bps: Option<i64>,
) -> Result<(), AppError> {
    for (name, val) in [
        ("vat_rate_bps", vat_rate_bps),
        ("vat_reclaimable_pct_bps", vat_reclaimable_pct_bps),
        ("expense_deductible_pct_bps", expense_deductible_pct_bps),
    ] {
        if let Some(v) = val {
            if !(0..=10000).contains(&v) {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: format!("{} must be in range [0, 10000] bps", name),
                });
            }
        }
    }
    Ok(())
}

fn validate_expense_only_fields(
    event_type: &str,
    vat_reclaimable_pct_bps: Option<i64>,
    expense_deductible_pct_bps: Option<i64>,
    prepaid_until: Option<&str>,
) -> Result<(), AppError> {
    if event_type != "expense"
        && event_type != "prepaid_expense"
        && (vat_reclaimable_pct_bps.is_some() || expense_deductible_pct_bps.is_some())
    {
        return Err(AppError {
            code: "VALIDATION".into(),
            message:
                "vat_reclaimable_pct_bps and expense_deductible_pct_bps are only valid for expense or prepaid_expense events"
                    .into(),
        });
    }
    if event_type != "prepaid_expense" && prepaid_until.is_some() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "prepaid_until is only valid for prepaid_expense events".into(),
        });
    }
    Ok(())
}

fn validate_prepaid_until(
    event_type: &str,
    event_date: &str,
    prepaid_until: Option<&str>,
) -> Result<(), AppError> {
    if event_type != "prepaid_expense" {
        return Ok(());
    }
    let until_str = prepaid_until.ok_or_else(|| AppError {
        code: "VALIDATION".into(),
        message: "prepaid_until is required for prepaid_expense events".into(),
    })?;
    let expense_date = event_date
        .get(..10)
        .and_then(|day| NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
        .ok_or_else(|| AppError {
            code: "VALIDATION".into(),
            message: format!("Invalid event_date format: {}", event_date),
        })?;
    let until_date = NaiveDate::parse_from_str(until_str, "%Y-%m-%d").map_err(|_| AppError {
        code: "VALIDATION".into(),
        message: format!("Invalid prepaid_until format: {}", until_str),
    })?;
    if until_date.year() <= expense_date.year() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "prepaid_until year must be strictly greater than event_date year".into(),
        });
    }
    Ok(())
}

#[tauri::command]
pub fn create_taxable_event(
    state: State<'_, AppState>,
    input: CreateTaxableEventInput,
) -> Result<i64, AppError> {
    let conn = state.conn()?;
    create_taxable_event_inner(&conn, input)
}

fn create_taxable_event_inner(
    conn: &Connection,
    input: CreateTaxableEventInput,
) -> Result<i64, AppError> {
    validate_event_type_taxable(&input.event_type)?;
    crate::shared::validate_event_date(&input.event_date)?;
    validate_expense_only_fields(
        &input.event_type,
        input.vat_reclaimable_pct_bps,
        input.expense_deductible_pct_bps,
        input.prepaid_until.as_deref(),
    )?;
    validate_taxable_field_ranges(
        input.vat_rate_bps,
        input.vat_reclaimable_pct_bps,
        input.expense_deductible_pct_bps,
    )?;
    validate_prepaid_until(
        &input.event_type,
        &input.event_date,
        input.prepaid_until.as_deref(),
    )?;
    let account_id = resolve_default_taxable_account(conn, input.person_id, &input.event_type)?;
    let resolved_reclaimed_vat = if (input.event_type == "expense"
        || input.event_type == "prepaid_expense")
        && input.reclaimed_vat.is_none()
    {
        Some(get_person_vat_payer(conn, input.person_id)?)
    } else {
        input.reclaimed_vat
    };
    let event_type_is_prepaid = input.event_type == "prepaid_expense";
    with_savepoint_app(conn, || {
        let id = repository::create_taxable_event(
            conn,
            repository::CreateTaxableEventParams {
                account_id,
                event_type: input.event_type,
                amount_minor: input.amount_minor,
                event_date: input.event_date.clone(),
                note: input.note.clone(),
                vat_rate_bps: input.vat_rate_bps,
                vat_reclaimable_pct_bps: input.vat_reclaimable_pct_bps,
                expense_deductible_pct_bps: input.expense_deductible_pct_bps,
                prepaid_until: input.prepaid_until.clone(),
                reclaimed_vat: resolved_reclaimed_vat,
            },
        )?;
        if event_type_is_prepaid {
            if let Some(until) = input.prepaid_until {
                let ctx = repository::prepaid_expenses::PrepaidExpenseContext {
                    parent_event_id: id,
                    account_id,
                    expense_date: input.event_date,
                    prepaid_until: until,
                    total_amount_minor: input.amount_minor,
                    expense_deductible_pct_bps: input.expense_deductible_pct_bps,
                    note: input.note,
                    vat_rate_bps: input.vat_rate_bps,
                    vat_reclaimable_pct_bps: input.vat_reclaimable_pct_bps,
                    reclaimed_vat: resolved_reclaimed_vat.unwrap_or(false),
                };
                repository::prepaid_expenses::generate_prepaid_child_events(conn, &ctx)?;
            }
        }
        Ok(id)
    })
}

#[tauri::command]
pub fn create_taxable_split_group(
    state: State<'_, AppState>,
    input: CreateTaxableSplitGroupInput,
) -> Result<i64, AppError> {
    let conn = state.conn()?;
    create_taxable_split_group_inner(&conn, input)
}

fn create_taxable_split_group_inner(
    conn: &Connection,
    input: CreateTaxableSplitGroupInput,
) -> Result<i64, AppError> {
    validate_split_group_event_type(&input.event_type)?;
    if input.legs.len() < 2 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "A split group requires at least 2 legs".into(),
        });
    }
    for leg in &input.legs {
        crate::shared::validate_event_date(&leg.event_date)?;
        validate_expense_only_fields(
            &input.event_type,
            leg.vat_reclaimable_pct_bps,
            leg.expense_deductible_pct_bps,
            leg.prepaid_until.as_deref(),
        )?;
        validate_taxable_field_ranges(
            leg.vat_rate_bps,
            leg.vat_reclaimable_pct_bps,
            leg.expense_deductible_pct_bps,
        )?;
    }
    let account_id = resolve_default_taxable_account(conn, input.person_id, &input.event_type)?;
    let default_reclaimed_vat = if input.event_type == "expense" {
        Some(get_person_vat_payer(conn, input.person_id)?)
    } else {
        None
    };
    let legs = input
        .legs
        .into_iter()
        .map(|l| repository::TaxableEventLeg {
            amount_minor: l.amount_minor,
            event_date: l.event_date,
            note: l.note,
            vat_rate_bps: l.vat_rate_bps,
            vat_reclaimable_pct_bps: l.vat_reclaimable_pct_bps,
            expense_deductible_pct_bps: l.expense_deductible_pct_bps,
            prepaid_until: l.prepaid_until,
            reclaimed_vat: l.reclaimed_vat.or(default_reclaimed_vat),
        })
        .collect();
    let id = repository::create_taxable_split_group_with_legs(
        conn,
        repository::CreateTaxableSplitGroupWithLegsParams {
            account_id,
            event_type: input.event_type,
            group_note: input.group_note,
            legs,
        },
    )?;
    Ok(id)
}

#[tauri::command]
pub fn update_taxable_event(
    state: State<'_, AppState>,
    input: UpdateTaxableEventInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    update_taxable_event_inner(&conn, input)
}

fn update_taxable_event_inner(
    conn: &Connection,
    input: UpdateTaxableEventInput,
) -> Result<(), AppError> {
    validate_event_type_taxable(&input.event_type)?;
    validate_expense_only_fields(
        &input.event_type,
        input.vat_reclaimable_pct_bps,
        input.expense_deductible_pct_bps,
        input.prepaid_until.as_deref(),
    )?;
    validate_taxable_field_ranges(
        input.vat_rate_bps,
        input.vat_reclaimable_pct_bps,
        input.expense_deductible_pct_bps,
    )?;
    crate::shared::validate_event_date(&input.event_date)?;
    validate_prepaid_until(
        &input.event_type,
        &input.event_date,
        input.prepaid_until.as_deref(),
    )?;
    repository::check_event_not_system_generated(conn, input.event_id)?;
    repository::check_event_split_group_date_conflict(
        conn,
        repository::CheckEventSplitGroupDateConflictParams {
            event_id: input.event_id,
            new_date: input.event_date.clone(),
        },
    )?;
    with_savepoint_app(conn, || {
        let updated = repository::update_taxable_event(
            conn,
            repository::UpdateTaxableEventParams {
                event_id: input.event_id,
                event_type: input.event_type.clone(),
                amount_minor: input.amount_minor,
                event_date: input.event_date.clone(),
                note: input.note.clone(),
                vat_rate_bps: input.vat_rate_bps,
                vat_reclaimable_pct_bps: input.vat_reclaimable_pct_bps,
                expense_deductible_pct_bps: input.expense_deductible_pct_bps,
                prepaid_until: input.prepaid_until.clone(),
                reclaimed_vat: input.reclaimed_vat,
            },
        )?;
        // The repository has confirmed that input.event_type is the stored type.
        if input.event_type != "prepaid_expense" {
            return Ok(());
        }
        if let Some(prepaid_until) = input.prepaid_until {
            let ctx = repository::prepaid_expenses::PrepaidExpenseContext {
                parent_event_id: input.event_id,
                account_id: updated.account_id,
                expense_date: input.event_date,
                prepaid_until,
                total_amount_minor: input.amount_minor,
                expense_deductible_pct_bps: input.expense_deductible_pct_bps,
                note: input.note,
                vat_rate_bps: input.vat_rate_bps,
                vat_reclaimable_pct_bps: input.vat_reclaimable_pct_bps,
                reclaimed_vat: updated.reclaimed_vat.unwrap_or(false),
            };
            repository::prepaid_expenses::regenerate_prepaid_child_events(conn, &ctx)?;
        }
        Ok(())
    })
}

#[tauri::command]
pub fn update_taxable_split_group(
    state: State<'_, AppState>,
    input: UpdateTaxableSplitGroupInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    update_taxable_split_group_inner(&conn, input)
}

fn update_taxable_split_group_inner(
    conn: &Connection,
    input: UpdateTaxableSplitGroupInput,
) -> Result<(), AppError> {
    validate_split_group_event_type(&input.event_type)?;
    for leg in &input.updated_legs {
        crate::shared::validate_event_date(&leg.event_date)?;
        validate_expense_only_fields(
            &input.event_type,
            leg.vat_reclaimable_pct_bps,
            leg.expense_deductible_pct_bps,
            leg.prepaid_until.as_deref(),
        )?;
        validate_taxable_field_ranges(
            leg.vat_rate_bps,
            leg.vat_reclaimable_pct_bps,
            leg.expense_deductible_pct_bps,
        )?;
    }
    for leg in &input.new_legs {
        crate::shared::validate_event_date(&leg.event_date)?;
        validate_expense_only_fields(
            &input.event_type,
            leg.vat_reclaimable_pct_bps,
            leg.expense_deductible_pct_bps,
            leg.prepaid_until.as_deref(),
        )?;
        validate_taxable_field_ranges(
            leg.vat_rate_bps,
            leg.vat_reclaimable_pct_bps,
            leg.expense_deductible_pct_bps,
        )?;
    }
    repository::check_split_group_not_system_generated(conn, input.split_group_id)?;
    let updated_legs = input
        .updated_legs
        .into_iter()
        .map(|l| repository::UpdatedSplitLeg {
            event_id: l.event_id,
            amount_minor: l.amount_minor,
            event_date: l.event_date,
            note: l.note,
            vat_rate_bps: l.vat_rate_bps,
            vat_reclaimable_pct_bps: l.vat_reclaimable_pct_bps,
            expense_deductible_pct_bps: l.expense_deductible_pct_bps,
            prepaid_until: l.prepaid_until,
            reclaimed_vat: l.reclaimed_vat,
        })
        .collect();
    let new_legs = input
        .new_legs
        .into_iter()
        .map(|l| repository::NewSplitLeg {
            amount_minor: l.amount_minor,
            event_date: l.event_date,
            note: l.note,
            vat_rate_bps: l.vat_rate_bps,
            vat_reclaimable_pct_bps: l.vat_reclaimable_pct_bps,
            expense_deductible_pct_bps: l.expense_deductible_pct_bps,
            prepaid_until: l.prepaid_until,
            reclaimed_vat: l.reclaimed_vat,
        })
        .collect();
    repository::update_taxable_split_group(
        conn,
        repository::UpdateTaxableSplitGroupParams {
            split_group_id: input.split_group_id,
            event_type: input.event_type,
            group_note: input.group_note,
            updated_legs,
            new_legs,
            removed_leg_ids: input.removed_leg_ids,
        },
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Taxable-cashflow link commands (Phase 2)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkCashflowsInput {
    pub taxable_event_id: i64,
    pub cashflow_event_ids: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlinkCashflowInput {
    pub taxable_event_id: i64,
    pub cashflow_event_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEligibleCashflowsInput {
    pub person_id: i64,
    pub amount_minor: Option<i64>,
    pub exclude_already_linked: bool,
}

#[tauri::command]
pub fn link_cashflows_to_taxable(
    state: State<'_, AppState>,
    input: LinkCashflowsInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::link_cashflows_to_taxable(
        &conn,
        repository::LinkCashflowsParams {
            taxable_event_id: input.taxable_event_id,
            cashflow_event_ids: input.cashflow_event_ids,
        },
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkCashflowsToSplitGroupInput {
    pub split_group_id: i64,
    pub cashflow_event_ids: Vec<i64>,
}

#[tauri::command]
pub fn link_cashflows_to_split_group(
    state: State<'_, AppState>,
    input: LinkCashflowsToSplitGroupInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::link_cashflows_to_split_group(
        &conn,
        repository::LinkCashflowsToSplitGroupParams {
            split_group_id: input.split_group_id,
            cashflow_event_ids: input.cashflow_event_ids,
        },
    )?;
    Ok(())
}

#[tauri::command]
pub fn unlink_cashflow_from_taxable(
    state: State<'_, AppState>,
    input: UnlinkCashflowInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::unlink_cashflow_from_taxable(
        &conn,
        input.taxable_event_id,
        input.cashflow_event_id,
    )?;
    Ok(())
}

#[tauri::command]
pub fn list_linked_cashflows(
    state: State<'_, AppState>,
    taxable_event_id: i64,
) -> Result<Vec<EventWithData>, AppError> {
    let conn = state.conn()?;
    let events =
        repository::list_linked_cashflows(&conn, taxable_event_id).map_err(AppError::from)?;
    Ok(events)
}

#[tauri::command]
pub fn list_eligible_cashflows(
    state: State<'_, AppState>,
    input: ListEligibleCashflowsInput,
) -> Result<Vec<EventWithData>, AppError> {
    let conn = state.conn()?;
    let events = repository::list_eligible_cashflows(
        &conn,
        repository::EligibleCashflowsParams {
            person_id: input.person_id,
            amount_minor: input.amount_minor,
            exclude_already_linked: input.exclude_already_linked,
        },
    )
    .map_err(AppError::from)?;
    Ok(events)
}

#[tauri::command]
pub fn get_unmatched_cashflow_count(
    state: State<'_, AppState>,
    person_id: Option<i64>,
) -> Result<i64, AppError> {
    let conn = state.conn()?;
    repository::count_unmatched_cashflows(&conn, person_id).map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::persons::repository::{create_person, CreatePersonParams};
    use crate::features::transactions::models::SplitGroupEntry;
    use rusqlite::{params, Connection};

    fn mk_account(conn: &Connection) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: "Test Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn leg(counterpart_account_id: Option<i64>, amount_minor: i64) -> SplitGroupEntry {
        SplitGroupEntry {
            amount_minor,
            event_date: "2026-03-01".to_owned(),
            note: None,
            counterpart_account_id,
            bucket_id: None,
            original_currency_id: None,
            original_amount_minor: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
        }
    }

    /// Creates a split group on a fresh account with a transfer leg and a plain cashflow leg,
    /// both dated 2026-03-01, and returns the transfer's `(from_event_id, to_event_id)`.
    fn seed_split_group_with_transfer(conn: &Connection) -> (i64, i64) {
        let (source, counterpart) = (mk_account(conn), mk_account(conn));
        repository::create_split_group_with_legs(
            conn,
            repository::CreateSplitGroupWithLegsParams {
                account_id: source,
                group_note: None,
                legs: vec![leg(Some(counterpart), -4000), leg(None, -1000)],
            },
        )
        .unwrap();
        conn.query_row(
            "SELECT id, linked_event_id FROM event WHERE account_id = ?1 AND event_type = 'transfer'",
            params![source],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    }

    fn transfer_input(
        from_event_id: i64,
        to_event_id: i64,
        from_date: &str,
        to_date: &str,
    ) -> UpdateTransferInput {
        UpdateTransferInput {
            from_event_id,
            to_event_id,
            from_date: from_date.to_owned(),
            to_date: to_date.to_owned(),
            amount_minor: -5000,
            to_amount_minor: 5000,
            note: None,
            original_currency_id: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
        }
    }

    fn latest_date_and_amount(conn: &Connection, event_id: i64) -> (String, i64) {
        conn.query_row(
            "SELECT ed.event_date, ed.amount_minor FROM event e JOIN event_data ed ON ed.id = e.latest_data_id WHERE e.id = ?1",
            params![event_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    }

    fn mk_person(conn: &Connection, vat_payer: bool) -> i64 {
        create_person(
            conn,
            CreatePersonParams {
                name: "Alice".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer,
            },
        )
        .expect("create person failed")
    }

    /// 1,230.00 gross with 23% VAT, fully reclaimable, prepaid for a year.
    fn prepaid_input(person_id: i64) -> CreateTaxableEventInput {
        CreateTaxableEventInput {
            person_id,
            event_type: "prepaid_expense".to_owned(),
            amount_minor: 123_000,
            event_date: "2024-07-01".to_owned(),
            note: None,
            vat_rate_bps: Some(2300),
            vat_reclaimable_pct_bps: Some(10000),
            expense_deductible_pct_bps: None,
            prepaid_until: Some("2025-06-30".to_owned()),
            reclaimed_vat: None,
        }
    }

    fn prepaid_update_input(event_id: i64, amount_minor: i64) -> UpdateTaxableEventInput {
        UpdateTaxableEventInput {
            event_id,
            event_type: "prepaid_expense".to_owned(),
            amount_minor,
            event_date: "2024-07-01".to_owned(),
            note: None,
            vat_rate_bps: Some(2300),
            vat_reclaimable_pct_bps: Some(10000),
            expense_deductible_pct_bps: None,
            prepaid_until: Some("2025-06-30".to_owned()),
            reclaimed_vat: None,
        }
    }

    fn children_sum(conn: &Connection, parent_event_id: i64) -> i64 {
        conn.query_row(
            "SELECT COALESCE(SUM(ed.amount_minor), 0)
             FROM event e
             JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.linked_prepaid_event_id = ?1 AND e.is_system_generated = 1",
            params![parent_event_id],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn update_transfer_rejects_moving_a_split_group_leg_to_another_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (from, to) = seed_split_group_with_transfer(&conn);

        let err =
            update_transfer_with_conn(&conn, transfer_input(from, to, "2026-03-05", "2026-03-05"))
                .unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("split group"));
        assert_eq!(
            latest_date_and_amount(&conn, from),
            ("2026-03-01".to_owned(), -4000)
        );
        assert_eq!(
            latest_date_and_amount(&conn, to),
            ("2026-03-01".to_owned(), 4000)
        );
    }

    #[test]
    fn update_transfer_checks_the_to_leg_date_too() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (from, to) = seed_split_group_with_transfer(&conn);
        // create_split_group_with_legs only tags the source leg, so tag the counterpart directly.
        conn.execute(
            "UPDATE event SET split_group_id = (SELECT split_group_id FROM event WHERE id = ?1) WHERE id = ?2",
            params![from, to],
        )
        .unwrap();

        let err =
            update_transfer_with_conn(&conn, transfer_input(from, to, "2026-03-01", "2026-03-05"))
                .unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert_eq!(
            latest_date_and_amount(&conn, to),
            ("2026-03-01".to_owned(), 4000)
        );
    }

    #[test]
    fn update_transfer_allows_editing_a_split_group_leg_without_changing_its_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (from, to) = seed_split_group_with_transfer(&conn);

        update_transfer_with_conn(&conn, transfer_input(from, to, "2026-03-01", "2026-03-01"))
            .unwrap();

        assert_eq!(
            latest_date_and_amount(&conn, from),
            ("2026-03-01".to_owned(), -5000)
        );
        assert_eq!(
            latest_date_and_amount(&conn, to),
            ("2026-03-01".to_owned(), 5000)
        );
    }

    #[test]
    fn update_transfer_moves_a_transfer_that_is_not_in_a_split_group() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (source, counterpart) = (mk_account(&conn), mk_account(&conn));
        let (from, to) = repository::create_transfer(
            &conn,
            &repository::CashflowEntry {
                account_id: source,
                amount_minor: -4000,
                event_date: "2026-03-01".to_owned(),
                note: None,
                counterpart_account_id: Some(counterpart),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        update_transfer_with_conn(&conn, transfer_input(from, to, "2026-03-05", "2026-03-05"))
            .unwrap();

        assert_eq!(
            latest_date_and_amount(&conn, from),
            ("2026-03-05".to_owned(), -5000)
        );
        assert_eq!(
            latest_date_and_amount(&conn, to),
            ("2026-03-05".to_owned(), 5000)
        );
    }

    #[test]
    fn prepaid_for_vat_payer_splits_gross_minus_reclaimed_vat() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, true);

        let id = create_taxable_event_inner(&conn, prepaid_input(person_id)).unwrap();

        assert_eq!(children_sum(&conn, id), 100_000);
    }

    #[test]
    fn prepaid_for_non_vat_payer_splits_full_gross() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, false);

        let id = create_taxable_event_inner(&conn, prepaid_input(person_id)).unwrap();

        assert_eq!(children_sum(&conn, id), 123_000);
    }

    #[test]
    fn prepaid_update_without_reclaimed_vat_resolves_it_like_create() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, true);
        let id = create_taxable_event_inner(&conn, prepaid_input(person_id)).unwrap();

        update_taxable_event_inner(&conn, prepaid_update_input(id, 246_000)).unwrap();

        assert_eq!(children_sum(&conn, id), 200_000);
    }

    fn expense_input(person_id: i64) -> CreateTaxableEventInput {
        CreateTaxableEventInput {
            event_type: "expense".to_owned(),
            prepaid_until: None,
            ..prepaid_input(person_id)
        }
    }

    fn children_count(conn: &Connection, parent_event_id: i64) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM event WHERE linked_prepaid_event_id = ?1",
            params![parent_event_id],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn update_rejects_prepaid_sent_as_expense() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, false);
        let id = create_taxable_event_inner(&conn, prepaid_input(person_id)).unwrap();

        let err = update_taxable_event_inner(
            &conn,
            UpdateTaxableEventInput {
                event_type: "expense".to_owned(),
                prepaid_until: None,
                ..prepaid_update_input(id, 123_000)
            },
        )
        .unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        let stored = repository::get_event_by_id(&conn, id).unwrap().unwrap();
        assert_eq!(stored.prepaid_until.as_deref(), Some("2025-06-30"));
    }

    #[test]
    fn update_rejects_expense_sent_as_prepaid_and_generates_no_children() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, false);
        let id = create_taxable_event_inner(&conn, expense_input(person_id)).unwrap();

        let err = update_taxable_event_inner(&conn, prepaid_update_input(id, 123_000)).unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert_eq!(children_count(&conn, id), 0);
    }

    fn fail_child_inserts(conn: &Connection) {
        conn.execute_batch(
            "CREATE TEMP TRIGGER fail_child_insert BEFORE INSERT ON event
             WHEN NEW.is_system_generated = 1
             BEGIN SELECT RAISE(ABORT, 'child insert failed'); END;",
        )
        .unwrap();
    }

    #[test]
    fn create_prepaid_rolls_back_parent_when_children_fail() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, false);
        fail_child_inserts(&conn);

        assert!(create_taxable_event_inner(&conn, prepaid_input(person_id)).is_err());

        let parents: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event WHERE event_type = 'prepaid_expense'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(parents, 0);
    }

    #[test]
    fn update_prepaid_rolls_back_parent_when_children_fail() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, false);
        let id = create_taxable_event_inner(&conn, prepaid_input(person_id)).unwrap();
        fail_child_inserts(&conn);

        assert!(update_taxable_event_inner(&conn, prepaid_update_input(id, 246_000)).is_err());

        let stored = repository::get_event_by_id(&conn, id).unwrap().unwrap();
        assert_eq!(stored.amount_minor, 123_000);
        assert_eq!(children_sum(&conn, id), 123_000);
    }

    fn split_leg(amount_minor: i64) -> SplitTaxableLegInput {
        SplitTaxableLegInput {
            amount_minor,
            event_date: "2024-07-01".to_owned(),
            note: None,
            vat_rate_bps: None,
            vat_reclaimable_pct_bps: None,
            expense_deductible_pct_bps: None,
            prepaid_until: None,
            reclaimed_vat: None,
        }
    }

    fn split_input(person_id: i64, event_type: &str) -> CreateTaxableSplitGroupInput {
        CreateTaxableSplitGroupInput {
            person_id,
            event_type: event_type.to_owned(),
            group_note: None,
            legs: vec![split_leg(3000), split_leg(2000)],
        }
    }

    fn split_update_input(split_group_id: i64, event_type: &str) -> UpdateTaxableSplitGroupInput {
        UpdateTaxableSplitGroupInput {
            split_group_id,
            event_type: event_type.to_owned(),
            group_note: Some("changed".to_owned()),
            updated_legs: vec![],
            new_legs: vec![],
            removed_leg_ids: vec![],
        }
    }

    fn split_group_note(conn: &Connection, split_group_id: i64) -> Option<String> {
        conn.query_row(
            "SELECT note FROM split_group WHERE id = ?1",
            params![split_group_id],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn split_group_create_rejects_prepaid_expense() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, false);

        let err =
            create_taxable_split_group_inner(&conn, split_input(person_id, "prepaid_expense"))
                .unwrap_err();

        assert_eq!(err.code, "VALIDATION");
    }

    #[test]
    fn split_group_update_rejects_prepaid_expense() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, false);
        let group_id =
            create_taxable_split_group_inner(&conn, split_input(person_id, "expense")).unwrap();

        let err = update_taxable_split_group_inner(
            &conn,
            split_update_input(group_id, "prepaid_expense"),
        )
        .unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert_eq!(split_group_note(&conn, group_id), None);
    }

    #[test]
    fn split_group_update_rejects_type_mismatch() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, false);
        let group_id =
            create_taxable_split_group_inner(&conn, split_input(person_id, "expense")).unwrap();

        let err = update_taxable_split_group_inner(&conn, split_update_input(group_id, "revenue"))
            .unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert_eq!(split_group_note(&conn, group_id), None);
    }

    #[test]
    fn split_group_update_with_matching_type_succeeds() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, false);
        let group_id =
            create_taxable_split_group_inner(&conn, split_input(person_id, "revenue")).unwrap();

        update_taxable_split_group_inner(&conn, split_update_input(group_id, "revenue")).unwrap();

        assert_eq!(
            split_group_note(&conn, group_id).as_deref(),
            Some("changed")
        );
    }

    #[test]
    fn prepaid_until_is_rejected_for_plain_expense() {
        let conn = initialize_in_memory().unwrap();
        let person_id = mk_person(&conn, false);

        let err = create_taxable_event_inner(
            &conn,
            CreateTaxableEventInput {
                prepaid_until: Some("2025-06-30".to_owned()),
                ..expense_input(person_id)
            },
        )
        .unwrap_err();

        assert_eq!(err.code, "VALIDATION");
    }
}
