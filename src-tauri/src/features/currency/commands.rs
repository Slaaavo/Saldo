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
    pub is_direct: bool,
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
    let conn = state.conn()?;
    set_fx_rate_manual_with_conn(&conn, input)
}

fn set_fx_rate_manual_with_conn(
    conn: &rusqlite::Connection,
    input: SetFxRateManualInput,
) -> Result<(), AppError> {
    if input.rate_mantissa == 0 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "rate_mantissa must not be zero".into(),
        });
    }
    crate::shared::validate_iso_date(&input.date, "date")?;
    repository::set_fx_rate_manual(
        conn,
        repository::SetFxRateManualParams {
            from_currency_id: input.from_currency_id,
            to_currency_id: input.to_currency_id,
            date: input.date,
            rate_mantissa: input.rate_mantissa,
            rate_exponent: input.rate_exponent,
            is_direct: input.is_direct,
        },
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
    let store_date = resolve_fx_fetch_date(date_iso)?;
    crate::smart_fetch_fx_rates(&state, &store_date, force.unwrap_or(false)).await
}

/// The date to store fetched FX rates under: `date_iso` when given, otherwise today.
fn resolve_fx_fetch_date(date_iso: Option<String>) -> Result<String, AppError> {
    match date_iso {
        Some(date) => {
            crate::shared::validate_iso_date(&date, "date_iso")?;
            Ok(date)
        }
        None => Ok(chrono::Local::now().format("%Y-%m-%d").to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;

    fn fx_input(date: &str) -> SetFxRateManualInput {
        SetFxRateManualInput {
            from_currency_id: 1,
            to_currency_id: 2,
            date: date.to_owned(),
            rate_mantissa: 10842,
            rate_exponent: -4,
            is_direct: false,
        }
    }

    fn stored_rate_dates(conn: &rusqlite::Connection) -> Vec<String> {
        let mut stmt = conn.prepare("SELECT date FROM fx_rate").unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn set_fx_rate_manual_accepts_a_padded_date() {
        let conn = initialize_in_memory().expect("DB init failed");

        set_fx_rate_manual_with_conn(&conn, fx_input("2027-01-09")).unwrap();

        assert_eq!(stored_rate_dates(&conn), ["2027-01-09"]);
    }

    #[test]
    fn set_fx_rate_manual_rejects_anything_but_a_padded_date() {
        let conn = initialize_in_memory().expect("DB init failed");

        for date in [
            "2027-1-9",
            "2027-01-9",
            "2027-01-09T00:00:00",
            "09-01-2027",
            "",
        ] {
            let err = set_fx_rate_manual_with_conn(&conn, fx_input(date)).unwrap_err();
            assert_eq!(err.code, "VALIDATION", "{date:?}");
        }
        assert!(stored_rate_dates(&conn).is_empty());
    }

    #[test]
    fn fx_fetch_date_defaults_to_today() {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        assert_eq!(resolve_fx_fetch_date(None).unwrap(), today);
    }

    #[test]
    fn fx_fetch_date_accepts_a_padded_date() {
        let date = resolve_fx_fetch_date(Some("2027-01-09".to_owned())).unwrap();

        assert_eq!(date, "2027-01-09");
    }

    #[test]
    fn fx_fetch_date_rejects_anything_but_a_padded_date() {
        for date in ["2027-1-9", "2027-01-09T00:00:00", "tomorrow", ""] {
            let err = resolve_fx_fetch_date(Some(date.to_owned())).unwrap_err();
            assert_eq!(err.code, "VALIDATION", "{date:?}");
        }
    }
}
