use std::collections::BTreeMap;
use std::fmt;

use chrono::{Datelike, NaiveDate};
use serde::{Serialize, Serializer};

use crate::error::AppError;
use crate::shared::{div_round_i128, MAX_AMOUNT_MINOR};

const FULL_BPS: i64 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct YearMonth {
    year: i32,
    month: u8,
}

impl YearMonth {
    pub const FIRST_YEAR: i32 = 1990;
    pub const LAST_YEAR: i32 = 2200;

    /// Takes only a zero-padded `YYYY-MM` with a year from 1990 to 2200.
    pub fn parse(text: &str) -> Result<YearMonth, AppError> {
        let invalid = || validation(format!("'{text}' is not a month in YYYY-MM format."));
        let (year, month) = text.split_once('-').ok_or_else(invalid)?;
        let digits =
            |part: &str, len: usize| part.len() == len && part.bytes().all(|b| b.is_ascii_digit());
        if !digits(year, 4) || !digits(month, 2) {
            return Err(invalid());
        }
        let year: i32 = year.parse().map_err(|_| invalid())?;
        let month: u8 = month.parse().map_err(|_| invalid())?;
        if !(1..=12).contains(&month) || !(Self::FIRST_YEAR..=Self::LAST_YEAR).contains(&year) {
            return Err(invalid());
        }
        Ok(YearMonth { year, month })
    }

    pub fn of(date: NaiveDate) -> YearMonth {
        YearMonth {
            year: date.year(),
            month: date.month() as u8,
        }
    }

    pub fn first_of_year(year: i32) -> YearMonth {
        YearMonth { year, month: 1 }
    }

    pub fn last_of_year(year: i32) -> YearMonth {
        YearMonth { year, month: 12 }
    }

    pub fn next(self) -> YearMonth {
        if self.month == 12 {
            YearMonth {
                year: self.year + 1,
                month: 1,
            }
        } else {
            YearMonth {
                year: self.year,
                month: self.month + 1,
            }
        }
    }

    /// Every month from `from` to `to`, both included.
    pub fn range(from: YearMonth, to: YearMonth) -> impl Iterator<Item = YearMonth> {
        std::iter::successors((from <= to).then_some(from), move |month| {
            (*month < to).then(|| month.next())
        })
    }
}

impl fmt::Display for YearMonth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month)
    }
}

impl Serialize for YearMonth {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// One typed month. The fields are private and `new` checks what the table's CHECKs check, so a
/// cell that exists is valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetCell {
    month: YearMonth,
    budget_minor: Option<i64>,
    vat_bps: Option<i64>,
}

impl BudgetCell {
    pub fn new(
        month: YearMonth,
        budget_minor: Option<i64>,
        vat_bps: Option<i64>,
    ) -> Result<BudgetCell, AppError> {
        if budget_minor.is_none() && vat_bps.is_none() {
            return Err(validation(format!("{month}: type a budget or a VAT rate.")));
        }
        if budget_minor.is_some_and(|budget| budget < 0) {
            return Err(validation(format!(
                "{month}: the budget can't be negative."
            )));
        }
        if budget_minor.is_some_and(|budget| budget > MAX_AMOUNT_MINOR) {
            return Err(validation(format!("{month}: the budget is too large.")));
        }
        if vat_bps.is_some_and(|vat| !(0..=FULL_BPS).contains(&vat)) {
            return Err(validation(format!(
                "{month}: the VAT rate must be between 0 % and 100 %."
            )));
        }
        Ok(BudgetCell {
            month,
            budget_minor,
            vat_bps,
        })
    }

    pub fn month(&self) -> YearMonth {
        self.month
    }

    pub fn budget_minor(&self) -> Option<i64> {
        self.budget_minor
    }

    pub fn vat_bps(&self) -> Option<i64> {
        self.vat_bps
    }
}

/// What arrived in each month, VAT included.
pub type GrossInflows = BTreeMap<YearMonth, i64>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthLine {
    pub month: YearMonth,
    pub started: bool,
    pub budgeted: bool,
    pub received_minor: Option<i64>,
    pub difference_minor: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    pub value_minor: i64,
    pub lines: Vec<MonthLine>,
}

/// The one definition of an income budget's value: the sum of received minus budget over every
/// budgeted month that has started by `current`. `lines_for` only decides which months are
/// drawn, never the value.
pub fn evaluate(
    cells: &[BudgetCell],
    gross: &GrossInflows,
    current: YearMonth,
    lines_for: Option<(YearMonth, YearMonth)>,
) -> Evaluation {
    let by_month: BTreeMap<YearMonth, &BudgetCell> =
        cells.iter().map(|cell| (cell.month, cell)).collect();
    let line = |month: YearMonth| {
        let cell = by_month.get(&month);
        let budget = cell.and_then(|cell| cell.budget_minor);
        let vat = cell.and_then(|cell| cell.vat_bps).unwrap_or(0);
        let started = month <= current;
        let received = started.then(|| net_of_vat(gross.get(&month).copied().unwrap_or(0), vat));
        MonthLine {
            month,
            started,
            budgeted: budget.is_some(),
            received_minor: received,
            difference_minor: budget
                .zip(received)
                .map(|(budget, received)| received - budget),
        }
    };
    let value_minor = by_month
        .keys()
        .filter_map(|&month| line(month).difference_minor)
        .sum();
    let lines = lines_for
        .map(|(from, to)| YearMonth::range(from, to).map(line).collect())
        .unwrap_or_default();
    Evaluation { value_minor, lines }
}

/// The earliest month whose inflows `evaluate` reads, or None when it reads none.
pub fn first_month_needed(
    cells: &[BudgetCell],
    lines_for: Option<(YearMonth, YearMonth)>,
) -> Option<YearMonth> {
    cells
        .iter()
        .map(|cell| cell.month)
        .chain(lines_for.map(|(from, _)| from))
        .min()
}

/// Rounds half away from zero: gross * 10000 / (10000 + vat_bps), through i128.
pub fn net_of_vat(gross_minor: i64, vat_bps: i64) -> i64 {
    div_round_i128(
        i128::from(gross_minor) * i128::from(FULL_BPS),
        i128::from(FULL_BPS + vat_bps),
    ) as i64
}

fn validation(message: String) -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn month(text: &str) -> YearMonth {
        YearMonth::parse(text).unwrap()
    }

    fn cell(text: &str, budget_minor: Option<i64>, vat_bps: Option<i64>) -> BudgetCell {
        BudgetCell::new(month(text), budget_minor, vat_bps).unwrap()
    }

    fn inflows(entries: &[(&str, i64)]) -> GrossInflows {
        entries
            .iter()
            .map(|&(text, gross)| (month(text), gross))
            .collect()
    }

    fn value(cells: &[BudgetCell], gross: &GrossInflows, current: &str) -> i64 {
        evaluate(cells, gross, month(current), None).value_minor
    }

    #[test]
    fn the_spec_example_sums_to_minus_6315_on_2026_11_08() {
        let vat = Some(2_300);
        let cells = [
            cell("2026-07", Some(285_000), vat),
            cell("2026-08", Some(564_000), vat),
            cell("2026-09", Some(564_000), vat),
            cell("2026-10", Some(564_000), vat),
            cell("2026-11", Some(612_000), vat),
            cell("2026-12", Some(612_000), vat),
        ];
        let gross = inflows(&[
            ("2026-07", 350_550),
            ("2026-08", 662_601),
            ("2026-09", 674_163),
            ("2026-10", 720_411),
        ]);
        let current = YearMonth::of(NaiveDate::from_ymd_opt(2026, 11, 8).unwrap());

        let evaluation = evaluate(
            &cells,
            &gross,
            current,
            Some((month("2026-07"), month("2026-12"))),
        );

        let differences: Vec<Option<i64>> = evaluation
            .lines
            .iter()
            .map(|line| line.difference_minor)
            .collect();
        assert_eq!(
            differences,
            [
                Some(0),
                Some(-25_300),
                Some(-15_900),
                Some(21_700),
                Some(-612_000),
                None
            ]
        );
        assert_eq!(evaluation.value_minor, -631_500);
    }

    #[test]
    fn net_of_vat_rounds_half_away_from_zero() {
        assert_eq!(net_of_vat(12_300, 2_300), 10_000);
        assert_eq!(net_of_vat(10_000, 0), 10_000);
        assert_eq!(net_of_vat(3, 2_000), 3);
        assert_eq!(net_of_vat(1, 10_000), 1);
        assert_eq!(net_of_vat(-1, 10_000), -1);
        assert_eq!(net_of_vat(5, 10_000), 3);
        assert_eq!(net_of_vat(0, 2_300), 0);
    }

    #[test]
    fn a_month_with_no_budget_is_ignored_even_with_inflow() {
        let cells = [cell("2026-10", Some(100_000), None)];
        let gross = inflows(&[("2026-09", 500_000), ("2026-10", 100_000)]);

        assert_eq!(value(&cells, &gross, "2026-11"), 0);
    }

    #[test]
    fn a_typed_zero_budget_is_a_budget_of_zero() {
        let cells = [cell("2026-10", Some(0), Some(2_300))];
        let gross = inflows(&[("2026-10", 123_000)]);

        assert_eq!(value(&cells, &gross, "2026-10"), 100_000);
        assert_eq!(value(&cells, &GrossInflows::new(), "2026-10"), 0);
    }

    #[test]
    fn an_empty_vat_in_a_budgeted_month_is_zero_percent() {
        let cells = [cell("2026-10", Some(100_000), None)];
        let gross = inflows(&[("2026-10", 123_000)]);

        assert_eq!(value(&cells, &gross, "2026-10"), 23_000);
    }

    #[test]
    fn a_budgeted_month_counts_from_its_first_day_with_no_inflow() {
        let cells = [cell("2026-11", Some(612_000), Some(2_300))];
        let day = |d| YearMonth::of(NaiveDate::from_ymd_opt(2026, 11, d).unwrap());
        let october_31 = YearMonth::of(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap());

        assert_eq!(
            evaluate(&cells, &GrossInflows::new(), day(1), None).value_minor,
            -612_000
        );
        assert_eq!(
            evaluate(&cells, &GrossInflows::new(), october_31, None).value_minor,
            0
        );
    }

    #[test]
    fn a_future_month_does_not_count() {
        let cells = [
            cell("2026-11", Some(100_000), None),
            cell("2026-12", Some(100_000), None),
        ];
        let gross = inflows(&[("2026-12", 400_000)]);

        let evaluation = evaluate(
            &cells,
            &gross,
            month("2026-11"),
            Some((month("2026-12"), month("2026-12"))),
        );

        assert_eq!(evaluation.value_minor, -100_000);
        assert_eq!(
            evaluation.lines,
            [MonthLine {
                month: month("2026-12"),
                started: false,
                budgeted: true,
                received_minor: None,
                difference_minor: None,
            }]
        );
    }

    #[test]
    fn a_vat_only_month_shows_received_and_no_difference() {
        let cells = [cell("2026-10", None, Some(2_300))];
        let gross = inflows(&[("2026-10", 123_000)]);

        let evaluation = evaluate(
            &cells,
            &gross,
            month("2026-11"),
            Some((month("2026-10"), month("2026-10"))),
        );

        assert_eq!(evaluation.value_minor, 0);
        assert_eq!(
            evaluation.lines,
            [MonthLine {
                month: month("2026-10"),
                started: true,
                budgeted: false,
                received_minor: Some(100_000),
                difference_minor: None,
            }]
        );
    }

    #[test]
    fn lines_cover_the_span_and_never_change_the_value() {
        let cells = [
            cell("2025-12", Some(50_000), None),
            cell("2026-02", Some(10_000), None),
        ];
        let gross = inflows(&[("2026-01", 7_000), ("2026-02", 4_000)]);
        let current = month("2026-02");

        let without = evaluate(&cells, &gross, current, None);
        let one_year = evaluate(
            &cells,
            &gross,
            current,
            Some((month("2026-01"), month("2026-12"))),
        );

        assert_eq!(without.lines, []);
        assert_eq!(without.value_minor, -50_000 - 6_000);
        assert_eq!(one_year.value_minor, without.value_minor);
        let drawn: Vec<(String, Option<i64>, Option<i64>)> = one_year
            .lines
            .iter()
            .take(3)
            .map(|line| {
                (
                    line.month.to_string(),
                    line.received_minor,
                    line.difference_minor,
                )
            })
            .collect();
        assert_eq!(
            drawn,
            [
                ("2026-01".to_owned(), Some(7_000), None),
                ("2026-02".to_owned(), Some(4_000), Some(-6_000)),
                ("2026-03".to_owned(), None, None),
            ]
        );
        assert_eq!(one_year.lines.len(), 12);
    }

    #[test]
    fn first_month_needed_is_the_earliest_cell_or_drawn_month() {
        let cells = [
            cell("2026-03", Some(1), None),
            cell("2025-11", None, Some(0)),
        ];

        assert_eq!(first_month_needed(&cells, None), Some(month("2025-11")));
        assert_eq!(
            first_month_needed(&cells, Some((month("2025-01"), month("2025-12")))),
            Some(month("2025-01"))
        );
        assert_eq!(first_month_needed(&[], None), None);
    }

    #[test]
    fn budget_cell_refuses_bad_values() {
        let november = month("2026-11");
        let refused = [
            BudgetCell::new(november, None, None),
            BudgetCell::new(november, Some(-1), None),
            BudgetCell::new(november, Some(MAX_AMOUNT_MINOR + 1), None),
            BudgetCell::new(november, None, Some(-1)),
            BudgetCell::new(november, Some(0), Some(10_001)),
        ];

        for result in refused {
            let err = result.unwrap_err();
            assert_eq!(err.code, "VALIDATION");
            assert!(err.message.starts_with("2026-11"), "{}", err.message);
        }
        assert!(BudgetCell::new(november, Some(MAX_AMOUNT_MINOR), Some(10_000)).is_ok());
        assert!(BudgetCell::new(november, None, Some(0)).is_ok());
    }

    #[test]
    fn year_month_parse_refuses_unpadded_and_month_13() {
        for bad in [
            "2026-1",
            "2026-13",
            "2026-00",
            "26-11",
            "2026/11",
            "2026-11-01",
            "",
            "1989-12",
            "2201-01",
            "+026-11",
            "2026-1a",
        ] {
            assert_eq!(
                YearMonth::parse(bad).unwrap_err().code,
                "VALIDATION",
                "{bad}"
            );
        }
        assert_eq!(month("2026-01").to_string(), "2026-01");
        assert_eq!(
            serde_json::to_value(month("2026-11")).unwrap(),
            serde_json::json!("2026-11")
        );
    }

    #[test]
    fn a_range_runs_across_years_and_is_empty_when_reversed() {
        let months: Vec<String> = YearMonth::range(month("2025-11"), month("2026-02"))
            .map(|m| m.to_string())
            .collect();

        assert_eq!(months, ["2025-11", "2025-12", "2026-01", "2026-02"]);
        assert_eq!(
            YearMonth::range(month("2026-02"), month("2026-01")).count(),
            0
        );
    }
}
