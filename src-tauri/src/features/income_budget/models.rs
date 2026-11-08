use serde::Serialize;

/// An account whose incoming cashflows count as the income budget's Received.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CountedAccount {
    pub id: i64,
    pub name: String,
}
