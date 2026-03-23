use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProfileRuleRow {
    pub id: i64,
    pub profile_id: i64,
    pub rule_type: String,
    pub sort_order: i64,
    pub params_json: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProfileRow {
    pub id: i64,
    pub name: String,
    pub column_mapping_json: String,
    pub rules: Vec<ImportProfileRuleRow>,
    pub created_at: String,
    pub updated_at: String,
}

/// Input type for a single rule when creating or updating a profile.
/// Defined here so both `commands` and `repository` can share it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    pub rule_type: String,
    pub sort_order: i64,
    pub params_json: String,
}
