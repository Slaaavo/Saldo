use crate::error::AppError;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

use super::models::{Currency, FxRateRow};
use super::repository;

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

#[tauri::command]
pub fn list_currencies(
    state: State<'_, AppState>,
    include_custom: Option<bool>,
) -> Result<Vec<Currency>, AppError> {
    let conn = state.conn()?;
    let currencies = repository::list_currencies(&conn, include_custom)?;
    Ok(currencies)
}

#[tauri::command]
pub fn get_consolidation_currency(state: State<'_, AppState>) -> Result<Currency, AppError> {
    let conn = state.conn()?;
    let currency = repository::get_consolidation_currency(&conn)?;
    Ok(currency)
}

#[tauri::command]
pub fn set_consolidation_currency(
    state: State<'_, AppState>,
    input: SetConsolidationCurrencyInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
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
    crate::shared::validate_event_date(&input.date)?;
    let conn = state.conn()?;
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
    let conn = state.conn()?;
    let rates = repository::list_fx_rates(&conn, date.as_deref())?;
    Ok(rates)
}

#[tauri::command]
pub fn get_missing_rate_dates(state: State<'_, AppState>) -> Result<Vec<String>, AppError> {
    let conn = state.conn()?;
    let consolidation = repository::get_consolidation_currency(&conn)?;
    let dates = repository::get_dates_needing_fx_rates(&conn, consolidation.id)?;
    Ok(dates)
}

#[tauri::command]
pub async fn fetch_fx_rates(
    state: tauri::State<'_, crate::AppState>,
    date_iso: Option<String>,
    force: Option<bool>,
) -> Result<Vec<FxRateRow>, AppError> {
    let store_date =
        date_iso.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    crate::smart_fetch_fx_rates(&state, &store_date, force.unwrap_or(false)).await
}
