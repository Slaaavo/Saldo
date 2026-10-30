use crate::error::AppError;
use crate::features::ekasa::models::{EkasaImportProfileRow, EkasaRuleInput};
use crate::features::transactions::repository::find_recorded_source;
use crate::features::transactions::SourceRef;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use super::ekasa_api::LiveEkasaApi;
use super::reading::{read_receipt_file, ReceiptReading};
use super::repository;
use super::rules::{self, RuleMatcher, RuleProblem};

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
        if let Some(problem) = rules::pattern_problem(&rule.name_pattern) {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: format!(
                    "The pattern '{}' is not valid: {problem}.",
                    rule.name_pattern
                ),
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

/// Takes the DB lock only for the lookup, so callers can `.await` right after.
fn is_already_imported(state: &AppState, source_ref: &SourceRef) -> Result<bool, AppError> {
    let conn = state.conn()?;
    Ok(find_recorded_source(&conn, source_ref)?.is_some())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessReceiptResult {
    #[serde(flatten)]
    pub reading: ReceiptReading,
    /// Whether a live entry already carries this receipt's reference.
    pub already_imported: bool,
}

#[tauri::command]
pub async fn process_receipt_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    file_path: String,
) -> Result<ProcessReceiptResult, AppError> {
    // A resolution failure only drops this directory from the PDFium search.
    let resource_dir = app.path().resource_dir().ok();
    let reading = tauri::async_runtime::spawn_blocking(move || {
        read_receipt_file(&file_path, None, resource_dir.as_deref(), &LiveEkasaApi)
    })
    .await
    .map_err(|e| AppError {
        code: "TASK_ERROR".into(),
        message: format!("Reading the receipt failed: {}", e),
    })??;
    Ok(ProcessReceiptResult {
        already_imported: is_already_imported(&state, &reading.source_ref)?,
        reading,
    })
}

/// The wizard keeps each rule's percentages as the user typed them, so it sends only the
/// patterns and copies the matched row's own percentages into the leg.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchEkasaRulesInput {
    pub item_names: Vec<String>,
    /// The rules step's patterns in row order, blank rows included.
    pub patterns: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaRuleMatches {
    /// Per item name, the row of the first rule that matches it, numbered from 1 as in
    /// `invalid_rules`; None when no rule matches.
    pub matched_rows: Vec<Option<usize>>,
    pub invalid_rules: Vec<RuleProblem>,
}

#[tauri::command]
pub fn match_ekasa_rules(input: MatchEkasaRulesInput) -> EkasaRuleMatches {
    let matcher = RuleMatcher::compile(input.patterns.iter().map(String::as_str));
    EkasaRuleMatches {
        matched_rows: input
            .item_names
            .iter()
            .map(|name| matcher.position_for(name))
            .collect(),
        invalid_rules: matcher.problems().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::ekasa::models::EkasaReceiptData;
    use crate::features::ekasa::reading::EkasaApi;

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
    fn upsert_rejects_a_pattern_the_matcher_cannot_use() {
        let err = validate_upsert_profile_input(&input(
            10000,
            10000,
            vec![rule("coffee", 0, 0), rule("kava(?=1kg)", 0, 0)],
        ))
        .unwrap_err();

        assert_eq!(err.code, "VALIDATION");
        assert!(
            err.message.starts_with("The pattern 'kava(?=1kg)' "),
            "{}",
            err.message
        );
        assert!(
            validate_upsert_profile_input(&input(10000, 10000, vec![rule("káva|čaj", 0, 0)]))
                .is_ok()
        );
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

    fn match_rules(item_names: &[&str], patterns: &[&str]) -> serde_json::Value {
        serde_json::to_value(match_ekasa_rules(MatchEkasaRulesInput {
            item_names: item_names.iter().map(|name| name.to_string()).collect(),
            patterns: patterns.iter().map(|pattern| pattern.to_string()).collect(),
        }))
        .unwrap()
    }

    #[test]
    fn the_wizard_gets_the_matching_row_of_each_item() {
        let matches = match_rules(
            &["KÁVA ZRNKOVÁ", "MLIEKO 1L", "ROZOK", "PAPIER A4"],
            &["", "kava", "mlieko|rozok"],
        );

        assert_eq!(
            matches,
            serde_json::json!({ "matchedRows": [2, 3, 3, null], "invalidRules": [] })
        );
    }

    #[test]
    fn the_wizard_hears_of_a_rule_that_does_not_compile() {
        let matches = match_rules(&["KAVA"], &["ka(va", "kava"]);

        assert_eq!(matches["matchedRows"], serde_json::json!([2]));
        assert_eq!(matches["invalidRules"][0]["position"], 1);
        assert_eq!(matches["invalidRules"][0]["pattern"], "ka(va");
    }

    struct NoService;

    impl EkasaApi for NoService {
        fn fetch(&self, _: &str) -> Result<Option<EkasaReceiptData>, AppError> {
            panic!("an offline receipt never asks the eKasa service")
        }
    }

    #[test]
    fn the_wizard_gets_the_reading_with_whether_it_was_imported() {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ekasa/offline-receipt.png"
        );
        let reading = read_receipt_file(fixture, None, None, &NoService).unwrap();

        let result = serde_json::to_value(ProcessReceiptResult {
            reading,
            already_imported: true,
        })
        .unwrap();

        assert_eq!(result["kind"], "offline");
        assert_eq!(result["date"], "2018-02-13");
        assert_eq!(result["totalMinor"], 23723);
        assert_eq!(result["lines"], serde_json::Value::Null);
        assert_eq!(result["vendor"], serde_json::Value::Null);
        assert!(result["sourceRef"]
            .as_str()
            .is_some_and(|r| r.starts_with("offline:C44B3977-")));
        assert_eq!(result["alreadyImported"], true);
    }
}
