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

/// A likely cause of the gap between the bank app and Saldo. Every row cause closes the whole gap
/// when its fix runs.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DifferenceCause {
    /// The account was imported from CSV before and its newest row is more than 3 days old.
    /// `newest_day` (`YYYY-MM-DD`) is `None` when it has no row at all.
    StaleImport { newest_day: Option<String> },
    /// Deleting `row` closes the gap, and `copy_of` is a counted row with the same amount dated
    /// within 5 days of it.
    NearCopy {
        row: SuspectRow,
        copy_of: SuspectRow,
    },
    /// Deleting `row` closes the gap.
    EqualsGap { row: SuspectRow },
    /// Flipping the sign of `row` closes the gap. Its target is always a plain cash flow.
    FlippedSign { row: SuspectRow },
}

/// One bank line as Saldo counts it on the account: a standalone entry or a whole split group.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuspectRow {
    pub target: RowTarget,
    pub event_date: String,
    /// For a split group, the sum of its counted rows on this account.
    pub amount_minor: i64,
    pub note: Option<String>,
    pub counterpart_name: Option<String>,
}

/// What a fix acts on, and so which existing command runs it.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RowTarget {
    Cashflow {
        event_id: i64,
    },
    /// Deleting it removes both legs.
    Transfer {
        event_id: i64,
    },
    UnexplainedDifference {
        event_id: i64,
    },
    SplitGroup {
        split_group_id: i64,
        parts: u32,
    },
}
