use serde::{Deserialize, Serialize};

use crate::features::buckets::BucketLink;

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
    pub counterpart_account_id: Option<i64>,
    pub counterpart_account_name: Option<String>,
    pub bucket_id: Option<i64>,
    pub bucket_name: Option<String>,
    pub original_currency_id: Option<i64>,
    pub original_currency_code: Option<String>,
    pub original_amount_minor: Option<i64>,
    pub original_currency_minor_units: Option<i64>,
    pub fx_rate_mantissa: Option<i64>,
    pub fx_rate_exponent: Option<i64>,
    pub linked_event_id: Option<i64>,
    pub split_group_id: Option<i64>,
    pub split_group_note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListEventsResult {
    pub events: Vec<EventWithData>,
    pub total_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRow {
    pub account_id: i64,
    pub account_name: String,
    pub account_type: String,
    pub iban: Option<String>,
    pub balance_minor: i64,
    pub currency_code: String,
    pub currency_minor_units: i64,
    /// True when this account's currency is a custom unit (not a standard currency).
    pub is_custom: bool,
    pub converted_balance_minor: i64,
    pub fx_rate_missing: bool,
    /// True when this account (account_type='account') is linked to at least one asset.
    pub is_linked_to_asset: bool,
    /// For account-type rows: asset IDs this account is linked to.
    /// For asset-type rows: account IDs linked to this asset.
    pub linked_asset_ids: Vec<i64>,
    /// For account-type rows: true when this account is the source of an active bucket link.
    pub is_bucket_linked: bool,
    /// For bucket-type rows: the event-bound links contributing to this bucket's balance.
    pub bucket_links: Vec<BucketLink>,
    /// For bucket-type rows: converted sum of linked account balances (consolidation currency).
    pub linked_balance_minor: i64,
    /// For bucket-type rows: converted sum of cashflows tagged to this bucket via event_data.bucket_id (consolidation currency).
    pub cashflow_tagged_minor: i64,
    pub person_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitGroupEntry {
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub counterpart_account_id: Option<i64>,
    pub bucket_id: Option<i64>,
    pub original_currency_id: Option<i64>,
    pub original_amount_minor: Option<i64>,
    pub fx_rate_mantissa: Option<i64>,
    pub fx_rate_exponent: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSplitGroupInput {
    pub account_id: i64,
    pub group_note: Option<String>,
    pub legs: Vec<SplitGroupEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSplitGroupDateInput {
    pub split_group_id: i64,
    pub new_date: String,
}
