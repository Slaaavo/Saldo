use serde::{Deserialize, Serialize};

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
