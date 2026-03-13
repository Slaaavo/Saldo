use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BucketAllocation {
    pub id: i64,
    pub bucket_id: i64,
    pub source_account_id: i64,
    pub source_account_name: String,
    pub source_account_type: String,
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
