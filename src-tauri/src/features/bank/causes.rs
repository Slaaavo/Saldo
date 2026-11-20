//! The likely causes of a gap between the bank app and Saldo: the rows whose one fix closes the
//! whole gap, searched among the rows added or changed since the account's newest check.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};

use chrono::NaiveDate;

use super::models::{DifferenceCause, RowTarget, SuspectRow};

const NEAR_COPY_DAYS: i64 = 5;
const STALE_IMPORT_DAYS: i64 = 3;
const MAX_PER_KIND: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowType {
    Cashflow,
    Transfer,
    UnexplainedDifference,
}

/// A row the live balance counts on the account.
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
    /// The later of the event's creation and its current version's, on the database clock.
    pub changed_at: String,
}

/// The import hint's facts, present only when the hint shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaleImport {
    pub newest_day: Option<String>,
}

/// The hint for an account imported from CSV before: shown when its newest cash flow or transfer
/// dated up to `today` is more than 3 days old, or it has none.
pub(crate) fn stale_import(newest_day: Option<String>, today: NaiveDate) -> Option<StaleImport> {
    let fresh = newest_day
        .as_deref()
        .and_then(day_of)
        .is_some_and(|day| (today - day).num_days() <= STALE_IMPORT_DAYS);
    (!fresh).then_some(StaleImport { newest_day })
}

/// The causes of a gap of `difference_minor` (the bank minus Saldo) among the account's counted
/// `rows`. A unit changed after `since` is a suspect; with no check at all, every unit is.
pub(crate) fn find_causes(
    difference_minor: i64,
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
    let closing_amount = match difference_minor.checked_neg() {
        Some(0) | None => return causes,
        Some(amount) => amount,
    };

    let units = units_of(rows);
    let is_suspect = |unit: &Unit| since.is_none_or(|since| unit.changed_at.as_str() > since);

    let mut near_copies: Vec<(&Unit, &Unit)> = Vec::new();
    let mut equals_gap: Vec<&Unit> = Vec::new();
    let mut listed = HashSet::new();
    for unit in units
        .iter()
        .filter(|unit| is_suspect(unit) && unit.row.amount_minor == closing_amount)
    {
        match closest_twin(&units, unit) {
            Some(twin) => {
                let (row, copy_of) = if is_suspect(twin) && changed_later(twin, unit) {
                    (twin, unit)
                } else {
                    (unit, twin)
                };
                if listed.insert(row.key) {
                    near_copies.push((row, copy_of));
                }
            }
            None => equals_gap.push(unit),
        }
    }
    let mut flipped: Vec<&Unit> = units
        .iter()
        .filter(|unit| {
            is_suspect(unit)
                && unit.flippable
                && unit.row.amount_minor.checked_mul(2) == Some(closing_amount)
        })
        .collect();

    near_copies.sort_by(|a, b| newest_first(a.0, b.0));
    flipped.sort_by(|a, b| newest_first(a, b));
    equals_gap.sort_by(|a, b| newest_first(a, b));

    causes.extend(
        near_copies
            .into_iter()
            .take(MAX_PER_KIND)
            .map(|(row, copy_of)| DifferenceCause::NearCopy {
                row: row.row.clone(),
                copy_of: copy_of.row.clone(),
            }),
    );
    causes.extend(flipped.into_iter().take(MAX_PER_KIND).map(|unit| {
        DifferenceCause::FlippedSign {
            row: unit.row.clone(),
        }
    }));
    causes.extend(equals_gap.into_iter().take(MAX_PER_KIND).map(|unit| {
        DifferenceCause::EqualsGap {
            row: unit.row.clone(),
        }
    }));
    causes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum UnitKey {
    Event(i64),
    SplitGroup(i64),
}

/// One bank line: a standalone row, or every counted row of a split group on the account.
struct Unit {
    key: UnitKey,
    row: SuspectRow,
    day: Option<NaiveDate>,
    changed_at: String,
    flippable: bool,
}

fn units_of(rows: Vec<CountedRow>) -> Vec<Unit> {
    let mut grouped: BTreeMap<UnitKey, Vec<CountedRow>> = BTreeMap::new();
    for row in rows {
        let key = match row.split_group_id {
            Some(split_group_id) => UnitKey::SplitGroup(split_group_id),
            None => UnitKey::Event(row.event_id),
        };
        grouped.entry(key).or_default().push(row);
    }
    grouped
        .into_iter()
        .map(|(key, rows)| unit_of(key, rows))
        .collect()
}

/// `rows` is never empty: every unit starts from a row.
fn unit_of(key: UnitKey, mut rows: Vec<CountedRow>) -> Unit {
    rows.sort_by_key(|row| row.event_id);
    let latest = |field: fn(&CountedRow) -> &str| {
        rows.iter().map(field).max().unwrap_or_default().to_owned()
    };
    let event_date = latest(|row| row.event_date.as_str());
    let changed_at = latest(|row| row.changed_at.as_str());
    let first = &rows[0];
    let target = match key {
        UnitKey::SplitGroup(split_group_id) => RowTarget::SplitGroup {
            split_group_id,
            parts: u32::try_from(rows.len()).unwrap_or(u32::MAX),
        },
        UnitKey::Event(event_id) => match first.flow {
            FlowType::Cashflow => RowTarget::Cashflow { event_id },
            FlowType::Transfer => RowTarget::Transfer { event_id },
            FlowType::UnexplainedDifference => RowTarget::UnexplainedDifference { event_id },
        },
    };
    Unit {
        key,
        day: day_of(&event_date),
        changed_at,
        flippable: matches!(target, RowTarget::Cashflow { .. })
            && !first.has_linked_event
            && !first.has_original_amount,
        row: SuspectRow {
            target,
            event_date,
            amount_minor: rows.iter().map(|row| row.amount_minor).sum(),
            note: first.note.clone(),
            counterpart_name: first.counterpart_name.clone(),
        },
    }
}

/// The counted unit other than `unit` with its amount and the closest date within 5 days of it.
fn closest_twin<'a>(units: &'a [Unit], unit: &Unit) -> Option<&'a Unit> {
    let day = unit.day?;
    units
        .iter()
        .filter(|other| other.key != unit.key && other.row.amount_minor == unit.row.amount_minor)
        .filter_map(|other| {
            let distance = (other.day? - day).num_days().abs();
            (distance <= NEAR_COPY_DAYS).then_some((distance, other))
        })
        .min_by(|(a_distance, a), (b_distance, b)| {
            a_distance.cmp(b_distance).then_with(|| changed_order(a, b))
        })
        .map(|(_, twin)| twin)
}

fn changed_order(a: &Unit, b: &Unit) -> Ordering {
    (&a.changed_at, a.key).cmp(&(&b.changed_at, b.key))
}

fn changed_later(a: &Unit, b: &Unit) -> bool {
    changed_order(a, b) == Ordering::Greater
}

fn newest_first(a: &Unit, b: &Unit) -> Ordering {
    b.row
        .event_date
        .cmp(&a.row.event_date)
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

    /// Saldo is 12.50 lower than the bank, so a wrong spending row of -12.50 explains it.
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

    fn suspect(target: RowTarget, amount_minor: i64, day: &str, note: &str) -> SuspectRow {
        SuspectRow {
            target,
            event_date: format!("{day}T12:00:00"),
            amount_minor,
            note: Some(note.to_owned()),
            counterpart_name: None,
        }
    }

    fn row(event_id: i64, amount_minor: i64, day: &str) -> SuspectRow {
        suspect(
            RowTarget::Cashflow { event_id },
            amount_minor,
            day,
            &format!("row {event_id}"),
        )
    }

    fn causes_of(difference_minor: i64, rows: Vec<CountedRow>) -> Vec<DifferenceCause> {
        find_causes(difference_minor, rows, Some(SINCE), None)
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
                row: row(1, -GAP, "2026-10-20")
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
                row: suspect(
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
    fn the_import_hint_comes_first_then_copies_flipped_signs_and_rows_equal_to_the_gap() {
        let rows = vec![
            cashflow(1, -2 * GAP, "2026-11-01", BEFORE),
            cashflow(2, -2 * GAP, "2026-11-03", AFTER),
            cashflow(3, -GAP, "2026-11-10", AFTER),
            cashflow(4, -2 * GAP, "2026-11-20", AFTER),
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
                DifferenceCause::NearCopy {
                    row: row(2, -2 * GAP, "2026-11-03"),
                    copy_of: row(1, -2 * GAP, "2026-11-01"),
                },
                DifferenceCause::FlippedSign {
                    row: row(3, -GAP, "2026-11-10")
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
