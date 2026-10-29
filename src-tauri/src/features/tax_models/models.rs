use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::shared::div_round;

pub(super) fn validation(message: impl Into<String>) -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message: message.into(),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaxTier {
    pub threshold_minor: i64,
    pub rate_bps: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ForecastLine {
    Revenue,
    Payroll,
    Depreciation,
    DeductibleCosts,
    NonDeductibleCosts,
}

impl ForecastLine {
    pub const ALL: [ForecastLine; 5] = [
        ForecastLine::Revenue,
        ForecastLine::Payroll,
        ForecastLine::Depreciation,
        ForecastLine::DeductibleCosts,
        ForecastLine::NonDeductibleCosts,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ForecastLine::Revenue => "revenue",
            ForecastLine::Payroll => "payroll",
            ForecastLine::Depreciation => "depreciation",
            ForecastLine::DeductibleCosts => "deductible_costs",
            ForecastLine::NonDeductibleCosts => "non_deductible_costs",
        }
    }

    pub fn parse(s: &str) -> Result<Self, AppError> {
        Self::ALL
            .into_iter()
            .find(|line| line.as_str() == s)
            .ok_or_else(|| validation(format!("Unknown forecast line '{s}'.")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueSource {
    Typed,
    Entries,
    Schedule,
    Average,
    MonthlyAmount,
    Nothing,
}

impl ValueSource {
    #[cfg(test)]
    pub const ALL: [ValueSource; 6] = [
        ValueSource::Typed,
        ValueSource::Entries,
        ValueSource::Schedule,
        ValueSource::Average,
        ValueSource::MonthlyAmount,
        ValueSource::Nothing,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Revenue,
    Depreciation,
    Prepaid,
    Expense,
}

impl EntryKind {
    #[cfg(test)]
    pub const ALL: [EntryKind; 4] = [
        EntryKind::Revenue,
        EntryKind::Depreciation,
        EntryKind::Prepaid,
        EntryKind::Expense,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryNotCountedReason {
    TypedValue,
    AfterActualMonths,
}

impl EntryNotCountedReason {
    #[cfg(test)]
    pub const ALL: [EntryNotCountedReason; 2] = [
        EntryNotCountedReason::TypedValue,
        EntryNotCountedReason::AfterActualMonths,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ActualMonths(u8);

impl ActualMonths {
    pub fn new(month: i64) -> Result<Self, AppError> {
        u8::try_from(month)
            .ok()
            .filter(|month| *month <= 12)
            .map(ActualMonths)
            .ok_or_else(|| validation("The actual months must be between 0 and 12."))
    }

    pub fn get(self) -> u8 {
        self.0
    }

    pub fn covers(self, month: u8) -> bool {
        month <= self.0
    }

    pub fn months_left(self) -> i64 {
        12 - i64::from(self.0)
    }

    pub fn average_rest(self, so_far_minor: i64) -> Option<i64> {
        (1..=11)
            .contains(&self.0)
            .then(|| div_round(so_far_minor * self.months_left(), i64::from(self.0)))
    }

    pub fn default_for(calendar_year: i64, today: NaiveDate) -> Self {
        match calendar_year.cmp(&i64::from(today.year())) {
            std::cmp::Ordering::Less => ActualMonths(12),
            std::cmp::Ordering::Greater => ActualMonths(0),
            std::cmp::Ordering::Equal => ActualMonths(today.month0() as u8),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypedPair {
    pub so_far_minor: Option<i64>,
    pub rest_minor: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EarlierProfit {
    pub profit_year: i64,
    pub amount_minor: i64,
    pub dividend_tax_rate_bps: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinimumTaxBand {
    pub lower_bound_minor: i64,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastFigures {
    pub actuals_through_month: ActualMonths,
    pub payroll_monthly_minor: i64,
    pub typed_values: BTreeMap<ForecastLine, TypedPair>,
    pub earlier_profits: Vec<EarlierProfit>,
}

impl Default for ForecastFigures {
    fn default() -> Self {
        ForecastFigures {
            actuals_through_month: ActualMonths(12),
            payroll_monthly_minor: 0,
            typed_values: BTreeMap::new(),
            earlier_profits: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventDataForCalc {
    pub event_id: i64,
    pub kind: EntryKind,
    pub month: u8,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub reclaimed_vat: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaxEventBreakdown {
    pub event_id: i64,
    pub kind: EntryKind,
    pub event_date: String,
    pub note: Option<String>,
    pub amount_minor: i64,
    pub net_amount_minor: i64,
    pub vat_amount_minor: i64,
    pub reclaimable_vat_minor: i64,
    pub non_reclaimable_vat_minor: i64,
    pub tax_deductible_cost_minor: i64,
    pub non_tax_deductible_cost_minor: i64,
    pub not_counted: Option<EntryNotCountedReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastCell {
    pub amount_minor: i64,
    pub source: ValueSource,
    pub untyped_minor: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastLineResult {
    pub line: ForecastLine,
    pub so_far: ForecastCell,
    pub rest: ForecastCell,
    pub year_minor: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EarlierProfitResult {
    pub profit_year: i64,
    pub amount_minor: i64,
    pub payable_minor: i64,
    pub dividend_tax_rate_bps: i64,
    pub dividend_tax_minor: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaxCalculationResult {
    pub total_income_minor: i64,
    pub total_tax_deductible_expenses_minor: i64,
    pub total_non_tax_deductible_expenses_minor: i64,
    pub tax_basis_minor: i64,
    pub tax_amount_minor: i64,
    pub total_profit_minor: i64,
    pub reserve_fund_generation_minor: i64,
    pub dividend_minor: i64,
    pub withholding_tax_minor: i64,
    pub net_dividend_minor: i64,
    pub monthly_tax_burden_minor: i64,
    pub person_type: String,
    pub event_breakdowns: Vec<TaxEventBreakdown>,
    pub actuals_through_month: u8,
    pub lines: Vec<ForecastLineResult>,
    pub applied_flat_rate_bps: Option<i64>,
    pub computed_tax_minor: i64,
    pub minimum_tax_minor: Option<i64>,
    pub dividend_from_year_minor: i64,
    pub dividend_from_year_tax_minor: i64,
    pub earlier_profits: Vec<EarlierProfitResult>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaxModelRow {
    pub id: i64,
    pub name: String,
    pub calendar_year: i64,
    pub person_id: i64,
    pub person_name: String,
    pub person_type: String,
    pub vat_status: String,
    pub vat_from_date: Option<String>,
    pub reserve_fund_current_minor: Option<i64>,
    pub reserve_fund_pct_bps: Option<i64>,
    pub reserve_fund_max_minor: Option<i64>,
    pub dividend_tax_rate_bps: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaxModelBracketRow {
    pub id: i64,
    pub sort_order: i64,
    pub lower_bound_minor: i64,
    pub rate_type: String,
    pub flat_rate_bps: Option<i64>,
    pub tiers_json: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaxModelDetail {
    pub id: i64,
    pub name: String,
    pub calendar_year: i64,
    pub person_id: i64,
    pub person_name: String,
    pub person_type: String,
    pub vat_status: String,
    pub vat_from_date: Option<String>,
    pub reserve_fund_current_minor: Option<i64>,
    pub reserve_fund_pct_bps: Option<i64>,
    pub reserve_fund_max_minor: Option<i64>,
    pub dividend_tax_rate_bps: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub brackets: Vec<TaxModelBracketRow>,
    pub minimum_tax_bands: Vec<MinimumTaxBand>,
    pub figures: ForecastFigures,
}

impl TaxModelDetail {
    pub fn is_company(&self) -> bool {
        self.person_type == "legal"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAX_FORECAST_TS: &str = include_str!("../../../../src/shared/types/taxForecast.ts");

    fn ts_string_list(name: &str) -> Vec<String> {
        let declaration = format!("export const {name} = [");
        let start = TAX_FORECAST_TS
            .find(&declaration)
            .unwrap_or_else(|| panic!("{name} is missing from taxForecast.ts"))
            + declaration.len();
        let end = start + TAX_FORECAST_TS[start..].find(']').expect("unclosed list");
        TAX_FORECAST_TS[start..end]
            .split(',')
            .map(|item| item.trim().trim_matches(|c| c == '\'' || c == '"'))
            .filter(|item| !item.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn serde_names<T: Serialize>(items: &[T]) -> Vec<String> {
        items
            .iter()
            .map(|item| match serde_json::to_value(item).unwrap() {
                serde_json::Value::String(name) => name,
                other => panic!("expected a string, got {other}"),
            })
            .collect()
    }

    #[test]
    fn forecast_vocabularies_match_typescript() {
        let line_names = serde_names(&ForecastLine::ALL);
        assert_eq!(ts_string_list("FORECAST_LINES"), line_names);
        assert_eq!(
            ts_string_list("VALUE_SOURCES"),
            serde_names(&ValueSource::ALL)
        );
        assert_eq!(ts_string_list("ENTRY_KINDS"), serde_names(&EntryKind::ALL));
        assert_eq!(
            ts_string_list("ENTRY_NOT_COUNTED_REASONS"),
            serde_names(&EntryNotCountedReason::ALL)
        );
        for (line, name) in ForecastLine::ALL.into_iter().zip(&line_names) {
            assert_eq!(line.as_str(), name);
            assert_eq!(ForecastLine::parse(name).unwrap(), line);
        }
    }

    #[test]
    fn unknown_forecast_line_is_rejected() {
        assert_eq!(ForecastLine::parse("bonus").unwrap_err().code, "VALIDATION");
    }

    #[test]
    fn actual_months_accept_zero_to_twelve_only() {
        assert_eq!(ActualMonths::new(0).unwrap().get(), 0);
        assert_eq!(ActualMonths::new(12).unwrap().get(), 12);
        for bad in [-1, 13, 256] {
            assert_eq!(ActualMonths::new(bad).unwrap_err().code, "VALIDATION");
        }
    }

    #[test]
    fn default_actual_months_follow_today() {
        let date = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        assert_eq!(ActualMonths::default_for(2026, date(2026, 10, 29)).get(), 9);
        assert_eq!(ActualMonths::default_for(2026, date(2026, 1, 15)).get(), 0);
        assert_eq!(
            ActualMonths::default_for(2026, date(2026, 12, 31)).get(),
            11
        );
        assert_eq!(
            ActualMonths::default_for(2025, date(2026, 10, 29)).get(),
            12
        );
        assert_eq!(ActualMonths::default_for(2027, date(2026, 10, 29)).get(), 0);
    }

    #[test]
    fn figures_serialize_typed_values_keyed_by_line() {
        let figures = ForecastFigures {
            actuals_through_month: ActualMonths(9),
            payroll_monthly_minor: 400_000,
            typed_values: BTreeMap::from([(
                ForecastLine::DeductibleCosts,
                TypedPair {
                    so_far_minor: Some(1_800_000),
                    rest_minor: None,
                },
            )]),
            earlier_profits: vec![],
        };
        assert_eq!(
            serde_json::to_value(&figures).unwrap(),
            serde_json::json!({
                "actualsThroughMonth": 9,
                "payrollMonthlyMinor": 400_000,
                "typedValues": { "deductible_costs": { "soFarMinor": 1_800_000, "restMinor": null } },
                "earlierProfits": [],
            })
        );
    }
}
