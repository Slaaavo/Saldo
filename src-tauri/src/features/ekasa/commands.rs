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

fn validate_bps(name: &str, value: i64) -> Result<(), AppError> {
    if !(0..=10000).contains(&value) {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: format!("{} must be in range [0, 10000] bps", name),
        });
    }
    Ok(())
}

fn validate_upsert_profile_input(input: &UpsertEkasaProfileInput) -> Result<(), AppError> {
    validate_bps(
        "default_deductible_pct_bps",
        input.default_deductible_pct_bps,
    )?;
    validate_bps(
        "default_vat_reclaimable_pct_bps",
        input.default_vat_reclaimable_pct_bps,
    )?;
    for (index, rule) in input.rules.iter().enumerate() {
        if rule.name_pattern.trim().is_empty() {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: format!("Rule {}: name pattern must not be blank.", index + 1),
            });
        }
        validate_bps("deductible_pct_bps", rule.deductible_pct_bps)?;
        validate_bps("vat_reclaimable_pct_bps", rule.vat_reclaimable_pct_bps)?;
    }
    Ok(())
}

#[tauri::command]
pub fn upsert_ekasa_profile(
    state: State<'_, AppState>,
    input: UpsertEkasaProfileInput,
) -> Result<EkasaImportProfileRow, AppError> {
    validate_upsert_profile_input(&input)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(name_pattern: &str, deductible: i64, vat_reclaimable: i64) -> EkasaRuleInput {
        EkasaRuleInput {
            sort_order: 0,
            name_pattern: name_pattern.to_string(),
            deductible_pct_bps: deductible,
            vat_reclaimable_pct_bps: vat_reclaimable,
        }
    }

    fn input(
        default_deductible: i64,
        default_vat_reclaimable: i64,
        rules: Vec<EkasaRuleInput>,
    ) -> UpsertEkasaProfileInput {
        UpsertEkasaProfileInput {
            person_id: 1,
            default_deductible_pct_bps: default_deductible,
            default_vat_reclaimable_pct_bps: default_vat_reclaimable,
            rules,
        }
    }

    fn assert_validation_error(input: UpsertEkasaProfileInput) {
        let err = validate_upsert_profile_input(&input).unwrap_err();
        assert_eq!(err.code, "VALIDATION");
    }

    #[test]
    fn upsert_accepts_bounds_of_the_bps_range() {
        let ok = input(0, 10000, vec![rule("coffee", 10000, 0)]);
        assert!(validate_upsert_profile_input(&ok).is_ok());
    }

    #[test]
    fn upsert_rejects_default_bps_out_of_range() {
        assert_validation_error(input(-1, 10000, vec![]));
        assert_validation_error(input(10001, 10000, vec![]));
        assert_validation_error(input(10000, -1, vec![]));
        assert_validation_error(input(10000, 10001, vec![]));
    }

    #[test]
    fn upsert_rejects_rule_bps_out_of_range() {
        assert_validation_error(input(10000, 10000, vec![rule("a", -1, 0)]));
        assert_validation_error(input(10000, 10000, vec![rule("a", 10001, 0)]));
        assert_validation_error(input(10000, 10000, vec![rule("a", 0, -1)]));
        assert_validation_error(input(10000, 10000, vec![rule("a", 0, 10001)]));
    }

    #[test]
    fn upsert_rejects_blank_and_whitespace_patterns() {
        for pattern in ["", " ", "\t\n"] {
            assert_validation_error(input(
                10000,
                10000,
                vec![rule("coffee", 0, 0), rule(pattern, 0, 0)],
            ));
        }
    }
}
