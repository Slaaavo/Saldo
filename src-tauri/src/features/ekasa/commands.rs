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
use super::rules::{self, RuleMatch, RuleProblem, RuleSource, ShareRules, Shares};

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
                    "Rule {}: the pattern '{}' is not valid: {problem}.",
                    index + 1,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchEkasaRulesInput {
    pub item_names: Vec<String>,
    pub rules: Vec<RuleDraftInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleDraftInput {
    pub name_pattern: String,
    pub deductible_pct_bps: i64,
    pub vat_reclaimable_pct_bps: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EkasaRuleMatches {
    pub items: Vec<ItemRuleMatch>,
    pub invalid_rules: Vec<RuleProblem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRuleMatch {
    pub matched_rule: Option<RuleMatch>,
}

#[tauri::command]
pub fn match_ekasa_rules(input: MatchEkasaRulesInput) -> Result<EkasaRuleMatches, AppError> {
    for rule in &input.rules {
        validate_bps("deductible_pct_bps", rule.deductible_pct_bps)?;
        validate_bps("vat_reclaimable_pct_bps", rule.vat_reclaimable_pct_bps)?;
    }
    let rules = ShareRules::compile(input.rules.iter().map(|rule| RuleSource {
        pattern: &rule.name_pattern,
        shares: Shares {
            deductible_pct_bps: rule.deductible_pct_bps,
            vat_reclaimable_pct_bps: rule.vat_reclaimable_pct_bps,
        },
    }));
    Ok(EkasaRuleMatches {
        items: input
            .item_names
            .iter()
            .map(|name| ItemRuleMatch {
                matched_rule: rules.shares_for(name).cloned(),
            })
            .collect(),
        invalid_rules: rules.problems().to_vec(),
    })
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
        assert!(err.message.starts_with("Rule 2: "), "{}", err.message);
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

    fn draft(name_pattern: &str, deductible_pct_bps: i64) -> RuleDraftInput {
        RuleDraftInput {
            name_pattern: name_pattern.to_owned(),
            deductible_pct_bps,
            vat_reclaimable_pct_bps: 0,
        }
    }

    fn match_rules(
        item_names: &[&str],
        rules: Vec<RuleDraftInput>,
    ) -> Result<EkasaRuleMatches, AppError> {
        match_ekasa_rules(MatchEkasaRulesInput {
            item_names: item_names.iter().map(|name| name.to_string()).collect(),
            rules,
        })
    }

    #[test]
    fn the_wizard_gets_the_matching_row_of_each_item() {
        let matches = match_rules(
            &["KÁVA ZRNKOVÁ", "MLIEKO 1L", "ROZOK"],
            vec![draft("", 0), draft("kava", 10000), draft("mlieko|rozok", 0)],
        )
        .unwrap();

        let positions: Vec<Option<(usize, i64)>> = matches
            .items
            .iter()
            .map(|item| {
                item.matched_rule
                    .as_ref()
                    .map(|rule| (rule.position, rule.shares.deductible_pct_bps))
            })
            .collect();
        assert_eq!(positions, [Some((2, 10000)), Some((3, 0)), Some((3, 0))]);
        assert!(matches.invalid_rules.is_empty());
    }

    #[test]
    fn an_item_no_rule_matches_has_no_rule() {
        let matches = match_rules(&["PAPIER A4"], vec![draft("kava", 10000)]).unwrap();

        assert_eq!(matches.items[0].matched_rule, None);
    }

    #[test]
    fn the_wizard_hears_of_a_rule_that_does_not_compile() {
        let matches =
            match_rules(&["KAVA"], vec![draft("ka(va", 0), draft("kava", 10000)]).unwrap();

        assert_eq!(matches.items[0].matched_rule.as_ref().unwrap().position, 2);
        assert_eq!(matches.invalid_rules[0].position, 1);
        assert_eq!(matches.invalid_rules[0].pattern, "ka(va");
    }

    #[test]
    fn match_rules_refuses_shares_out_of_range() {
        let err = match_rules(&["KAVA"], vec![draft("kava", 10001)]).unwrap_err();

        assert_eq!(err.code, "VALIDATION");
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
