use std::collections::BTreeMap;

use crate::error::AppError;
use crate::features::tax_models::models::{
    validation, ActualMonths, EarlierProfit, ForecastFigures, ForecastLine, MinimumTaxBand,
    TaxCalculationResult, TaxModelDetail, TaxModelRow, TypedPair,
};
use crate::AppState;
use chrono::NaiveDate;
use serde::Deserialize;
use tauri::State;

use super::{calculation, repository};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BracketInputData {
    pub sort_order: i64,
    pub lower_bound_minor: i64,
    pub rate_type: String,
    pub flat_rate_bps: Option<i64>,
    pub tiers_json: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinimumTaxBandInput {
    pub lower_bound_minor: i64,
    pub amount_minor: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxModelSettingsInput {
    pub name: String,
    pub calendar_year: i64,
    pub person_id: i64,
    pub vat_status: String,
    pub vat_from_date: Option<String>,
    pub reserve_fund_current_minor: Option<i64>,
    pub reserve_fund_pct_bps: Option<i64>,
    pub reserve_fund_max_minor: Option<i64>,
    pub dividend_tax_rate_bps: Option<i64>,
    pub brackets: Vec<BracketInputData>,
    pub minimum_tax_bands: Vec<MinimumTaxBandInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaxModelInput {
    pub model_id: i64,
    #[serde(flatten)]
    pub settings: TaxModelSettingsInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypedPairInput {
    pub so_far_minor: Option<i64>,
    pub rest_minor: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EarlierProfitInput {
    pub profit_year: i64,
    pub amount_minor: i64,
    pub dividend_tax_rate_bps: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastFiguresInput {
    pub actuals_through_month: i64,
    pub payroll_monthly_minor: i64,
    pub typed_values: BTreeMap<String, TypedPairInput>,
    pub earlier_profits: Vec<EarlierProfitInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTaxForecastFiguresInput {
    pub model_id: i64,
    pub figures: ForecastFiguresInput,
}

const MAX_AMOUNT_MINOR: i64 = 10_000_000_000_000;

fn validate_amount(amount_minor: i64, label: impl std::fmt::Display) -> Result<(), AppError> {
    if amount_minor < 0 {
        Err(validation(format!("{label} can't be negative.")))
    } else if amount_minor > MAX_AMOUNT_MINOR {
        Err(validation(format!("{label} is too large.")))
    } else {
        Ok(())
    }
}

fn validate_minimum_tax_bands(bands: &[MinimumTaxBandInput]) -> Result<(), AppError> {
    let mut prev_lower: Option<i64> = None;
    for (i, band) in bands.iter().enumerate() {
        validate_amount(
            band.lower_bound_minor,
            format_args!("Minimum tax band {}: the bound", i + 1),
        )?;
        validate_amount(
            band.amount_minor,
            format_args!("Minimum tax band {}: the amount", i + 1),
        )?;
        if prev_lower.is_some_and(|prev| band.lower_bound_minor <= prev) {
            return Err(validation(format!(
                "Minimum tax band {}: the bound must be above the previous band's.",
                i + 1
            )));
        }
        prev_lower = Some(band.lower_bound_minor);
    }
    Ok(())
}

fn validate_earlier_profit_years(
    earlier_profits: &[EarlierProfit],
    calendar_year: i64,
) -> Result<(), AppError> {
    match earlier_profits
        .iter()
        .find(|row| row.profit_year >= calendar_year)
    {
        Some(row) => Err(validation(format!(
            "The earlier profit of {} must be from a year before {calendar_year}.",
            row.profit_year
        ))),
        None => Ok(()),
    }
}

fn parse_forecast_figures(
    input: ForecastFiguresInput,
    calendar_year: i64,
) -> Result<ForecastFigures, AppError> {
    let actuals_through_month = ActualMonths::new(input.actuals_through_month)?;
    validate_amount(input.payroll_monthly_minor, "The monthly payroll")?;

    let mut typed_values = BTreeMap::new();
    for (key, pair) in input.typed_values {
        let line = ForecastLine::parse(&key)?;
        for amount in [pair.so_far_minor, pair.rest_minor].into_iter().flatten() {
            validate_amount(amount, format_args!("The typed {key} amount"))?;
        }
        if pair.so_far_minor.is_some() && actuals_through_month.get() == 0 {
            return Err(validation(format!(
                "There are no actual months to type {key} for."
            )));
        }
        if pair.rest_minor.is_some() && actuals_through_month.months_left() == 0 {
            return Err(validation(format!(
                "There are no months left to type {key} for."
            )));
        }
        if pair.so_far_minor.is_some() || pair.rest_minor.is_some() {
            typed_values.insert(
                line,
                TypedPair {
                    so_far_minor: pair.so_far_minor,
                    rest_minor: pair.rest_minor,
                },
            );
        }
    }

    let mut earlier_profits: Vec<EarlierProfit> = input
        .earlier_profits
        .into_iter()
        .map(|row| EarlierProfit {
            profit_year: row.profit_year,
            amount_minor: row.amount_minor,
            dividend_tax_rate_bps: row.dividend_tax_rate_bps,
        })
        .collect();
    earlier_profits.sort_by_key(|row| row.profit_year);
    validate_earlier_profit_years(&earlier_profits, calendar_year)?;
    for row in &earlier_profits {
        validate_amount(
            row.amount_minor,
            format_args!("The earlier profit of {}", row.profit_year),
        )?;
        if !(0..=10000).contains(&row.dividend_tax_rate_bps) {
            return Err(validation(format!(
                "The dividend tax rate for {} must be between 0 and 100%.",
                row.profit_year
            )));
        }
    }
    if let Some(pair) = earlier_profits
        .windows(2)
        .find(|pair| pair[0].profit_year == pair[1].profit_year)
    {
        return Err(validation(format!(
            "The earlier profit of {} is listed twice.",
            pair[0].profit_year
        )));
    }

    Ok(ForecastFigures {
        actuals_through_month,
        payroll_monthly_minor: input.payroll_monthly_minor,
        typed_values,
        earlier_profits,
    })
}

fn validate_tax_model_input(input: &TaxModelSettingsInput) -> Result<(), AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Tax model name is required.".into(),
        });
    }
    if !(2000..=2100).contains(&input.calendar_year) {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Calendar year must be between 2000 and 2100.".into(),
        });
    }
    if !matches!(input.vat_status.as_str(), "none" | "all_year" | "from_date") {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "vat_status must be 'none', 'all_year', or 'from_date'.".into(),
        });
    }
    if input.vat_status == "from_date" {
        match &input.vat_from_date {
            Some(d) if !d.trim().is_empty() => {}
            _ => {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: "vat_from_date is required when vat_status is 'from_date'.".into(),
                });
            }
        }
    }
    if let Some(d) = &input.vat_from_date {
        if NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "vat_from_date must be a valid date in YYYY-MM-DD format.".into(),
            });
        }
    }
    for bps in [input.reserve_fund_pct_bps, input.dividend_tax_rate_bps]
        .iter()
        .flatten()
    {
        if !(0..=10000).contains(bps) {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Basis points values must be between 0 and 10000.".into(),
            });
        }
    }
    if input.reserve_fund_pct_bps.is_some_and(|pct| pct > 0)
        && input.reserve_fund_max_minor.is_none()
    {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Reserve fund maximum is required when a reserve fund percentage is set."
                .into(),
        });
    }
    if input.brackets.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "At least one income bracket is required.".into(),
        });
    }
    let mut prev_lower: Option<i64> = None;
    for (i, bracket) in input.brackets.iter().enumerate() {
        if bracket.lower_bound_minor < 0 {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: format!("Bracket {}: lower_bound_minor must be >= 0.", i + 1),
            });
        }
        if let Some(prev) = prev_lower {
            if bracket.lower_bound_minor <= prev {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: format!(
                        "Bracket {}: lower_bound_minor must be strictly greater than the previous bracket's.",
                        i + 1
                    ),
                });
            }
        }
        prev_lower = Some(bracket.lower_bound_minor);

        match bracket.rate_type.as_str() {
            "flat" => match bracket.flat_rate_bps {
                Some(bps) if (0..=10000).contains(&bps) => {}
                Some(_) => {
                    return Err(AppError {
                        code: "VALIDATION".into(),
                        message: format!(
                            "Bracket {}: flat_rate_bps must be between 0 and 10000.",
                            i + 1
                        ),
                    });
                }
                None => {
                    return Err(AppError {
                        code: "VALIDATION".into(),
                        message: format!(
                            "Bracket {}: flat_rate_bps is required for flat rate brackets.",
                            i + 1
                        ),
                    });
                }
            },
            "progressive" => {
                let tiers_str = bracket
                    .tiers_json
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| AppError {
                        code: "VALIDATION".into(),
                        message: format!(
                            "Bracket {}: tiers_json is required for progressive brackets.",
                            i + 1
                        ),
                    })?;
                let tiers: Vec<serde_json::Value> =
                    serde_json::from_str(tiers_str).map_err(|_| AppError {
                        code: "VALIDATION".into(),
                        message: format!(
                            "Bracket {}: tiers_json must be a valid JSON array.",
                            i + 1
                        ),
                    })?;
                if tiers.is_empty() {
                    return Err(AppError {
                        code: "VALIDATION".into(),
                        message: format!(
                            "Bracket {}: progressive brackets must have at least one tier.",
                            i + 1
                        ),
                    });
                }
                let mut prev_threshold: Option<i64> = None;
                for (j, tier) in tiers.iter().enumerate() {
                    let obj = tier.as_object().ok_or_else(|| AppError {
                        code: "VALIDATION".into(),
                        message: format!(
                            "Bracket {}, tier {}: each tier must be an object.",
                            i + 1,
                            j + 1
                        ),
                    })?;
                    let threshold_minor = obj
                        .get("thresholdMinor")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| AppError {
                            code: "VALIDATION".into(),
                            message: format!(
                                "Bracket {}, tier {}: missing or invalid 'thresholdMinor' (i64).",
                                i + 1,
                                j + 1
                            ),
                        })?;
                    // The last tier has no upper cap, so its threshold is never used.
                    if j + 1 < tiers.len() {
                        if threshold_minor < 0 {
                            return Err(AppError {
                                code: "VALIDATION".into(),
                                message: format!(
                                    "Bracket {}, tier {}: thresholdMinor must be >= 0.",
                                    i + 1,
                                    j + 1
                                ),
                            });
                        }
                        if prev_threshold.is_some_and(|prev| threshold_minor <= prev) {
                            return Err(AppError {
                                code: "VALIDATION".into(),
                                message: format!(
                                    "Bracket {}, tier {}: thresholdMinor must be strictly greater than the previous tier's.",
                                    i + 1,
                                    j + 1
                                ),
                            });
                        }
                        prev_threshold = Some(threshold_minor);
                    }
                    let rate_bps =
                        obj.get("rateBps")
                            .and_then(|v| v.as_i64())
                            .ok_or_else(|| AppError {
                                code: "VALIDATION".into(),
                                message: format!(
                                    "Bracket {}, tier {}: missing or invalid 'rateBps' (i64).",
                                    i + 1,
                                    j + 1
                                ),
                            })?;
                    if !(0..=10000).contains(&rate_bps) {
                        return Err(AppError {
                            code: "VALIDATION".into(),
                            message: format!(
                                "Bracket {}, tier {}: rateBps must be between 0 and 10000.",
                                i + 1,
                                j + 1
                            ),
                        });
                    }
                }
            }
            _ => {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: format!(
                        "Bracket {}: rate_type must be 'flat' or 'progressive'.",
                        i + 1
                    ),
                });
            }
        }
    }
    validate_minimum_tax_bands(&input.minimum_tax_bands)
}

fn map_brackets(brackets: Vec<BracketInputData>) -> Vec<repository::BracketInput> {
    brackets
        .into_iter()
        .map(|b| repository::BracketInput {
            sort_order: b.sort_order,
            lower_bound_minor: b.lower_bound_minor,
            rate_type: b.rate_type,
            flat_rate_bps: b.flat_rate_bps,
            tiers_json: b.tiers_json,
        })
        .collect()
}

fn map_bands(bands: Vec<MinimumTaxBandInput>) -> Vec<MinimumTaxBand> {
    bands
        .into_iter()
        .map(|b| MinimumTaxBand {
            lower_bound_minor: b.lower_bound_minor,
            amount_minor: b.amount_minor,
        })
        .collect()
}

#[tauri::command]
pub fn create_tax_model(
    state: State<'_, AppState>,
    input: TaxModelSettingsInput,
) -> Result<i64, AppError> {
    validate_tax_model_input(&input)?;
    let actuals_through_month =
        ActualMonths::default_for(input.calendar_year, chrono::Local::now().date_naive());
    let conn = state.conn()?;
    repository::create_tax_model(
        &conn,
        repository::CreateTaxModelParams {
            name: input.name.trim().to_owned(),
            calendar_year: input.calendar_year,
            person_id: input.person_id,
            vat_status: input.vat_status,
            vat_from_date: input.vat_from_date,
            reserve_fund_current_minor: input.reserve_fund_current_minor,
            reserve_fund_pct_bps: input.reserve_fund_pct_bps,
            reserve_fund_max_minor: input.reserve_fund_max_minor,
            dividend_tax_rate_bps: input.dividend_tax_rate_bps,
            brackets: map_brackets(input.brackets),
            minimum_tax_bands: map_bands(input.minimum_tax_bands),
            actuals_through_month,
        },
    )
}

#[tauri::command]
pub fn list_tax_models(state: State<'_, AppState>) -> Result<Vec<TaxModelRow>, AppError> {
    let conn = state.conn()?;
    repository::list_tax_models(&conn).map_err(AppError::from)
}

#[tauri::command]
pub fn get_tax_model(
    state: State<'_, AppState>,
    model_id: i64,
) -> Result<TaxModelDetail, AppError> {
    let conn = state.conn()?;
    repository::get_tax_model(&conn, model_id)
}

#[tauri::command]
pub fn update_tax_model(
    state: State<'_, AppState>,
    input: UpdateTaxModelInput,
) -> Result<(), AppError> {
    let UpdateTaxModelInput { model_id, settings } = input;
    validate_tax_model_input(&settings)?;
    let conn = state.conn()?;
    let stored = repository::get_tax_model(&conn, model_id)?;
    validate_earlier_profit_years(&stored.figures.earlier_profits, settings.calendar_year)?;
    repository::update_tax_model(
        &conn,
        repository::UpdateTaxModelParams {
            model_id,
            name: settings.name.trim().to_owned(),
            calendar_year: settings.calendar_year,
            person_id: settings.person_id,
            vat_status: settings.vat_status,
            vat_from_date: settings.vat_from_date,
            reserve_fund_current_minor: settings.reserve_fund_current_minor,
            reserve_fund_pct_bps: settings.reserve_fund_pct_bps,
            reserve_fund_max_minor: settings.reserve_fund_max_minor,
            dividend_tax_rate_bps: settings.dividend_tax_rate_bps,
            brackets: map_brackets(settings.brackets),
            minimum_tax_bands: map_bands(settings.minimum_tax_bands),
        },
    )
}

#[tauri::command]
pub fn save_tax_forecast_figures(
    state: State<'_, AppState>,
    input: SaveTaxForecastFiguresInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    let model = repository::get_tax_model(&conn, input.model_id)?;
    let figures = parse_forecast_figures(input.figures, model.calendar_year)?;
    repository::save_tax_forecast_figures(
        &conn,
        repository::SaveTaxForecastFiguresParams {
            model_id: input.model_id,
            figures,
        },
    )
}

#[tauri::command]
pub fn delete_tax_model(state: State<'_, AppState>, model_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::delete_tax_model(&conn, model_id)
}

#[tauri::command]
pub fn calculate_tax_model(
    state: State<'_, AppState>,
    model_id: i64,
) -> Result<TaxCalculationResult, AppError> {
    let conn = state.conn()?;
    calculate_tax_model_inner(&conn, model_id)
}

pub(crate) fn calculate_tax_model_inner(
    conn: &rusqlite::Connection,
    model_id: i64,
) -> Result<TaxCalculationResult, AppError> {
    let model = repository::get_tax_model(conn, model_id)?;
    let events = repository::list_taxable_events_for_model(
        conn,
        repository::ListTaxableEventsForModelParams {
            person_id: model.person_id,
            calendar_year: model.calendar_year as i32,
        },
    )?;
    Ok(calculation::calculate_tax_model_results(&model, &events))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(lower_bound_minor: i64) -> BracketInputData {
        BracketInputData {
            sort_order: 0,
            lower_bound_minor,
            rate_type: "flat".into(),
            flat_rate_bps: Some(1900),
            tiers_json: None,
        }
    }

    fn settings() -> TaxModelSettingsInput {
        TaxModelSettingsInput {
            name: "Model".into(),
            calendar_year: 2026,
            person_id: 1,
            vat_status: "none".into(),
            vat_from_date: None,
            reserve_fund_current_minor: None,
            reserve_fund_pct_bps: None,
            reserve_fund_max_minor: None,
            dividend_tax_rate_bps: None,
            brackets: vec![flat(0)],
            minimum_tax_bands: vec![],
        }
    }

    fn validate_reserve(
        reserve_fund_pct_bps: Option<i64>,
        reserve_fund_max_minor: Option<i64>,
    ) -> Result<(), AppError> {
        validate_tax_model_input(&TaxModelSettingsInput {
            reserve_fund_pct_bps,
            reserve_fund_max_minor,
            ..settings()
        })
    }

    #[test]
    fn reserve_fund_pct_without_max_is_rejected() {
        let err = validate_reserve(Some(1000), None).unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("maximum"));
    }

    #[test]
    fn reserve_fund_pct_with_max_is_valid() {
        assert!(validate_reserve(Some(1000), Some(50_000)).is_ok());
    }

    fn validate_tiers(tiers_json: &str) -> Result<(), AppError> {
        let progressive = BracketInputData {
            sort_order: 0,
            lower_bound_minor: 0,
            rate_type: "progressive".into(),
            flat_rate_bps: None,
            tiers_json: Some(tiers_json.into()),
        };
        validate_tax_model_input(&TaxModelSettingsInput {
            brackets: vec![progressive],
            ..settings()
        })
    }

    fn validate_vat_from_date(vat_from_date: &str) -> Result<(), AppError> {
        validate_tax_model_input(&TaxModelSettingsInput {
            vat_status: "from_date".into(),
            vat_from_date: Some(vat_from_date.into()),
            ..settings()
        })
    }

    #[test]
    fn increasing_tier_thresholds_are_valid_and_last_threshold_is_ignored() {
        let tiers = r#"[{"thresholdMinor":1000,"rateBps":1000},
                        {"thresholdMinor":5000,"rateBps":2000},
                        {"thresholdMinor":0,"rateBps":3000}]"#;
        assert!(validate_tiers(tiers).is_ok());
    }

    #[test]
    fn descending_tier_thresholds_are_rejected() {
        let tiers = r#"[{"thresholdMinor":5000,"rateBps":1000},
                        {"thresholdMinor":1000,"rateBps":2000},
                        {"thresholdMinor":0,"rateBps":3000}]"#;
        let err = validate_tiers(tiers).unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("tier 2"));
    }

    #[test]
    fn equal_tier_thresholds_are_rejected() {
        let tiers = r#"[{"thresholdMinor":5000,"rateBps":1000},
                        {"thresholdMinor":5000,"rateBps":2000},
                        {"thresholdMinor":0,"rateBps":3000}]"#;
        assert_eq!(validate_tiers(tiers).unwrap_err().code, "VALIDATION");
    }

    #[test]
    fn negative_tier_threshold_is_rejected() {
        let tiers = r#"[{"thresholdMinor":-1,"rateBps":1000},
                        {"thresholdMinor":0,"rateBps":3000}]"#;
        let err = validate_tiers(tiers).unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("tier 1"));
    }

    #[test]
    fn vat_from_date_must_be_a_plain_date() {
        assert!(validate_vat_from_date("2026-06-01").is_ok());
        for bad in ["2026-13-01", "2026-06-01T00:00:00", "01.06.2026", "garbage"] {
            let err = validate_vat_from_date(bad).unwrap_err();
            assert_eq!(err.code, "VALIDATION", "{bad} should be rejected");
        }
    }

    #[test]
    fn reserve_fund_zero_or_empty_pct_without_max_is_valid() {
        assert!(validate_reserve(Some(0), None).is_ok());
        assert!(validate_reserve(None, None).is_ok());
    }

    fn validate_bands(bands: &[(i64, i64)]) -> Result<(), AppError> {
        validate_tax_model_input(&TaxModelSettingsInput {
            minimum_tax_bands: bands
                .iter()
                .map(|&(lower_bound_minor, amount_minor)| MinimumTaxBandInput {
                    lower_bound_minor,
                    amount_minor,
                })
                .collect(),
            ..settings()
        })
    }

    #[test]
    fn increasing_minimum_tax_bands_are_valid() {
        assert!(validate_bands(&[]).is_ok());
        assert!(
            validate_bands(&[(0, 340_00), (50_000_00, 960_00), (250_000_00, 1_920_00)]).is_ok()
        );
    }

    #[test]
    fn minimum_tax_bands_that_do_not_increase_are_rejected() {
        for bands in [
            vec![(0, 340_00), (0, 960_00)],
            vec![(50_000_00, 340_00), (0, 960_00)],
            vec![(-1, 340_00)],
            vec![(0, -1)],
        ] {
            assert_eq!(
                validate_bands(&bands).unwrap_err().code,
                "VALIDATION",
                "{bands:?} should be rejected"
            );
        }
    }

    #[test]
    fn minimum_tax_band_amounts_stop_at_the_amount_limit() {
        assert!(validate_bands(&[(0, 10_000_000_000_000), (10_000_000_000_000, 0)]).is_ok());
        for bands in [vec![(0, 10_000_000_000_001)], vec![(10_000_000_000_001, 0)]] {
            assert_eq!(
                validate_bands(&bands).unwrap_err().code,
                "VALIDATION",
                "{bands:?} should be rejected"
            );
        }
    }

    #[test]
    fn update_settings_follow_the_minimum_tax_bands_through_serde_flatten() {
        let input: UpdateTaxModelInput = serde_json::from_value(serde_json::json!({
            "modelId": 7,
            "name": "Company 2026",
            "calendarYear": 2026,
            "personId": 3,
            "vatStatus": "from_date",
            "vatFromDate": "2026-06-01",
            "reserveFundCurrentMinor": 50_000,
            "reserveFundPctBps": 1000,
            "reserveFundMaxMinor": 9_007_199_254_740_991_i64,
            "dividendTaxRateBps": 700,
            "brackets": [{ "sortOrder": 0, "lowerBoundMinor": 0, "rateType": "flat", "flatRateBps": 1000, "tiersJson": null }],
            "minimumTaxBands": [{ "lowerBoundMinor": 0, "amountMinor": 34_000 }],
        }))
        .expect("update input should deserialize");

        assert_eq!(input.model_id, 7);
        let settings = input.settings;
        assert_eq!(settings.name, "Company 2026");
        assert_eq!(settings.calendar_year, 2026);
        assert_eq!(settings.person_id, 3);
        assert_eq!(settings.vat_from_date.as_deref(), Some("2026-06-01"));
        assert_eq!(settings.reserve_fund_current_minor, Some(50_000));
        assert_eq!(settings.reserve_fund_max_minor, Some(9_007_199_254_740_991));
        assert_eq!(settings.dividend_tax_rate_bps, Some(700));
        assert_eq!(settings.brackets[0].flat_rate_bps, Some(1000));
        assert_eq!(settings.minimum_tax_bands[0].amount_minor, 34_000);
        assert!(validate_tax_model_input(&settings).is_ok());
    }

    #[test]
    fn update_input_without_minimum_tax_bands_is_rejected() {
        let result = serde_json::from_value::<UpdateTaxModelInput>(serde_json::json!({
            "modelId": 7,
            "name": "Company 2026",
            "calendarYear": 2026,
            "personId": 3,
            "vatStatus": "none",
            "brackets": [],
        }));
        assert!(result.is_err());
    }

    #[test]
    fn a_calendar_year_must_follow_every_earlier_profit_year() {
        let earlier = [EarlierProfit {
            profit_year: 2025,
            amount_minor: 1_000_00,
            dividend_tax_rate_bps: 700,
        }];
        assert!(validate_earlier_profit_years(&earlier, 2026).is_ok());
        for year in [2025, 2024] {
            assert_eq!(
                validate_earlier_profit_years(&earlier, year)
                    .unwrap_err()
                    .code,
                "VALIDATION"
            );
        }
    }

    fn figures_input(json: serde_json::Value) -> ForecastFiguresInput {
        let mut input = serde_json::json!({
            "actualsThroughMonth": 9,
            "payrollMonthlyMinor": 4_000_00,
            "typedValues": {},
            "earlierProfits": [],
        });
        input
            .as_object_mut()
            .unwrap()
            .extend(json.as_object().unwrap().clone());
        serde_json::from_value(input).expect("figures input should deserialize")
    }

    fn parse(json: serde_json::Value) -> Result<ForecastFigures, AppError> {
        parse_forecast_figures(figures_input(json), 2026)
    }

    fn earlier_input(year: i64, amount_minor: i64, rate_bps: i64) -> serde_json::Value {
        serde_json::json!({ "profitYear": year, "amountMinor": amount_minor, "dividendTaxRateBps": rate_bps })
    }

    #[test]
    fn figures_parse_into_the_model_shape() {
        let figures = parse(serde_json::json!({
            "typedValues": {
                "revenue": { "soFarMinor": 90_000_00, "restMinor": null },
                "payroll": { "soFarMinor": null, "restMinor": null },
                "deductible_costs": { "soFarMinor": 0, "restMinor": 5_600_00 },
            },
            "earlierProfits": [earlier_input(2025, 15_000_00, 700), earlier_input(2023, 2_000_00, 700)],
        }))
        .expect("figures should parse");

        assert_eq!(figures.actuals_through_month.get(), 9);
        assert_eq!(figures.payroll_monthly_minor, 4_000_00);
        assert_eq!(
            figures.typed_values,
            BTreeMap::from([
                (
                    ForecastLine::Revenue,
                    TypedPair {
                        so_far_minor: Some(90_000_00),
                        rest_minor: None,
                    },
                ),
                (
                    ForecastLine::DeductibleCosts,
                    TypedPair {
                        so_far_minor: Some(0),
                        rest_minor: Some(5_600_00),
                    },
                ),
            ])
        );
        let years: Vec<i64> = figures
            .earlier_profits
            .iter()
            .map(|row| row.profit_year)
            .collect();
        assert_eq!(years, vec![2023, 2025]);
    }

    #[test]
    fn invalid_figures_are_rejected() {
        let cases = [
            ("month 13", serde_json::json!({ "actualsThroughMonth": 13 })),
            ("month -1", serde_json::json!({ "actualsThroughMonth": -1 })),
            (
                "negative payroll",
                serde_json::json!({ "payrollMonthlyMinor": -1 }),
            ),
            (
                "negative typed amount",
                serde_json::json!({ "typedValues": { "revenue": { "soFarMinor": -1, "restMinor": null } } }),
            ),
            (
                "unknown line",
                serde_json::json!({ "typedValues": { "bonus": { "soFarMinor": 1, "restMinor": null } } }),
            ),
            (
                "typed so far with no actual months",
                serde_json::json!({ "actualsThroughMonth": 0, "typedValues": { "revenue": { "soFarMinor": 1, "restMinor": null } } }),
            ),
            (
                "typed rest with no months left",
                serde_json::json!({ "actualsThroughMonth": 12, "typedValues": { "revenue": { "soFarMinor": null, "restMinor": 1 } } }),
            ),
            (
                "profit year equal to the model's",
                serde_json::json!({ "earlierProfits": [earlier_input(2026, 1, 700)] }),
            ),
            (
                "profit year after the model's",
                serde_json::json!({ "earlierProfits": [earlier_input(2027, 1, 700)] }),
            ),
            (
                "negative earlier profit",
                serde_json::json!({ "earlierProfits": [earlier_input(2025, -1, 700)] }),
            ),
            (
                "rate of 10001",
                serde_json::json!({ "earlierProfits": [earlier_input(2025, 1, 10001)] }),
            ),
            (
                "negative rate",
                serde_json::json!({ "earlierProfits": [earlier_input(2025, 1, -1)] }),
            ),
            (
                "profit year twice",
                serde_json::json!({ "earlierProfits": [earlier_input(2025, 1, 700), earlier_input(2025, 2, 700)] }),
            ),
        ];
        for (case, json) in cases {
            assert_eq!(parse(json).unwrap_err().code, "VALIDATION", "{case}");
        }
    }

    fn every_figure_amount(amount_minor: i64) -> [(&'static str, serde_json::Value); 4] {
        [
            (
                "payroll",
                serde_json::json!({ "payrollMonthlyMinor": amount_minor }),
            ),
            (
                "typed so far",
                serde_json::json!({ "typedValues": { "revenue": { "soFarMinor": amount_minor, "restMinor": null } } }),
            ),
            (
                "typed rest",
                serde_json::json!({ "typedValues": { "revenue": { "soFarMinor": null, "restMinor": amount_minor } } }),
            ),
            (
                "earlier profit",
                serde_json::json!({ "earlierProfits": [earlier_input(2025, amount_minor, 700)] }),
            ),
        ]
    }

    #[test]
    fn figure_amounts_stop_at_the_amount_limit() {
        for (case, json) in every_figure_amount(10_000_000_000_000) {
            assert!(parse(json).is_ok(), "{case} at the limit should parse");
        }
        for (case, json) in every_figure_amount(10_000_000_000_001) {
            assert_eq!(
                parse(json).unwrap_err().code,
                "VALIDATION",
                "{case} above the limit"
            );
        }
    }

    #[test]
    fn edge_months_accept_the_cells_that_cover_months() {
        let at_zero = parse(serde_json::json!({
            "actualsThroughMonth": 0,
            "typedValues": { "revenue": { "soFarMinor": null, "restMinor": 1 } },
        }));
        assert!(at_zero.is_ok());
        let at_twelve = parse(serde_json::json!({
            "actualsThroughMonth": 12,
            "typedValues": { "revenue": { "soFarMinor": 1, "restMinor": null } },
        }));
        assert!(at_twelve.is_ok());
    }
}
