use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Currency {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub minor_units: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub currency_id: i64,
    pub created_at: String,
}

/// Flattened event with its current (latest) event_data.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventWithData {
    pub id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub event_type: String,
    pub event_date: String,
    pub amount_minor: i64,
    pub note: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRow {
    pub account_id: i64,
    pub account_name: String,
    pub balance_minor: i64,
}
