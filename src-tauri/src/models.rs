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
    /// Sum of allocations from this account across all buckets (account's currency).
    /// Only meaningful for account-type rows.
    pub allocated_total_minor: i64,
    /// For bucket-type rows: converted sum of linked allocations (consolidation currency).
    pub linked_allocations_balance_minor: i64,
    /// Only populated when account is over-allocated; empty otherwise.
    pub over_allocation_buckets: Vec<AllocationDetail>,
    /// For bucket-type rows: the raw allocations contributing to this bucket's balance.
    pub linked_allocations: Vec<BucketAllocation>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BucketAllocation {
    pub id: i64,
    pub bucket_id: i64,
    pub source_account_id: i64,
    pub source_account_name: String,
    pub source_currency_id: i64,
    pub source_currency_code: String,
    pub source_currency_minor_units: i64,
    pub amount_minor: i64,
    pub effective_date: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AllocationDetail {
    pub bucket_id: i64,
    pub bucket_name: String,
    pub amount_minor: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OverAllocationWarning {
    pub source_account_id: i64,
    pub source_account_name: String,
    pub currency_code: String,
    pub currency_minor_units: i64,
    pub balance_minor: i64,
    pub total_allocated_minor: i64,
    pub over_allocation_minor: i64,
    pub allocations: Vec<AllocationDetail>,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbLocationInfo {
    pub current_path: String,
    pub is_default: bool,
    pub is_demo_mode: bool,
    pub fallback_warning: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickDbFolderResult {
    pub folder: String,
    pub db_exists: bool,
}
