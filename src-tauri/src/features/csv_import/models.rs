use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingRowInput {
    /// The row's index in the parsed file, unique within a call.
    pub index: u32,
    pub event_date: String,
    /// Signed, in the account's minor units.
    pub amount_minor: i64,
    pub note: Option<String>,
    /// An own account makes a transfer, a partner a cash flow with that partner.
    pub counterpart_account_id: Option<i64>,
    /// The row names an IBAN that matches no account in Saldo, so `counterpart_account_id` is null.
    pub names_unknown_account: bool,
    pub original_currency_id: Option<i64>,
    /// Signed like `amount_minor`.
    pub original_amount_minor: Option<i64>,
    pub fx_rate_mantissa: Option<i64>,
    pub fx_rate_exponent: Option<i64>,
    pub card: Option<String>,
    pub pending: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCsvImportInput {
    pub account_id: i64,
    pub rows: Vec<IncomingRowInput>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CsvImportPreview {
    /// In the order of the input rows.
    pub rows: Vec<RowPreview>,
    /// The rows' days (`YYYY-MM-DD`) on which the account has a live balance update, sorted.
    pub balance_update_days: Vec<String>,
    pub cancelled_holds: Vec<CancelledHold>,
    pub review_token: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CancelledHold {
    /// A split group's lowest live leg.
    pub event_id: i64,
    pub event_date: String,
    /// The ledger amount, a split group's total.
    pub amount_minor: i64,
    /// A split group's own note.
    pub note: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RowPreview {
    pub index: u32,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Verdict {
    /// Not in Saldo.
    New,
    /// Not in Saldo on this account, and the counterpart own account holds a plain cash flow
    /// that is this transfer's other side.
    CompletesTransfer {
        event_id: i64,
        account_id: i64,
        account_name: String,
        event_date: String,
        note: Option<String>,
    },
    /// Already in Saldo with this day and amount.
    InSaldo {
        event_id: i64,
        event_date: String,
        note: Option<String>,
    },
    /// The leg another account's import made for this transfer, on another day within 5 days.
    /// `event_date` is the leg's day now.
    TransferLeg {
        event_id: i64,
        other_account_name: String,
        event_date: String,
    },
    HoldPosted {
        event_id: i64,
        event_date: String,
        note: Option<String>,
    },
    HoldPostedAtOtherAmount {
        event_id: i64,
        event_date: String,
        note: Option<String>,
        hold_amount_minor: i64,
    },
    /// The booked form of the split card hold `event_id` at another amount. The row imports as a
    /// new entry, and the hold is listed among the cancelled holds.
    NewReplacingSplitHold {
        event_id: i64,
        event_date: String,
        hold_amount_minor: i64,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyCsvImportInput {
    pub account_id: i64,
    /// Exactly the rows that were previewed.
    pub rows: Vec<IncomingRowInput>,
    /// Exactly one per row index.
    pub choices: Vec<RowChoice>,
    /// Exactly one per cancelled hold of the preview.
    pub cancelled_holds: Vec<CancelledHoldChoice>,
    /// From the preview the user reviewed.
    pub review_token: String,
}

/// What `selected` does depends on the row's verdict:
///
/// | verdict | unselected | selected |
/// |---|---|---|
/// | `new` | skip | create |
/// | `completesTransfer` | skip | complete the transfer |
/// | `inSaldo` | record the bank facts and hold state, take the booked text | create a new row too |
/// | `transferLeg` | move the leg | create a new row |
/// | `holdPosted` | post the hold in place | create a new row, the hold stays pending |
/// | `holdPostedAtOtherAmount` | leave the hold, skip the row | post the hold at the row's amount |
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowChoice {
    pub index: u32,
    pub selected: bool,
    /// Ignored when `split` is set, and when the row posts a hold at another amount.
    pub bucket_id: Option<i64>,
    /// Refused on a row that posts a hold at another amount.
    pub split: Option<Vec<SplitLegChoice>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelledHoldChoice {
    pub event_id: i64,
    pub remove: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitLegChoice {
    pub amount_minor: i64,
    pub note: Option<String>,
    pub counterpart_account_id: Option<i64>,
    pub bucket_id: Option<i64>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CsvImportResult {
    /// Entries created: one per plain row, one per split leg.
    pub imported: u32,
    /// Cancelled holds removed: one per plain entry or split group.
    pub removed_holds: u32,
    /// Holds posted at the booked row's other amount.
    pub repriced_holds: u32,
}
