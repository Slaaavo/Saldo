use crate::{error::AppError, models::*, repository, AppState};
use chrono::NaiveDate;
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
    pub account_type: Option<String>,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetConsolidationCurrencyInput {
    pub currency_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetFxRateManualInput {
    pub from_currency_id: i64,
    pub to_currency_id: i64,
    pub date: String,
    pub rate_mantissa: i64,
    pub rate_exponent: i64,
}

fn validate_event_date(date_str: &str) -> Result<(), AppError> {
    if date_str.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "event_date is required".into(),
        });
    }
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|_| AppError {
        code: "VALIDATION".into(),
        message: "event_date must be a valid date in YYYY-MM-DD format".into(),
    })?;
    Ok(())
}

#[tauri::command]
pub fn create_balance_update(
    state: State<'_, AppState>,
    input: CreateBalanceUpdateInput,
) -> Result<i64, AppError> {
    validate_event_date(&input.event_date)?;
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
    let events = repository::list_events(&conn, filter.account_id, filter.before_date.as_deref())?;
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
    let account_type = input.account_type.as_deref().unwrap_or("account");
    if account_type != "account" && account_type != "bucket" {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "account_type must be 'account' or 'bucket'".into(),
        });
    }
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    let id = repository::create_account(
        &conn,
        input.name.trim(),
        input.currency_id,
        account_type,
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
    validate_event_date(&input.event_date)?;
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

#[tauri::command]
pub fn bulk_create_balance_updates(
    state: State<'_, AppState>,
    input: BulkCreateBalanceUpdatesInput,
) -> Result<Vec<i64>, AppError> {
    validate_event_date(&input.event_date)?;
    if input.entries.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "At least one balance entry is required".into(),
        });
    }
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
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
pub fn list_currencies(state: State<'_, AppState>) -> Result<Vec<Currency>, AppError> {
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    let currencies = repository::list_currencies(&conn)?;
    Ok(currencies)
}

#[tauri::command]
pub fn get_consolidation_currency(state: State<'_, AppState>) -> Result<Currency, AppError> {
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    let currency = repository::get_consolidation_currency(&conn)?;
    Ok(currency)
}

#[tauri::command]
pub fn set_consolidation_currency(
    state: State<'_, AppState>,
    input: SetConsolidationCurrencyInput,
) -> Result<(), AppError> {
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    repository::set_consolidation_currency(&conn, input.currency_id)?;
    Ok(())
}

#[tauri::command]
pub fn set_fx_rate_manual(
    state: State<'_, AppState>,
    input: SetFxRateManualInput,
) -> Result<(), AppError> {
    if input.rate_mantissa == 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "rate_mantissa must not be zero".into(),
        });
    }
    validate_event_date(&input.date)?;
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    repository::set_fx_rate_manual(
        &conn,
        input.from_currency_id,
        input.to_currency_id,
        &input.date,
        input.rate_mantissa,
        input.rate_exponent,
    )?;
    Ok(())
}

#[tauri::command]
pub fn list_fx_rates(
    state: State<'_, AppState>,
    date: Option<String>,
) -> Result<Vec<FxRateRow>, AppError> {
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    let rates = repository::list_fx_rates(&conn, date.as_deref())?;
    Ok(rates)
}

const SETTING_ALLOWLIST: &[&str] = &["consolidation_currency_code", "oxr_app_id"];

fn validate_setting_key(key: &str) -> Result<(), AppError> {
    if SETTING_ALLOWLIST.contains(&key) {
        Ok(())
    } else {
        Err(AppError {
            code: "VALIDATION".into(),
            message: format!(
                "Unknown setting key '{}'. Allowed: {:?}",
                key, SETTING_ALLOWLIST
            ),
        })
    }
}

#[tauri::command]
pub fn get_app_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, AppError> {
    validate_setting_key(&key)?;
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    let value = repository::get_app_setting(&conn, &key)?;
    Ok(value)
}

#[tauri::command]
pub fn set_app_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), AppError> {
    validate_setting_key(&key)?;
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    repository::set_app_setting(&conn, &key, &value)?;
    Ok(())
}

#[tauri::command]
pub fn get_missing_rate_dates(state: State<'_, AppState>) -> Result<Vec<String>, AppError> {
    let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
    let consolidation = repository::get_consolidation_currency(&conn)?;
    let dates = repository::get_dates_needing_fx_rates(&conn, consolidation.id)?;
    Ok(dates)
}

#[tauri::command]
pub async fn fetch_fx_rates(
    state: tauri::State<'_, crate::AppState>,
    date_iso: Option<String>,
) -> Result<Vec<FxRateRow>, AppError> {
    // 1. Read API key (lock → read → drop guard).
    let api_key = {
        let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
        repository::get_app_setting(&conn, "oxr_app_id")?
            .filter(|k| !k.is_empty())
            .ok_or_else(|| AppError {
                code: "CONFIG_ERROR".into(),
                message: "API key not configured. Set oxr_app_id in settings.".into(),
            })?
    };

    // 2. Read consolidation currency and active non-consolidation currencies.
    let (consolidation, active_currencies) = {
        let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
        let consolidation = repository::get_consolidation_currency(&conn)?;
        let active = repository::get_active_foreign_currencies(&conn, consolidation.id)?;
        (consolidation, active)
    };

    // 3. HTTP call — no DB lock held across the await.
    let oxr_response = crate::oxr::fetch_rates(&api_key, date_iso.as_deref()).await?;

    // 4. Determine the calendar date to store in the fx_rate table.
    let store_date = date_iso
        .clone()
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());

    // 5. Compute cross rate for each active currency.
    let mut rates: Vec<(String, i64, i64, i64, i64)> = Vec::new();
    for (code, id) in &active_currencies {
        match crate::oxr::compute_cross_rate(&oxr_response.rates, &consolidation.code, code) {
            Ok((m, e)) => rates.push((store_date.clone(), consolidation.id, *id, m, e)),
            Err(err) => eprintln!(
                "[fetch_fx_rates] cross rate error for {}: {}",
                code, err.message
            ),
        }
    }

    // 6. Upsert rates.
    {
        let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
        repository::upsert_fx_rates(&conn, &rates)?;
    }

    // 7. Return the stored rates for this date.
    let stored = {
        let conn = state.db.lock().map_err(|e| AppError::from(e.to_string()))?;
        repository::list_fx_rates(&conn, Some(&store_date))?
    };

    Ok(stored)
}
