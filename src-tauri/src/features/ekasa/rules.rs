use regex::{Regex, RegexBuilder};
use serde::Serialize;

use super::models::EkasaRuleRow;
use crate::shared::text::strip_accents;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shares {
    pub deductible_pct_bps: i64,
    pub vat_reclaimable_pct_bps: i64,
}

pub struct RuleSource<'a> {
    pub pattern: &'a str,
    pub shares: Shares,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleMatch {
    pub position: usize,
    pub pattern: String,
    #[serde(flatten)]
    pub shares: Shares,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleProblem {
    pub position: usize,
    pub pattern: String,
    pub message: String,
}

/// The one eKasa rule matcher. Rules apply in order and the first whose pattern matches wins.
/// A pattern is a case-insensitive regular expression matched anywhere in the item name, and
/// accents count for nothing on either side, so `kava` matches `KÁVA` and `káva` matches `KAVA`.
/// A blank pattern is skipped. A pattern that does not compile is skipped and reported, never an
/// error, so one bad rule of a stored profile does not stop the others.
pub struct RuleMatcher {
    patterns: Vec<(usize, Regex)>,
    problems: Vec<RuleProblem>,
}

impl RuleMatcher {
    /// Positions count every pattern given, blank ones too, so they name the wizard's rows.
    pub fn compile<'a>(patterns: impl IntoIterator<Item = &'a str>) -> Self {
        let mut compiled = Vec::new();
        let mut problems = Vec::new();
        for (index, pattern) in patterns.into_iter().enumerate() {
            if pattern.trim().is_empty() {
                continue;
            }
            let position = index + 1;
            match compile_pattern(pattern) {
                Ok(regex) => compiled.push((position, regex)),
                Err(e) => problems.push(RuleProblem {
                    position,
                    pattern: pattern.to_owned(),
                    message: one_line(&e),
                }),
            }
        }
        Self {
            patterns: compiled,
            problems,
        }
    }

    /// The position of the first rule whose pattern matches the item name.
    pub fn position_for(&self, item_name: &str) -> Option<usize> {
        let name = strip_accents(item_name);
        self.patterns
            .iter()
            .find(|(_, regex)| regex.is_match(&name))
            .map(|&(position, _)| position)
    }

    pub fn problems(&self) -> &[RuleProblem] {
        &self.problems
    }
}

/// A person's eKasa rules with the shares each one gives, as `saldo mcp` applies them.
pub struct ShareRules {
    matcher: RuleMatcher,
    rules: Vec<RuleMatch>,
}

impl ShareRules {
    pub fn compile<'a>(rules: impl IntoIterator<Item = RuleSource<'a>>) -> Self {
        let rules: Vec<RuleMatch> = rules
            .into_iter()
            .enumerate()
            .map(|(index, rule)| RuleMatch {
                position: index + 1,
                pattern: rule.pattern.to_owned(),
                shares: rule.shares,
            })
            .collect();
        Self {
            matcher: RuleMatcher::compile(rules.iter().map(|rule| rule.pattern.as_str())),
            rules,
        }
    }

    pub fn from_profile(rules: &[EkasaRuleRow]) -> Self {
        Self::compile(rules.iter().map(|rule| RuleSource {
            pattern: &rule.name_pattern,
            shares: Shares {
                deductible_pct_bps: rule.deductible_pct_bps,
                vat_reclaimable_pct_bps: rule.vat_reclaimable_pct_bps,
            },
        }))
    }

    pub fn shares_for(&self, item_name: &str) -> Option<&RuleMatch> {
        self.matcher
            .position_for(item_name)
            .map(|position| &self.rules[position - 1])
    }

    pub fn problems(&self) -> &[RuleProblem] {
        self.matcher.problems()
    }
}

pub fn pattern_problem(pattern: &str) -> Option<String> {
    compile_pattern(pattern).err().map(|e| one_line(&e))
}

/// The pattern loses its accents but is not lowercased: `\D`, `\W`, `\S` and `\B` would change
/// meaning. The case-insensitive flag covers case.
fn compile_pattern(pattern: &str) -> Result<Regex, regex::Error> {
    RegexBuilder::new(&strip_accents(pattern))
        .case_insensitive(true)
        .build()
}

/// A syntax error's text spans several lines with a caret under the pattern; its last line says
/// what is wrong.
fn one_line(error: &regex::Error) -> String {
    let text = error.to_string();
    let last = text
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(&text);
    last.trim().trim_start_matches("error: ").to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(pattern: &str, deductible_pct_bps: i64) -> RuleSource<'_> {
        RuleSource {
            pattern,
            shares: Shares {
                deductible_pct_bps,
                vat_reclaimable_pct_bps: deductible_pct_bps,
            },
        }
    }

    fn matched(rules: &ShareRules, name: &str) -> Option<(usize, i64)> {
        rules
            .shares_for(name)
            .map(|rule| (rule.position, rule.shares.deductible_pct_bps))
    }

    #[test]
    fn the_first_matching_rule_wins() {
        let rules = ShareRules::compile([rule("kava", 10000), rule("kava|caj", 5000)]);

        assert_eq!(matched(&rules, "KAVA ZRNKOVA"), Some((1, 10000)));
        assert_eq!(matched(&rules, "CAJ ZELENY"), Some((2, 5000)));
    }

    #[test]
    fn no_matching_rule_gives_nothing() {
        let rules = ShareRules::compile([rule("kava", 10000)]);

        assert_eq!(matched(&rules, "MLIEKO 1L"), None);
        assert_eq!(matched(&ShareRules::compile([]), "MLIEKO 1L"), None);
    }

    #[test]
    fn a_pattern_matches_anywhere_in_the_name_ignoring_case() {
        let rules = ShareRules::compile([rule("Lavazza", 10000)]);

        assert_eq!(matched(&rules, "KAVA LAVAZZA 1KG"), Some((1, 10000)));
        assert_eq!(matched(&rules, "kava lavazza"), Some((1, 10000)));
    }

    #[test]
    fn accents_count_for_nothing_on_either_side() {
        let plain = ShareRules::compile([rule("kava", 10000)]);
        let accented = ShareRules::compile([rule("káva", 10000)]);
        let upper = ShareRules::compile([rule("ČAJ", 10000)]);

        for name in ["KÁVA", "káva", "KAVA", "Káva zrnková"] {
            assert_eq!(matched(&plain, name), Some((1, 10000)), "{name}");
            assert_eq!(matched(&accented, name), Some((1, 10000)), "{name}");
        }
        assert_eq!(matched(&upper, "caj zeleny"), Some((1, 10000)));
        assert_eq!(matched(&upper, "Čaj"), Some((1, 10000)));
    }

    #[test]
    fn escapes_keep_their_meaning() {
        let rules = ShareRules::compile([rule(r"^\D+$", 10000), rule(r"\d+\s?kg", 5000)]);

        assert_eq!(matched(&rules, "ČAJ ZELENÝ"), Some((1, 10000)));
        assert_eq!(matched(&rules, "KAVA 1 KG"), Some((2, 5000)));
    }

    #[test]
    fn a_blank_pattern_is_skipped_but_keeps_its_position() {
        let rules = ShareRules::compile([rule("", 0), rule("  ", 0), rule("kava", 10000)]);

        assert_eq!(matched(&rules, "KAVA"), Some((3, 10000)));
        assert!(rules.problems().is_empty());
    }

    #[test]
    fn a_pattern_that_does_not_compile_is_skipped_and_reported() {
        let rules =
            ShareRules::compile([rule("kava(?=1kg)", 0), rule("caj(", 0), rule("kava", 10000)]);

        assert_eq!(matched(&rules, "KAVA1KG"), Some((3, 10000)));
        let reported: Vec<(usize, &str)> = rules
            .problems()
            .iter()
            .map(|p| (p.position, p.pattern.as_str()))
            .collect();
        assert_eq!(reported, [(1, "kava(?=1kg)"), (2, "caj(")]);
        assert!(rules.problems().iter().all(|p| !p.message.contains('\n')));
        assert!(rules.problems()[0].message.contains("look-around"));
    }

    #[test]
    fn a_stored_profile_matches_in_its_order() {
        let row = |sort_order: i64, pattern: &str, bps: i64| EkasaRuleRow {
            id: sort_order,
            profile_id: 1,
            sort_order,
            name_pattern: pattern.to_owned(),
            deductible_pct_bps: bps,
            vat_reclaimable_pct_bps: 0,
        };
        let rules = ShareRules::from_profile(&[row(0, "mlieko", 0), row(1, "mlieko|kava", 10000)]);

        assert_eq!(matched(&rules, "MLIEKO"), Some((1, 0)));
        assert_eq!(matched(&rules, "KAVA"), Some((2, 10000)));
    }

    #[test]
    fn pattern_problem_accepts_what_the_matcher_accepts() {
        assert_eq!(pattern_problem("káva|čaj"), None);
        assert_eq!(pattern_problem(r"\bcaj\b"), None);
        assert!(pattern_problem("(?<!x)y").is_some());
        assert!(pattern_problem("[a-").is_some());
    }
}
