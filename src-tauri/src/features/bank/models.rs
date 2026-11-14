use serde::Serialize;

/// Whether an own account's newest check still matches Saldo. Derived on every read, never stored.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Verification {
    Verified {
        check_id: i64,
        day: String,
    },
    Differs {
        check_id: i64,
        day: String,
        /// The bank's balance minus Saldo's side of the check.
        difference_minor: i64,
    },
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BookedDifference {
    pub event_id: i64,
    pub check_id: i64,
}
