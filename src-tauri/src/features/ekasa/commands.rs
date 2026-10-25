use crate::error::AppError;
use crate::features::ekasa::models::{EkasaImportProfileRow, EkasaRuleInput, ProcessReceiptResult};
use crate::AppState;
use serde::Deserialize;
use tauri::State;

use super::preprocessing::QrContent;
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

    match preprocessing::classify_qr_content(&qr_string) {
        QrContent::OnlineReceiptId(receipt_id) => {
            let response = ekasa_api::fetch_ekasa_receipt(&receipt_id).await?;
            let (receipt_data, processed_items) = match response.receipt {
                Some(receipt_data) => {
                    let processed_items =
                        preprocessing::preprocess_receipt_items(&receipt_data.items);
                    (Some(receipt_data), processed_items)
                }
                None => (None, vec![]),
            };
            Ok(ProcessReceiptResult {
                receipt_data,
                processed_items,
                qr_content: qr_string,
                offline_fallback: None,
            })
        }
        QrContent::Offline(offline) => Ok(ProcessReceiptResult {
            receipt_data: None,
            processed_items: vec![],
            qr_content: qr_string,
            offline_fallback: Some(offline),
        }),
        QrContent::NotEkasa => Err(AppError {
            code: "QR_NOT_EKASA".into(),
            message: "The QR code in this file is not an eKasa receipt code. \
                      Choose a file that shows the QR code printed on the receipt."
                .into(),
        }),
    }
}
