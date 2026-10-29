use std::borrow::Cow;

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::features::transactions::models::RecordedSource;
use crate::features::transactions::SourceRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "i64")]
pub struct Bps(i64);

impl Bps {
    pub fn get(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for Bps {
    type Error = String;

    fn try_from(value: i64) -> Result<Self, String> {
        if (0..=10_000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "{value} is not between 0 and 10000 basis points (10000 is 100 %)"
            ))
        }
    }
}

impl JsonSchema for Bps {
    fn schema_name() -> Cow<'static, str> {
        "Bps".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "integer",
            "minimum": 0,
            "maximum": 10000,
            "description": "Basis points: 10000 is 100 %, 2300 is 23 %."
        })
    }
}

// Every input struct denies unknown fields: a misspelt optional field must fail, never default.
// The shares the user approves in the review table are required.

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordBatchInput {
    /// Shown in saldo_context's recent batches, such as "Invoices 2026-10".
    pub label: String,
    /// Runs the whole batch through Saldo's checks, reports what it would record, and keeps
    /// nothing.
    #[serde(default)]
    pub dry_run: bool,
    pub items: Vec<DocumentItem>,
}

/// One source document. Everything it produces (lines, assets, payment links) hangs off it.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentItem {
    pub document: DocumentId,
    pub person_id: i64,
    /// YYYY-MM-DD. The entry date only when the document has no delivery date.
    pub issue_date: String,
    /// Dátum dodania (DUZP), YYYY-MM-DD. The entry date when present.
    pub delivery_date: Option<String>,
    pub note: Option<String>,
    /// Cash flow ids (from find_payments) that paid this document. They are linked to the
    /// document's entries; an asset alone cannot take one.
    #[serde(default)]
    pub payment_ids: Vec<i64>,
    pub content: DocumentContent,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentId {
    /// IČO when printed, else IČ DPH, else the issuer's name. For revenue, the company itself.
    pub issuer: String,
    /// The invoice or credit note number as printed.
    pub number: String,
    /// The source file, echoed on the result row. Not stored.
    pub file: String,
}

/// What the document becomes, tagged by `kind`.
#[derive(Debug, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentContent {
    /// One line per VAT rate. One line becomes one entry, two or more a split group. Negative
    /// amounts make a credit note.
    Revenue(RevenueContent),
    /// Lines as for revenue, plus depreciable equipment as assets. At least one line or asset.
    Expense(ExpenseContent),
    /// A cost paid ahead for a period that ends in a later year. Saldo spreads it over the
    /// years it covers.
    PrepaidExpense(PrepaidContent),
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevenueContent {
    pub lines: Vec<RevenueLine>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpenseContent {
    #[serde(default)]
    pub lines: Vec<ExpenseLine>,
    /// Each becomes an asset account with its depreciation schedule: over 1,700.00 € and a
    /// useful life over 12 months, or record it as a line instead.
    #[serde(default)]
    pub assets: Vec<AssetPart>,
    /// Whether the company reclaims this document's VAT; the person's VAT-payer flag when
    /// missing. Also sets an asset's price: without VAT when reclaimed, with VAT otherwise.
    pub reclaimed_vat: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepaidContent {
    pub line: ExpenseLine,
    /// The last day the payment covers, YYYY-MM-DD, in a later year than the entry date.
    pub prepaid_until: String,
    /// Whether the company reclaims this document's VAT; the person's VAT-payer flag when
    /// missing.
    pub reclaimed_vat: Option<bool>,
}

/// Serde buffers an internally tagged enum before it picks the variant, which hides the path of
/// an error inside it. Parsing the variant's struct on its own keeps that path in the message.
impl<'de> Deserialize<'de> for DocumentContent {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;

        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum Kind {
            Revenue,
            Expense,
            PrepaidExpense,
        }

        fn variant<T: serde::de::DeserializeOwned>(fields: serde_json::Value) -> Result<T, String> {
            serde_path_to_error::deserialize(fields)
                .map_err(|e| format!("{}: {}", e.path(), e.inner()))
        }

        let mut fields = serde_json::Map::deserialize(deserializer)?;
        let kind = fields
            .remove("kind")
            .ok_or_else(|| D::Error::missing_field("kind"))?;
        let kind: Kind = serde_json::from_value(kind).map_err(D::Error::custom)?;
        let fields = serde_json::Value::Object(fields);
        match kind {
            Kind::Revenue => variant(fields).map(DocumentContent::Revenue),
            Kind::Expense => variant(fields).map(DocumentContent::Expense),
            Kind::PrepaidExpense => variant(fields).map(DocumentContent::PrepaidExpense),
        }
        .map_err(D::Error::custom)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevenueLine {
    /// VAT included, in minor units of the person's revenue account currency.
    pub gross_minor: i64,
    /// 0 for no VAT.
    pub vat_rate_bps: Bps,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpenseLine {
    /// VAT included, in minor units of the person's expense account currency.
    pub gross_minor: i64,
    /// 0 for no VAT.
    pub vat_rate_bps: Bps,
    /// The tax-deductible share of the cost.
    pub deductible_pct_bps: Bps,
    /// The share of the VAT that can be reclaimed.
    pub vat_reclaimable_pct_bps: Bps,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetPart {
    pub name: String,
    /// VAT included, in minor units of the consolidation currency.
    pub gross_minor: i64,
    pub vat_rate_bps: Bps,
    /// Useful life in months. Depreciation spreads the purchase price over it.
    pub depreciation_months: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UndoBatchInput {
    pub batch_id: i64,
    /// Also remove what was changed in Saldo since the batch. Send it only after the user agreed
    /// to the list a refused undo returned.
    #[serde(default)]
    pub include_changed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchReceipt {
    pub dry_run: bool,
    pub batch_id: Option<i64>,
    pub documents: Vec<RecordedDocument>,
    pub affected_tax_model_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedDocument {
    pub index: usize,
    pub file: String,
    pub source_ref: SourceRef,
    pub person_id: i64,
    pub entry_date: String,
    pub date_source: DateSource,
    pub shape: RecordedShape,
    /// Lines and assets together, VAT included.
    pub total_gross_minor: i64,
    pub lines: Vec<RecordedLine>,
    pub assets: Vec<RecordedAsset>,
    pub linked_payment_ids: Vec<i64>,
    pub warnings: Vec<Warning>,
}

impl RecordedDocument {
    pub(super) fn forget_ids(&mut self) {
        self.shape = match self.shape {
            RecordedShape::Single { .. } => RecordedShape::Single { event_id: None },
            RecordedShape::SplitGroup { .. } => RecordedShape::SplitGroup {
                split_group_id: None,
            },
            RecordedShape::Prepaid {
                generated_shares, ..
            } => RecordedShape::Prepaid {
                event_id: None,
                generated_shares,
            },
            RecordedShape::AssetsOnly => RecordedShape::AssetsOnly,
        };
        for asset in &mut self.assets {
            asset.account_id = None;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DateSource {
    DeliveryDate,
    IssueDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum RecordedShape {
    Single {
        event_id: Option<i64>,
    },
    SplitGroup {
        split_group_id: Option<i64>,
    },
    Prepaid {
        event_id: Option<i64>,
        generated_shares: usize,
    },
    AssetsOnly,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedLine {
    pub gross_minor: i64,
    pub net_minor: i64,
    pub vat_rate_bps: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedAsset {
    pub account_id: Option<i64>,
    pub name: String,
    pub purchase_price_minor: i64,
    pub price_basis: PriceBasis,
    pub depreciation_months: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceBasis {
    NetVatReclaimed,
    GrossVatNotReclaimed,
}

/// Facts the review table should show. None of them stops the batch.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "code",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Warning {
    PaymentAmountDiffers {
        payments_total_minor: i64,
        document_total_minor: i64,
    },
    PaymentAlreadyLinked {
        payment_id: i64,
        linked_entry_ids: Vec<i64>,
    },
    SimilarDocument {
        source_ref: String,
        date: String,
    },
}

#[derive(Debug)]
pub enum RecordBatchError {
    /// At least one document failed. Nothing was written.
    Items {
        dry_run: bool,
        errors: Vec<ItemError>,
        passed: Vec<RecordedDocument>,
    },
    App(AppError),
}

impl From<AppError> for RecordBatchError {
    fn from(e: AppError) -> Self {
        RecordBatchError::App(e)
    }
}

impl From<rusqlite::Error> for RecordBatchError {
    fn from(e: rusqlite::Error) -> Self {
        RecordBatchError::App(e.into())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemError {
    pub index: usize,
    pub file: String,
    /// None when the reference itself could not be built.
    pub source_ref: Option<SourceRef>,
    pub code: String,
    pub message: String,
    /// For ALREADY_RECORDED: what carries the reference.
    pub recorded: Option<RecordedSource>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoReceipt {
    pub batch_id: i64,
    pub already_undone: bool,
    pub removed_event_ids: Vec<i64>,
    pub removed_asset_account_ids: Vec<i64>,
    pub skipped_already_deleted: Vec<i64>,
    pub affected_tax_model_ids: Vec<i64>,
}

#[derive(Debug)]
pub enum UndoBatchError {
    NotFound {
        batch_id: i64,
    },
    Changed {
        batch_id: i64,
        rows: Vec<ChangedRow>,
    },
    App(AppError),
}

impl From<AppError> for UndoBatchError {
    fn from(e: AppError) -> Self {
        UndoBatchError::App(e)
    }
}

impl From<rusqlite::Error> for UndoBatchError {
    fn from(e: rusqlite::Error) -> Self {
        UndoBatchError::App(e.into())
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedRow {
    pub event_id: i64,
    pub reason: ChangeReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeReason {
    EditedInSaldo,
    LegAddedInSaldo,
    EventAddedToAsset,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSummary {
    pub id: i64,
    pub label: String,
    pub document_count: i64,
    pub created_at: String,
    pub undone_at: Option<String>,
}
