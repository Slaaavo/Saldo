use crate::error::AppError;
use crate::features::ekasa::models::{EkasaReceiptData, EkasaReceiptResponse};
use crate::features::ekasa::reading::EkasaApi;
use serde_json::json;

const EKASA_API_URL: &str = "https://ekasa.financnasprava.sk/mdu/api/v1/opd/receipt/find";

/// The real service. It waits on Tauri's async runtime, so the app calls it from a blocking
/// thread, never from an async task.
pub struct LiveEkasaApi;

impl EkasaApi for LiveEkasaApi {
    fn fetch(&self, receipt_id: &str) -> Result<Option<EkasaReceiptData>, AppError> {
        tauri::async_runtime::block_on(fetch_ekasa_receipt(receipt_id))
            .map(|response| response.receipt)
    }
}

pub async fn fetch_ekasa_receipt(receipt_id: &str) -> Result<EkasaReceiptResponse, AppError> {
    let client = reqwest::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .https_only(true)
        .build()
        .map_err(|e| AppError {
            code: "NETWORK_ERROR".into(),
            message: format!("Failed to build HTTP client: {}", e),
        })?;

    let body = json!({ "receiptId": receipt_id });

    let response = client
        .post(EKASA_API_URL)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError {
            code: "NETWORK_ERROR".into(),
            message: format!("eKasa API request failed: {}", e),
        })?;

    let status = response.status();
    let response_text = response.text().await.map_err(|e| AppError {
        code: "NETWORK_ERROR".into(),
        message: format!("Failed to read eKasa API response body: {}", e),
    })?;

    if !status.is_success() {
        return Err(AppError {
            code: "EKASA_API_ERROR".into(),
            message: format!("eKasa API returned status {}", status),
        });
    }

    let receipt_response: EkasaReceiptResponse =
        serde_json::from_str(&response_text).map_err(|e| AppError {
            code: "PARSE_ERROR".into(),
            message: format!("Failed to parse eKasa API response: {}", e),
        })?;

    Ok(receipt_response)
}
