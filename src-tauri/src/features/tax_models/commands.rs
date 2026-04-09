use crate::error::AppError;
use crate::features::tax_models::models::{TaxModelDetail, TaxModelRow};
use crate::AppState;
use serde::Deserialize;
use tauri::State;

use super::repository;

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
pub struct CreateTaxModelInput {
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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaxModelInput {
    pub model_id: i64,
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
}

fn validate_tax_model_input(
    name: &str,
    calendar_year: i64,
    vat_status: &str,
    vat_from_date: &Option<String>,
    brackets: &[BracketInputData],
    bps_fields: &[Option<i64>],
) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Tax model name is required.".into(),
        });
    }
    if !(2000..=2100).contains(&calendar_year) {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "Calendar year must be between 2000 and 2100.".into(),
        });
    }
    if !matches!(vat_status, "none" | "all_year" | "from_date") {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "vat_status must be 'none', 'all_year', or 'from_date'.".into(),
        });
    }
    if vat_status == "from_date" {
        match vat_from_date {
            Some(d) if !d.trim().is_empty() => {}
            _ => {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: "vat_from_date is required when vat_status is 'from_date'.".into(),
                });
            }
        }
    }
    for bps in bps_fields.iter().flatten() {
        if !(0..=10000).contains(bps) {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Basis points values must be between 0 and 10000.".into(),
            });
        }
    }
    if brackets.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "At least one income bracket is required.".into(),
        });
    }
    let mut prev_lower: Option<i64> = None;
    for (i, bracket) in brackets.iter().enumerate() {
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
                for (j, tier) in tiers.iter().enumerate() {
                    let obj = tier.as_object().ok_or_else(|| AppError {
                        code: "VALIDATION".into(),
                        message: format!(
                            "Bracket {}, tier {}: each tier must be an object.",
                            i + 1,
                            j + 1
                        ),
                    })?;
                    obj.get("thresholdMinor")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| AppError {
                            code: "VALIDATION".into(),
                            message: format!(
                                "Bracket {}, tier {}: missing or invalid 'thresholdMinor' (i64).",
                                i + 1,
                                j + 1
                            ),
                        })?;
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
    Ok(())
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

#[tauri::command]
pub fn create_tax_model(
    state: State<'_, AppState>,
    input: CreateTaxModelInput,
) -> Result<i64, AppError> {
    validate_tax_model_input(
        &input.name,
        input.calendar_year,
        &input.vat_status,
        &input.vat_from_date,
        &input.brackets,
        &[input.reserve_fund_pct_bps, input.dividend_tax_rate_bps],
    )?;
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
    validate_tax_model_input(
        &input.name,
        input.calendar_year,
        &input.vat_status,
        &input.vat_from_date,
        &input.brackets,
        &[input.reserve_fund_pct_bps, input.dividend_tax_rate_bps],
    )?;
    let conn = state.conn()?;
    repository::update_tax_model(
        &conn,
        repository::UpdateTaxModelParams {
            model_id: input.model_id,
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
        },
    )
}

#[tauri::command]
pub fn delete_tax_model(state: State<'_, AppState>, model_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::delete_tax_model(&conn, model_id)
}
