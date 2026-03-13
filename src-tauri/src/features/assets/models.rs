use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountAssetLink {
    pub id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub asset_id: i64,
    pub asset_name: String,
}
