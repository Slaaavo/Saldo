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
// field names match the API names exactly (all camelCase).
// EkasaReceiptData is also Serialize so it can be forwarded to the frontend.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaReceiptResponse {
    pub receipt: Option<EkasaReceiptData>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaReceiptData {
    pub receipt_id: String,
    pub issue_date: String,
    pub total_price: f64,
    pub organization: EkasaReceiptOrganization,
    pub items: Vec<EkasaReceiptItem>,
    pub vat_summary: Vec<EkasaVatSummary>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaReceiptOrganization {
    pub name: String,
    pub ico: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaReceiptItem {
    pub name: String,
    pub quantity: f64,
    pub price: f64,
    pub vat_rate: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaVatSummaryVat {
    pub vat_rate: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaVatSummary {
    pub vat: EkasaVatSummaryVat,
    pub vat_base: f64,
    pub vat_amount: f64,
}

// ---------------------------------------------------------------------------
// Processed / result structs — serialized and sent to the frontend
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessedItem {
    pub name: String,
    pub amount_minor: i64,
    pub vat_rate_bps: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineReceiptData {
    pub event_date: String,
    pub total_amount_minor: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessReceiptResult {
    pub receipt_data: Option<EkasaReceiptData>,
    pub processed_items: Vec<ProcessedItem>,
    pub qr_content: String,
    pub offline_fallback: Option<OfflineReceiptData>,
}
