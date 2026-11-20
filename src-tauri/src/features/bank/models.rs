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

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DifferenceCause {
    StaleImport { newest_day: Option<String> },
    NearCopy { row: BankLine, copy_of: BankLine },
    EqualsGap { row: BankLine },
    FlippedSign { row: BankLine<CashflowTarget> },
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BankLine<T = RowTarget> {
    pub target: T,
    pub event_date: String,
    pub amount_minor: i64,
    pub note: Option<String>,
    pub counterpart_name: Option<String>,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RowTarget {
    Cashflow { event_id: i64 },
    Transfer { event_id: i64 },
    UnexplainedDifference { event_id: i64 },
    SplitGroup { split_group_id: i64, parts: u32 },
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CashflowTarget {
    Cashflow { event_id: i64 },
}
