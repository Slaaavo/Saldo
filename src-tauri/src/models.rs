use serde::{Deserialize, Serialize};

/// Flattened event with its current (latest) event_data.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventWithData {
    pub id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub account_type: String,
    pub event_type: String,
    pub event_date: String,
    pub amount_minor: i64,
    pub note: Option<String>,
    pub created_at: String,
    pub currency_code: String,
    pub currency_minor_units: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRow {
    pub account_id: i64,
    pub account_name: String,
    pub account_type: String,
    pub balance_minor: i64,
    pub currency_code: String,
    pub currency_minor_units: i64,
    pub converted_balance_minor: i64,
    pub fx_rate_missing: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Currency {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub minor_units: i64,
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
