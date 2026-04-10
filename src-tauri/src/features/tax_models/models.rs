use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaxTier {
    pub threshold_minor: i64,
    pub rate_bps: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventDataForCalc {
    pub event_id: i64,
    pub event_type: String,
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
    pub event_type: String,
    pub event_date: String,
    pub note: Option<String>,
    pub amount_minor: i64,
    pub net_amount_minor: i64,
    pub vat_amount_minor: i64,
    pub reclaimable_vat_minor: i64,
    pub non_reclaimable_vat_minor: i64,
    pub tax_deductible_cost_minor: i64,
    pub non_tax_deductible_cost_minor: i64,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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
}
