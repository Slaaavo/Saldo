use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BucketLink {
    pub id: i64,
    pub event_id: i64,
    pub source_account_id: i64,
    pub source_account_name: String,
    pub source_currency_id: i64,
    pub source_currency_code: String,
    pub source_currency_minor_units: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinkConflict {
    pub source_account_id: i64,
    pub source_account_name: String,
    pub conflict_date: String,
    pub other_bucket_id: i64,
    pub other_bucket_name: String,
}
