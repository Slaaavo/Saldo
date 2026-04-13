use crate::error::AppError;
use crate::features::ekasa::models::{EkasaImportProfileRow, EkasaRuleInput, ProcessReceiptResult};
use crate::AppState;
use serde::Deserialize;
use tauri::State;

use super::{ekasa_api, preprocessing, qr_decoder, repository};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertEkasaProfileInput {
    pub person_id: i64,
    pub default_deductible_pct_bps: i64,
    pub default_vat_reclaimable_pct_bps: i64,
    pub rules: Vec<EkasaRuleInput>,
}

#[tauri::command]
pub fn get_ekasa_profile(
    state: State<'_, AppState>,
    person_id: i64,
) -> Result<Option<EkasaImportProfileRow>, AppError> {
    let conn = state.conn()?;
    repository::get_ekasa_profile_for_person(&conn, person_id)
}

#[tauri::command]
pub fn upsert_ekasa_profile(
    state: State<'_, AppState>,
    input: UpsertEkasaProfileInput,
) -> Result<EkasaImportProfileRow, AppError> {
    let conn = state.conn()?;
    repository::upsert_ekasa_profile(
        &conn,
        repository::UpsertEkasaProfileParams {
            person_id: input.person_id,
            default_deductible_pct_bps: input.default_deductible_pct_bps,
            default_vat_reclaimable_pct_bps: input.default_vat_reclaimable_pct_bps,
            rules: input.rules,
        },
    )
}

#[tauri::command]
pub fn delete_ekasa_profile(state: State<'_, AppState>, profile_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    repository::delete_ekasa_profile(&conn, profile_id)
}

#[tauri::command]
pub fn list_ekasa_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<EkasaImportProfileRow>, AppError> {
    let conn = state.conn()?;
    repository::list_ekasa_profiles(&conn)
}

/// Extracts the receipt ID from a QR code URL string.
///
/// Returns `Some(id)` only if the string contains a `receiptId=` query parameter,
/// as seen in online eKasa QR codes. Returns `None` for plain IDs and offline QR formats.
fn extract_receipt_id_from_qr(qr_content: &str) -> Option<String> {
    // URL format: ...?receiptId=XXXX[&...]
    if let Some(pos) = qr_content.find("receiptId=") {
        let after = &qr_content[pos + "receiptId=".len()..];
        let end = after.find('&').unwrap_or(after.len());
        let id = after[..end].trim();
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    None
}

#[tauri::command]
pub async fn process_receipt_file(file_path: String) -> Result<ProcessReceiptResult, AppError> {
    // QR decoding involves PDF rendering (PDFium) and image processing (rxing),
    // both CPU-heavy synchronous operations. Must run on a blocking thread to
    // avoid stalling Tauri's async executor.
    let qr_string =
        tauri::async_runtime::spawn_blocking(move || qr_decoder::decode_qr_from_file(&file_path))
            .await
            .map_err(|e| AppError {
                code: "TASK_ERROR".into(),
                message: format!("QR decoding task failed: {}", e),
            })??;

    // Branch 1: URL with receiptId= param → extract ID and call API
    if let Some(receipt_id) = extract_receipt_id_from_qr(&qr_string) {
        let response = ekasa_api::fetch_ekasa_receipt(&receipt_id).await?;
        return match response.receipt {
            Some(receipt_data) => {
                let processed_items = preprocessing::preprocess_receipt_items(&receipt_data.items);
                Ok(ProcessReceiptResult {
                    receipt_data: Some(receipt_data),
                    processed_items,
                    qr_content: qr_string,
                    offline_fallback: None,
                })
            }
            None => Ok(ProcessReceiptResult {
                receipt_data: None,
                processed_items: vec![],
                qr_content: qr_string,
                offline_fallback: None,
            }),
        };
    }

    // Branch 2: Multiple colons → likely offline QR format; try offline parse first
    let colon_count = qr_string.chars().filter(|&c| c == ':').count();
    if colon_count > 2 {
        if let Some(offline) = preprocessing::parse_offline_qr(&qr_string) {
            return Ok(ProcessReceiptResult {
                receipt_data: None,
                processed_items: vec![],
                qr_content: qr_string,
                offline_fallback: Some(offline),
            });
        }
        // Offline parse failed — fall through to API attempt with the raw string
    }

    // Branch 3: Plain alphanumeric ID (no colons, or offline parse failed) → call API
    let receipt_id = qr_string.trim().to_string();
    let response = ekasa_api::fetch_ekasa_receipt(&receipt_id).await?;
    match response.receipt {
        Some(receipt_data) => {
            let processed_items = preprocessing::preprocess_receipt_items(&receipt_data.items);
            Ok(ProcessReceiptResult {
                receipt_data: Some(receipt_data),
                processed_items,
                qr_content: qr_string,
                offline_fallback: None,
            })
        }
        None => Ok(ProcessReceiptResult {
            receipt_data: None,
            processed_items: vec![],
            qr_content: qr_string,
            offline_fallback: None,
        }),
    }
}
