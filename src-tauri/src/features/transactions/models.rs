use serde::{Deserialize, Serialize};

use crate::features::buckets::{AllocationDetail, BucketAllocation};

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
    /// True when this account's currency is a custom unit (not a standard currency).
    pub is_custom: bool,
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
    /// For bucket-type rows: converted sum of allocations sourced from asset-type accounts (consolidation currency).
    pub linked_allocations_from_assets_minor: i64,
    /// True when this account (account_type='account') is linked to at least one asset.
    pub is_linked_to_asset: bool,
    /// For account-type rows: asset IDs this account is linked to.
    /// For asset-type rows: account IDs linked to this asset.
    pub linked_asset_ids: Vec<i64>,
}
