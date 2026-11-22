use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, Deserialize)]
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
    pub original_currency_id: Option<i64>,
    /// Signed like `amount_minor`.
    pub original_amount_minor: Option<i64>,
    pub fx_rate_mantissa: Option<i64>,
    pub fx_rate_exponent: Option<i64>,
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
    pub review_token: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RowPreview {
    pub index: u32,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, Hash, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Verdict {
    /// Not in Saldo. Ticked by default.
    New,
    /// Not in Saldo on this account, and the counterpart own account holds a plain cash flow
    /// that is this transfer's other side. Importing it turns that cash flow into the other leg.
    /// Ticked by default.
    CompletesTransfer {
        event_id: i64,
        account_id: i64,
        account_name: String,
        event_date: String,
        note: Option<String>,
    },
    /// Already in Saldo with this day and amount. Unticked by default.
    InSaldo {
        event_id: i64,
        event_date: String,
        note: Option<String>,
    },
    /// The leg another account's import made for this transfer, on another day within 5 days.
    /// `event_date` is the leg's day now; importing moves the leg to the row's day. Unticked by
    /// default.
    TransferLeg {
        event_id: i64,
        other_account_name: String,
        event_date: String,
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
    /// From the preview the user reviewed.
    pub review_token: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowChoice {
    pub index: u32,
    pub selected: bool,
    /// Ignored when `split` is set.
    pub bucket_id: Option<i64>,
    pub split: Option<Vec<SplitLegChoice>>,
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
}
