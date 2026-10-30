use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Profile structs — serialized and sent to the frontend
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaImportProfileRow {
    pub id: i64,
    pub person_id: i64,
    pub default_deductible_pct_bps: i64,
    pub default_vat_reclaimable_pct_bps: i64,
    pub rules: Vec<EkasaRuleRow>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaRuleRow {
    pub id: i64,
    pub profile_id: i64,
    pub sort_order: i64,
    pub name_pattern: String,
    pub deductible_pct_bps: i64,
    pub vat_reclaimable_pct_bps: i64,
}

/// Input type for a single rule when creating or updating a profile.
/// Defined here so both `commands` and `repository` can share it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaRuleInput {
    pub sort_order: i64,
    pub name_pattern: String,
    pub deductible_pct_bps: i64,
    pub vat_reclaimable_pct_bps: i64,
}

// ---------------------------------------------------------------------------
// eKasa API response structs — deserialized from the eKasa API JSON response.
// The API uses camelCase field names; `rename_all` is applied where the Rust
// field names match the API names exactly (all camelCase). Only the fields the
// reader uses are declared; serde skips the rest.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaReceiptResponse {
    pub receipt: Option<EkasaReceiptData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaReceiptData {
    pub issue_date: String,
    pub total_price: f64,
    pub organization: EkasaReceiptOrganization,
    pub items: Vec<EkasaReceiptItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaReceiptOrganization {
    pub name: String,
    pub ico: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaReceiptItem {
    pub name: String,
    pub price: f64,
    pub vat_rate: f64,
}

/// One line of a receipt as Saldo read it. The field names are those of a receipt line in
/// `record_batch`, so Claude copies a line and only adds to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptReadLine {
    pub name: String,
    /// VAT included, in minor units.
    pub gross_minor: i64,
    pub vat_rate_bps: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineReceiptData {
    pub event_date: String,
    pub total_amount_minor: i64,
}
