use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PersonRow {
    pub id: i64,
    pub name: String,
    pub person_type: String,
    pub is_default: bool,
    pub created_at: String,
    pub default_revenue_account_id: i64,
    pub default_expense_account_id: i64,
}
