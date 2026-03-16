use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PartnerAccountRow {
    pub id: i64,
    pub name: String,
    pub iban: Option<String>,
    pub currency_code: String,
    pub created_at: String,
}
