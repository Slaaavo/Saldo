use std::cmp::Ordering;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashSet};

use chrono::NaiveDate;

use super::models::{
    BankLine, CashflowTarget, DifferenceCause, RowTarget, UnexplainedDifferenceTarget,
};

const NEAR_COPY_DAYS: i64 = 5;
const STALE_IMPORT_DAYS: i64 = 3;
const MAX_PER_KIND: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowType {
    Cashflow,
    Transfer,
    UnexplainedDifference,
}

#[derive(Debug, Clone)]
pub(crate) struct CountedRow {
    pub event_id: i64,
    pub flow: FlowType,
    pub split_group_id: Option<i64>,
    pub has_linked_event: bool,
    pub has_original_amount: bool,
    pub amount_minor: i64,
    pub event_date: String,
    pub note: Option<String>,
    pub counterpart_name: Option<String>,
    pub changed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaleImport {
    pub newest_day: Option<String>,
}

pub(crate) fn stale_import(newest_day: Option<String>, today: NaiveDate) -> Option<StaleImport> {
    let fresh = newest_day
        .as_deref()
        .and_then(day_of)
        .is_some_and(|day| (today - day).num_days() <= STALE_IMPORT_DAYS);
    (!fresh).then_some(StaleImport { newest_day })
}

pub(crate) fn find_causes(
    bank_minus_saldo_minor: i64,
    rows: Vec<CountedRow>,
    since: Option<&str>,
    stale_import: Option<StaleImport>,
) -> Vec<DifferenceCause> {
    let mut causes: Vec<DifferenceCause> = stale_import
        .map(|hint| DifferenceCause::StaleImport {
            newest_day: hint.newest_day,
        })
        .into_iter()
        .collect();
    let closing_amount = match bank_minus_saldo_minor.checked_neg() {
        Some(0) | None => return causes,
        Some(amount) => amount,
    };

    let (booked, lines): (Vec<CountedLine>, Vec<CountedLine>) = counted_lines(rows)
        .into_iter()
        .partition(|counted| counted.booked_difference().is_some());
    let mut explained: Vec<(&CountedLine, BankLine<UnexplainedDifferenceTarget>)> = booked
        .iter()
        .filter(|counted| counted.line.amount_minor == closing_amount)
        .filter_map(|counted| Some((counted, counted.booked_difference()?)))
        .collect();
    let is_suspect =
        |counted: &CountedLine| since.is_none_or(|since| counted.changed_at.as_str() > since);

    let mut near_copies: Vec<(&CountedLine, &CountedLine)> = Vec::new();
    let mut equals_gap: Vec<&CountedLine> = Vec::new();
    let mut listed = HashSet::new();
    for counted in lines
        .iter()
        .filter(|counted| is_suspect(counted) && counted.line.amount_minor == closing_amount)
    {
        match closest_twin(&lines, counted) {
            Some(twin) => {
                let (row, copy_of) = if is_suspect(twin) && changed_later(twin, counted) {
                    (twin, counted)
                } else {
                    (counted, twin)
                };
                if listed.insert(row.key) {
                    near_copies.push((row, copy_of));
                }
            }
            None => equals_gap.push(counted),
        }
    }
    let mut flipped: Vec<(&CountedLine, BankLine<CashflowTarget>)> = lines
        .iter()
        .filter(|counted| {
            is_suspect(counted) && counted.line.amount_minor.checked_mul(2) == Some(closing_amount)
        })
        .filter_map(|counted| Some((counted, counted.flippable()?)))
        .collect();

    explained.sort_by(|a, b| newest_first(a.0, b.0));
    near_copies.sort_by(|a, b| newest_first(a.0, b.0));
    flipped.sort_by(|a, b| newest_first(a.0, b.0));
    equals_gap.sort_by(|a, b| newest_first(a, b));

    causes.extend(
        explained
            .into_iter()
            .take(MAX_PER_KIND)
            .map(|(_, row)| DifferenceCause::BookedDifferenceExplained { row }),
    );
    causes.extend(
        near_copies
            .into_iter()
            .take(MAX_PER_KIND)
            .map(|(row, copy_of)| DifferenceCause::NearCopy {
                row: row.line.clone(),
                copy_of: copy_of.line.clone(),
            }),
    );
    causes.extend(
        flipped
            .into_iter()
            .take(MAX_PER_KIND)
            .map(|(_, row)| DifferenceCause::FlippedSign { row }),
    );
    causes.extend(equals_gap.into_iter().take(MAX_PER_KIND).map(|counted| {
        DifferenceCause::EqualsGap {
            row: counted.line.clone(),
        }
    }));
    causes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum LineKey {
    Event(i64),
    SplitGroup(i64),
}

struct CountedLine {
    key: LineKey,
    line: BankLine,
    day: Option<NaiveDate>,
    changed_at: String,
    has_linked_event: bool,
    has_original_amount: bool,
}

impl CountedLine {
    fn flippable(&self) -> Option<BankLine<CashflowTarget>> {
        match self.line.target {
            RowTarget::Cashflow { event_id }
                if !self.has_linked_event && !self.has_original_amount =>
            {
                Some(self.line_with(CashflowTarget::Cashflow { event_id }))
            }
            _ => None,
        }
    }

    fn booked_difference(&self) -> Option<BankLine<UnexplainedDifferenceTarget>> {
        match self.line.target {
            RowTarget::UnexplainedDifference { event_id } => Some(
                self.line_with(UnexplainedDifferenceTarget::UnexplainedDifference { event_id }),
            ),
            _ => None,
        }
    }

    fn line_with<T>(&self, target: T) -> BankLine<T> {
        BankLine {
            target,
            event_date: self.line.event_date.clone(),
            amount_minor: self.line.amount_minor,
            note: self.line.note.clone(),
            counterpart_name: self.line.counterpart_name.clone(),
        }
    }
}

fn counted_lines(mut rows: Vec<CountedRow>) -> Vec<CountedLine> {
    rows.sort_by_key(|row| row.event_id);
    let mut grouped: BTreeMap<LineKey, (CountedRow, Vec<CountedRow>)> = BTreeMap::new();
    for row in rows {
        let key = match row.split_group_id {
            Some(split_group_id) => LineKey::SplitGroup(split_group_id),
            None => LineKey::Event(row.event_id),
        };
        match grouped.entry(key) {
            Entry::Vacant(group) => {
                group.insert((row, Vec::new()));
            }
            Entry::Occupied(mut group) => group.get_mut().1.push(row),
        }
    }
    grouped
        .into_iter()
        .map(|(key, (first, rest))| counted_line(key, first, rest))
        .collect()
}

fn counted_line(key: LineKey, first: CountedRow, rest: Vec<CountedRow>) -> CountedLine {
    let latest = |field: fn(&CountedRow) -> &str| {
        rest.iter()
            .map(field)
            .fold(field(&first), |a, b| a.max(b))
            .to_owned()
    };
    let event_date = latest(|row| row.event_date.as_str());
    let changed_at = latest(|row| row.changed_at.as_str());
    let amount_minor = first.amount_minor + rest.iter().map(|row| row.amount_minor).sum::<i64>();
    let target = match key {
        LineKey::SplitGroup(split_group_id) => RowTarget::SplitGroup {
            split_group_id,
            parts: u32::try_from(1 + rest.len()).unwrap_or(u32::MAX),
        },
        LineKey::Event(event_id) => match first.flow {
            FlowType::Cashflow => RowTarget::Cashflow { event_id },
            FlowType::Transfer => RowTarget::Transfer { event_id },
            FlowType::UnexplainedDifference => RowTarget::UnexplainedDifference { event_id },
        },
    };
    CountedLine {
        key,
        day: day_of(&event_date),
        changed_at,
        has_linked_event: first.has_linked_event,
        has_original_amount: first.has_original_amount,
        line: BankLine {
            target,
            event_date,
            amount_minor,
            note: first.note,
            counterpart_name: first.counterpart_name,
        },
    }
}

fn closest_twin<'a>(lines: &'a [CountedLine], counted: &CountedLine) -> Option<&'a CountedLine> {
    let day = counted.day?;
    lines
        .iter()
        .filter(|other| {
            other.key != counted.key && other.line.amount_minor == counted.line.amount_minor
        })
        .filter_map(|other| {
            let distance = (other.day? - day).num_days().abs();
            (distance <= NEAR_COPY_DAYS).then_some((distance, other))
        })
        .min_by(|(a_distance, a), (b_distance, b)| {
            a_distance.cmp(b_distance).then_with(|| changed_order(a, b))
        })
        .map(|(_, twin)| twin)
}

fn changed_order(a: &CountedLine, b: &CountedLine) -> Ordering {
    (&a.changed_at, a.key).cmp(&(&b.changed_at, b.key))
}

fn changed_later(a: &CountedLine, b: &CountedLine) -> bool {
    changed_order(a, b) == Ordering::Greater
}

fn newest_first(a: &CountedLine, b: &CountedLine) -> Ordering {
    b.line
        .event_date
        .cmp(&a.line.event_date)
        .then_with(|| changed_order(b, a))
}

fn day_of(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date.get(..10)?, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINCE: &str = "2026-11-14T09:00:00.000";
    const BEFORE: &str = "2026-11-14T08:00:00.000";
    const AFTER: &str = "2026-11-14T10:00:00.000";
    const LATER: &str = "2026-11-14T11:00:00.000";

    const GAP: i64 = 1_250;

    fn cashflow(event_id: i64, amount_minor: i64, day: &str, changed_at: &str) -> CountedRow {
        CountedRow {
            event_id,
            flow: FlowType::Cashflow,
            split_group_id: None,
            has_linked_event: false,
            has_original_amount: false,
            amount_minor,
            event_date: format!("{day}T12:00:00"),
            note: Some(format!("row {event_id}")),
            counterpart_name: None,
            changed_at: changed_at.to_owned(),
        }
    }

    fn booked(event_id: i64, amount_minor: i64, day: &str, changed_at: &str) -> CountedRow {
        CountedRow {
            flow: FlowType::UnexplainedDifference,
            note: None,
            ..cashflow(event_id, amount_minor, day, changed_at)
        }
    }

    fn explained(event_id: i64, amount_minor: i64, day: &str) -> DifferenceCause {
        DifferenceCause::BookedDifferenceExplained {
            row: BankLine {
                note: None,
                ..bank_line(
                    UnexplainedDifferenceTarget::UnexplainedDifference { event_id },
                    amount_minor,
                    day,
                    "",
                )
            },
        }
    }

    fn bank_line<T>(target: T, amount_minor: i64, day: &str, note: &str) -> BankLine<T> {
        BankLine {
            target,
            event_date: format!("{day}T12:00:00"),
            amount_minor,
            note: Some(note.to_owned()),
            counterpart_name: None,
        }
    }

    fn row(event_id: i64, amount_minor: i64, day: &str) -> BankLine {
        bank_line(
            RowTarget::Cashflow { event_id },
            amount_minor,
            day,
            &format!("row {event_id}"),
        )
    }

    fn flippable_row(event_id: i64, amount_minor: i64, day: &str) -> BankLine<CashflowTarget> {
        bank_line(
            CashflowTarget::Cashflow { event_id },
            amount_minor,
            day,
            &format!("row {event_id}"),
        )
    }

    fn causes_of(bank_minus_saldo_minor: i64, rows: Vec<CountedRow>) -> Vec<DifferenceCause> {
        find_causes(bank_minus_saldo_minor, rows, Some(SINCE), None)
    }

    #[test]
    fn a_new_row_equal_to_the_gap_is_listed_for_deleting() {
        let rows = vec![
            cashflow(1, -GAP, "2026-11-12", AFTER),
            cashflow(2, -3_000, "2026-11-12", AFTER),
        ];

        assert_eq!(
            causes_of(GAP, rows),
            vec![DifferenceCause::EqualsGap {
                row: row(1, -GAP, "2026-11-12")
            }]
        );
    }

    #[test]
    fn a_row_with_a_twin_within_5_days_is_a_near_copy_and_at_6_days_is_not() {
        let at_5_days = vec![
            cashflow(1, -GAP, "2026-11-07", AFTER),
            cashflow(2, -GAP, "2026-11-12", LATER),
        ];
        let at_6_days = vec![
            cashflow(1, -GAP, "2026-11-06", BEFORE),
            cashflow(2, -GAP, "2026-11-12", AFTER),
        ];

        assert_eq!(
            causes_of(GAP, at_5_days),
            vec![DifferenceCause::NearCopy {
                row: row(2, -GAP, "2026-11-12"),
                copy_of: row(1, -GAP, "2026-11-07"),
            }]
        );
        assert_eq!(
            causes_of(GAP, at_6_days),
            vec![DifferenceCause::EqualsGap {
                row: row(2, -GAP, "2026-11-12")
            }]
        );
    }

    #[test]
    fn two_new_copies_give_one_cause_that_deletes_the_later_changed_one() {
        let rows = vec![
            cashflow(1, -GAP, "2026-11-12", LATER),
            cashflow(2, -GAP, "2026-11-12", AFTER),
        ];

        assert_eq!(
            causes_of(GAP, rows),
            vec![DifferenceCause::NearCopy {
                row: row(1, -GAP, "2026-11-12"),
                copy_of: row(2, -GAP, "2026-11-12"),
            }]
        );
    }

    #[test]
    fn a_twin_older_than_the_newest_check_still_makes_a_near_copy() {
        let rows = vec![
            cashflow(1, -GAP, "2026-11-10", BEFORE),
            cashflow(2, -GAP, "2026-11-12", AFTER),
        ];

        assert_eq!(
            causes_of(GAP, rows),
            vec![DifferenceCause::NearCopy {
                row: row(2, -GAP, "2026-11-12"),
                copy_of: row(1, -GAP, "2026-11-10"),
            }]
        );
    }

    #[test]
    fn with_several_twins_the_copy_names_the_closest_in_date() {
        let rows = vec![
            cashflow(1, -GAP, "2026-11-08", BEFORE),
            cashflow(2, -GAP, "2026-11-11", BEFORE),
            cashflow(3, -GAP, "2026-11-12", AFTER),
        ];

        assert_eq!(
            causes_of(GAP, rows),
            vec![DifferenceCause::NearCopy {
                row: row(3, -GAP, "2026-11-12"),
                copy_of: row(2, -GAP, "2026-11-11"),
            }]
        );
    }

    #[test]
    fn only_a_plain_cash_flow_can_have_its_sign_flipped() {
        let split_leg = CountedRow {
            split_group_id: Some(7),
            ..cashflow(2, -GAP, "2026-11-01", AFTER)
        };
        let transfer = CountedRow {
            flow: FlowType::Transfer,
            has_linked_event: true,
            ..cashflow(3, -GAP, "2026-11-10", AFTER)
        };
        let foreign = CountedRow {
            has_original_amount: true,
            ..cashflow(4, -GAP, "2026-11-20", AFTER)
        };
        let rows = vec![
            cashflow(1, -GAP, "2026-10-20", AFTER),
            split_leg,
            transfer,
            foreign,
        ];

        assert_eq!(
            causes_of(2 * GAP, rows),
            vec![DifferenceCause::FlippedSign {
                row: flippable_row(1, -GAP, "2026-10-20")
            }]
        );
    }

    #[test]
    fn a_row_last_changed_before_the_newest_check_is_not_a_suspect_until_it_is_edited() {
        let unchanged = vec![cashflow(1, -GAP, "2026-11-12", BEFORE)];
        let edited = vec![cashflow(1, -GAP, "2026-11-12", AFTER)];

        assert_eq!(causes_of(GAP, unchanged), vec![]);
        assert_eq!(
            causes_of(GAP, edited),
            vec![DifferenceCause::EqualsGap {
                row: row(1, -GAP, "2026-11-12")
            }]
        );
    }

    #[test]
    fn with_no_check_every_counted_row_is_a_suspect() {
        let rows = vec![cashflow(1, -GAP, "2020-01-01", BEFORE)];

        assert_eq!(
            find_causes(GAP, rows, None, None),
            vec![DifferenceCause::EqualsGap {
                row: row(1, -GAP, "2020-01-01")
            }]
        );
    }

    #[test]
    fn a_split_group_is_one_bank_line_with_the_sum_of_its_rows() {
        let leg = |event_id, amount_minor| CountedRow {
            split_group_id: Some(7),
            ..cashflow(event_id, amount_minor, "2026-11-12", AFTER)
        };

        assert_eq!(
            causes_of(GAP, vec![leg(1, -1_000), leg(2, -250)]),
            vec![DifferenceCause::EqualsGap {
                row: bank_line(
                    RowTarget::SplitGroup {
                        split_group_id: 7,
                        parts: 2
                    },
                    -GAP,
                    "2026-11-12",
                    "row 1"
                )
            }]
        );
    }

    #[test]
    fn at_most_5_causes_of_a_kind_are_listed_newest_first() {
        let days = [
            "2026-01-01",
            "2026-01-08",
            "2026-01-15",
            "2026-01-22",
            "2026-01-29",
            "2026-02-05",
            "2026-02-12",
        ];
        let rows = days
            .iter()
            .zip(1..)
            .map(|(day, id)| cashflow(id, -GAP, day, AFTER))
            .collect();

        let listed: Vec<String> = causes_of(GAP, rows)
            .into_iter()
            .map(|cause| match cause {
                DifferenceCause::EqualsGap { row } => row.event_date,
                other => panic!("unexpected {other:?}"),
            })
            .collect();

        assert_eq!(
            listed,
            [
                "2026-02-12",
                "2026-02-05",
                "2026-01-29",
                "2026-01-22",
                "2026-01-15"
            ]
            .map(|day| format!("{day}T12:00:00"))
        );
    }

    #[test]
    fn a_booked_difference_that_closes_the_gap_is_listed_even_when_booked_before_the_check() {
        let rows = vec![
            booked(1, -GAP, "2026-11-12", BEFORE),
            cashflow(2, -GAP, "2026-11-11", AFTER),
        ];

        assert_eq!(
            causes_of(GAP, rows),
            vec![
                explained(1, -GAP, "2026-11-12"),
                DifferenceCause::EqualsGap {
                    row: row(2, -GAP, "2026-11-11")
                },
            ]
        );
    }

    #[test]
    fn a_booked_difference_of_another_amount_is_not_listed() {
        let rows = vec![booked(1, -3_000, "2026-11-12", AFTER)];

        assert_eq!(causes_of(GAP, rows), vec![]);
    }

    #[test]
    fn an_unexplained_difference_is_never_a_copy_a_flipped_sign_or_a_row_equal_to_the_gap() {
        let rows = vec![
            booked(1, -GAP, "2026-11-12", AFTER),
            booked(2, -2 * GAP, "2026-11-12", AFTER),
            booked(3, -2 * GAP, "2026-11-10", AFTER),
        ];

        assert_eq!(
            causes_of(2 * GAP, rows),
            vec![
                explained(2, -2 * GAP, "2026-11-12"),
                explained(3, -2 * GAP, "2026-11-10"),
            ]
        );
    }

    #[test]
    fn the_hint_comes_first_then_booked_differences_copies_flipped_signs_and_rows_equal_to_the_gap()
    {
        let rows = vec![
            cashflow(1, -2 * GAP, "2026-11-01", BEFORE),
            cashflow(2, -2 * GAP, "2026-11-03", AFTER),
            cashflow(3, -GAP, "2026-11-10", AFTER),
            cashflow(4, -2 * GAP, "2026-11-20", AFTER),
            booked(5, -2 * GAP, "2026-10-01", BEFORE),
        ];
        let hint = StaleImport {
            newest_day: Some("2026-11-12".to_owned()),
        };

        assert_eq!(
            find_causes(2 * GAP, rows, Some(SINCE), Some(hint)),
            vec![
                DifferenceCause::StaleImport {
                    newest_day: Some("2026-11-12".to_owned())
                },
                explained(5, -2 * GAP, "2026-10-01"),
                DifferenceCause::NearCopy {
                    row: row(2, -2 * GAP, "2026-11-03"),
                    copy_of: row(1, -2 * GAP, "2026-11-01"),
                },
                DifferenceCause::FlippedSign {
                    row: flippable_row(3, -GAP, "2026-11-10")
                },
                DifferenceCause::EqualsGap {
                    row: row(4, -2 * GAP, "2026-11-20")
                },
            ]
        );
    }

    #[test]
    fn with_no_gap_only_the_import_hint_is_listed() {
        let rows = vec![cashflow(1, 0, "2026-11-12", AFTER)];
        let hint = StaleImport { newest_day: None };

        assert_eq!(
            find_causes(0, rows, Some(SINCE), Some(hint)),
            vec![DifferenceCause::StaleImport { newest_day: None }]
        );
    }

    #[test]
    fn the_import_hint_shows_when_the_newest_row_is_more_than_3_days_old_or_missing() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        let hint = |newest_day: Option<&str>| stale_import(newest_day.map(str::to_owned), today);

        assert_eq!(
            hint(Some("2026-11-16")),
            Some(StaleImport {
                newest_day: Some("2026-11-16".to_owned())
            })
        );
        assert_eq!(hint(Some("2026-11-17")), None);
        assert_eq!(hint(Some("2026-11-20")), None);
        assert_eq!(hint(None), Some(StaleImport { newest_day: None }));
    }
}
