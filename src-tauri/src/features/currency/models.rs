use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Currency {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub minor_units: i64,
    pub is_custom: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FxRateRow {
    pub id: i64,
    pub date: String,
    pub from_currency_code: String,
    pub to_currency_code: String,
    pub rate_mantissa: i64,
    pub rate_exponent: i64,
    pub is_manual: bool,
    pub fetched_at: String,
}
