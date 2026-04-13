use crate::error::AppError;
use crate::features::ekasa::models::EkasaReceiptResponse;
use serde_json::json;

const EKASA_API_URL: &str = "https://ekasa.financnasprava.sk/mdu/api/v1/opd/receipt/find";

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

    eprintln!("[ekasa_api] POST {} body={}", EKASA_API_URL, body);

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

    eprintln!(
        "[ekasa_api] Response status={} body={}",
        status, response_text
    );

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
