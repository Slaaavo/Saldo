use serde::{Deserialize, Serialize};

use crate::features::buckets::BucketLink;

/// Flattened event with its current (latest) event_data.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventWithData {
    pub id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub account_type: String,
    pub event_type: String,
    pub event_date: String,
    pub amount_minor: i64,
    pub note: Option<String>,
    pub created_at: String,
    pub currency_code: String,
    pub currency_minor_units: i64,
    pub counterpart_account_id: Option<i64>,
    pub counterpart_account_name: Option<String>,
    pub bucket_id: Option<i64>,
    pub bucket_name: Option<String>,
    pub original_currency_id: Option<i64>,
    pub original_currency_code: Option<String>,
    pub original_amount_minor: Option<i64>,
    pub original_currency_minor_units: Option<i64>,
    pub fx_rate_mantissa: Option<i64>,
    pub fx_rate_exponent: Option<i64>,
    pub linked_event_id: Option<i64>,
    pub split_group_id: Option<i64>,
    pub split_group_note: Option<String>,
    pub vat_rate_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
    pub expense_deductible_pct_bps: Option<i64>,
    pub prepaid_until: Option<String>,
    pub is_linked_to_taxable: bool,
    pub linked_taxable_event_id: Option<i64>,
    pub has_linked_cashflows: bool,
    pub linked_cashflow_count: i64,
    pub linked_asset_id: Option<i64>,
    pub linked_prepaid_event_id: Option<i64>,
    pub is_system_generated: bool,
    pub reclaimed_vat: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListEventsResult {
    pub events: Vec<EventWithData>,
    pub total_count: i64,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Account,
    Bucket,
    Asset,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RateUsed {
    pub mantissa: i64,
    pub exponent: i64,
    /// False: 1 consolidation unit = rate held units. True (custom units): 1 held unit = rate
    /// consolidation units.
    pub is_direct: bool,
    pub date: String,
    pub is_manual: bool,
    pub age_days: i64,
    pub stale: bool,
}

/// An amount's value in the consolidation currency. Without a stored rate there is no value, and
/// nothing falls back to 1:1.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Valuation {
    SameCurrency { value_minor: i64 },
    Converted { value_minor: i64, rate: RateUsed },
    NoRate,
}

impl Valuation {
    pub fn value_minor(&self) -> Option<i64> {
        match self {
            Valuation::SameCurrency { value_minor } | Valuation::Converted { value_minor, .. } => {
                Some(*value_minor)
            }
            Valuation::NoRate => None,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ValuedBucketLink {
    #[serde(flatten)]
    pub link: BucketLink,
    pub balance_minor: i64,
    pub valuation: Valuation,
}

/// The cashflows of one source account tagged to a bucket after the bucket's latest balance update.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaggedCashflows {
    pub source_account_id: i64,
    pub source_account_name: String,
    pub currency_code: String,
    pub currency_minor_units: i64,
    pub is_custom: bool,
    pub amount_minor: i64,
    pub valuation: Valuation,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRow {
    pub account_id: i64,
    pub account_name: String,
    pub account_type: AccountKind,
    pub iban: Option<String>,
    pub balance_minor: i64,
    pub currency_code: String,
    pub currency_minor_units: i64,
    /// True when this account's currency is a custom unit (not a standard currency).
    pub is_custom: bool,
    pub valuation: Valuation,
    /// True when this account (account_type='account') is linked to at least one asset.
    pub is_linked_to_asset: bool,
    /// For account-type rows: asset IDs this account is linked to.
    /// For asset-type rows: account IDs linked to this asset.
    pub linked_asset_ids: Vec<i64>,
    /// For account-type rows: true when this account is the source of an active bucket link.
    pub is_bucket_linked: bool,
    pub bucket_links: Vec<ValuedBucketLink>,
    pub tagged_cashflows: Vec<TaggedCashflows>,
    /// The sum of this row's valued parts. A bucket's parts are its own balance, its links and its
    /// tagged cashflows.
    pub counted_minor: i64,
    /// Set only for an asset with linked accounts. None when the asset or a linked account has no
    /// rate.
    pub equity_minor: Option<i64>,
    pub person_id: Option<i64>,
    /// Asset depreciation fields — only populated for asset accounts with standard currencies.
    pub purchase_price_minor: Option<i64>,
    pub purchase_date: Option<String>,
    pub depreciation_period_months: Option<i64>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Money {
    pub amount_minor: i64,
    pub currency_code: String,
    pub minor_units: i64,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LinePart {
    Balance,
    LinkedAccount {
        source_account_id: i64,
        source_account_name: String,
    },
    TaggedCashflows {
        source_account_id: i64,
        source_account_name: String,
    },
    BucketTotal,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LineSubject {
    pub account_id: i64,
    pub account_name: String,
    pub account_type: AccountKind,
    pub part: LinePart,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CountedLine {
    #[serde(flatten)]
    pub subject: LineSubject,
    pub native: Money,
    /// None for an amount already in the consolidation currency.
    pub rate: Option<RateUsed>,
    /// This line's contribution to the metric's total, sign included.
    pub signed_minor: i64,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkippedLine {
    #[serde(flatten)]
    pub subject: LineSubject,
    pub amount: Money,
    pub reason: NotCountedReason,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NotCountedReason {
    NoRate,
    LinkedToAsset,
    Earmark,
    NotLiquid,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MetricKind {
    NetWorth,
    Liquid,
    TotalBalance,
    LeftToSpend,
}

/// A headline number with every line behind it. Built only by `Metric::new`, so the total is
/// always the sum of the counted lines.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    kind: MetricKind,
    total_minor: i64,
    counted: Vec<CountedLine>,
    skipped: Vec<SkippedLine>,
}

impl Metric {
    pub fn new(kind: MetricKind, counted: Vec<CountedLine>, skipped: Vec<SkippedLine>) -> Metric {
        Metric {
            kind,
            total_minor: counted.iter().map(|line| line.signed_minor).sum(),
            counted,
            skipped,
        }
    }
}

#[cfg(test)]
impl Metric {
    pub fn kind(&self) -> MetricKind {
        self.kind
    }

    pub fn total_minor(&self) -> i64 {
        self.total_minor
    }

    pub fn counted(&self) -> &[CountedLine] {
        &self.counted
    }

    pub fn skipped(&self) -> &[SkippedLine] {
        &self.skipped
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RateProblem {
    NoRate,
    Stale { rate: RateUsed, limit_days: i64 },
}

/// One per account whose currency has no rate or an old one. `native` is the account's own
/// balance, or what it tagged to a bucket when its own balance is zero or not in the snapshot.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RateWarning {
    pub account_id: i64,
    pub account_name: String,
    pub native: Money,
    pub problem: RateProblem,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub rows: Vec<SnapshotRow>,
    pub metrics: Vec<Metric>,
    pub rate_warnings: Vec<RateWarning>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitGroupEntry {
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub counterpart_account_id: Option<i64>,
    pub bucket_id: Option<i64>,
    pub original_currency_id: Option<i64>,
    pub original_amount_minor: Option<i64>,
    pub fx_rate_mantissa: Option<i64>,
    pub fx_rate_exponent: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSplitGroupInput {
    pub account_id: i64,
    pub group_note: Option<String>,
    pub legs: Vec<SplitGroupEntry>,
}

/// The kinds of taxable entry a document can become.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaxableKind {
    Revenue,
    Expense,
    PrepaidExpense,
}

impl TaxableKind {
    pub fn event_type(self) -> &'static str {
        match self {
            TaxableKind::Revenue => "revenue",
            TaxableKind::Expense => "expense",
            TaxableKind::PrepaidExpense => "prepaid_expense",
        }
    }
}

impl rusqlite::types::FromSql for TaxableKind {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value.as_str()? {
            "revenue" => Ok(TaxableKind::Revenue),
            "expense" => Ok(TaxableKind::Expense),
            "prepaid_expense" => Ok(TaxableKind::PrepaidExpense),
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        }
    }
}

/// One source document as Saldo holds it: a single entry, or the legs of a split group.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentEntry {
    pub kind: TaxableKind,
    pub person_id: i64,
    pub date: String,
    pub total_gross_minor: i64,
    pub note: Option<String>,
    pub source_ref: Option<String>,
    pub batch_id: Option<i64>,
    pub split_group_id: Option<i64>,
    pub prepaid_until: Option<String>,
    pub reclaimed_vat: Option<bool>,
    pub lines: Vec<EntryLine>,
    pub linked_payment_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryLine {
    pub event_id: i64,
    pub gross_minor: i64,
    pub net_minor: i64,
    pub vat_rate_bps: Option<i64>,
    pub deductible_pct_bps: Option<i64>,
    pub vat_reclaimable_pct_bps: Option<i64>,
}

/// A cash flow that can pay a document.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Payment {
    pub event_id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub person_id: Option<i64>,
    pub date: String,
    pub amount_minor: i64,
    pub currency: String,
    pub original_amount_minor: Option<i64>,
    pub original_currency: Option<String>,
    pub note: Option<String>,
    pub linked_entry_ids: Vec<i64>,
}

/// What already carries a document's reference: a live entry or an asset account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum RecordedSource {
    Entry {
        event_id: i64,
        batch_id: Option<i64>,
        date: String,
    },
    Asset {
        asset_account_id: i64,
        batch_id: Option<i64>,
        date: Option<String>,
    },
}
