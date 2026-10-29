use super::models::{
    ActualMonths, EntryKind, EntryNotCountedReason, ForecastCell, ForecastFigures, ForecastLine,
    ForecastLineResult, TaxEventBreakdown, ValueSource,
};

pub(super) struct Forecast {
    pub lines: Vec<ForecastLineResult>,
    pub totals: LineTotals,
    pub entries: Vec<TaxEventBreakdown>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct LineTotals {
    pub revenue_minor: i64,
    pub deductible_minor: i64,
    pub non_deductible_minor: i64,
}

#[derive(Clone, Copy)]
enum LineRole {
    Revenue,
    DeductibleCost,
    NonDeductibleCost,
}

#[derive(Clone, Copy)]
enum Part {
    Net,
    Deductible,
    NonDeductible,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Share {
    Averaged,
    Fixed,
}

#[derive(Clone, Copy)]
enum SoFarRule {
    Entries,
    Schedule,
    Nothing,
}

#[derive(Clone, Copy)]
enum RestRule {
    Average,
    MonthlyAmount,
    Schedule,
}

#[derive(Clone, Copy)]
struct LineSpec {
    role: LineRole,
    feeds: &'static [(EntryKind, Part, Share)],
    so_far: SoFarRule,
    rest: RestRule,
}

impl LineSpec {
    const fn of(line: ForecastLine) -> LineSpec {
        match line {
            ForecastLine::Revenue => LineSpec {
                role: LineRole::Revenue,
                feeds: &[(EntryKind::Revenue, Part::Net, Share::Averaged)],
                so_far: SoFarRule::Entries,
                rest: RestRule::Average,
            },
            ForecastLine::Payroll => LineSpec {
                role: LineRole::DeductibleCost,
                feeds: &[],
                so_far: SoFarRule::Nothing,
                rest: RestRule::MonthlyAmount,
            },
            ForecastLine::Depreciation => LineSpec {
                role: LineRole::DeductibleCost,
                feeds: &[(EntryKind::Depreciation, Part::Deductible, Share::Fixed)],
                so_far: SoFarRule::Schedule,
                rest: RestRule::Schedule,
            },
            ForecastLine::DeductibleCosts => LineSpec {
                role: LineRole::DeductibleCost,
                feeds: &[
                    (EntryKind::Expense, Part::Deductible, Share::Averaged),
                    (EntryKind::Prepaid, Part::Deductible, Share::Fixed),
                ],
                so_far: SoFarRule::Entries,
                rest: RestRule::Average,
            },
            ForecastLine::NonDeductibleCosts => LineSpec {
                role: LineRole::NonDeductibleCost,
                feeds: &[
                    (EntryKind::Expense, Part::NonDeductible, Share::Averaged),
                    (
                        EntryKind::Depreciation,
                        Part::NonDeductible,
                        Share::Averaged,
                    ),
                    (EntryKind::Prepaid, Part::NonDeductible, Share::Fixed),
                ],
                so_far: SoFarRule::Entries,
                rest: RestRule::Average,
            },
        }
    }

    fn amount(&self, entry: &TaxEventBreakdown, share: Share) -> i64 {
        self.feeds
            .iter()
            .filter(|&&(kind, _, feed_share)| kind == entry.kind && feed_share == share)
            .map(|(_, part, _)| match part {
                Part::Net => entry.net_amount_minor,
                Part::Deductible => entry.tax_deductible_cost_minor,
                Part::NonDeductible => entry.non_tax_deductible_cost_minor,
            })
            .sum()
    }

    fn reads_entries(&self, period: Period, share: Share) -> bool {
        match period {
            Period::SoFar => !matches!(self.so_far, SoFarRule::Nothing),
            Period::Rest => share == Share::Fixed,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Period {
    SoFar,
    Rest,
}

impl Period {
    fn of(month: u8, actual_months: ActualMonths) -> Period {
        if actual_months.covers(month) {
            Period::SoFar
        } else {
            Period::Rest
        }
    }
}

pub(super) fn build(figures: &ForecastFigures, entries: Vec<(u8, TaxEventBreakdown)>) -> Forecast {
    let t = figures.actuals_through_month;
    let entries: Vec<(Period, TaxEventBreakdown)> = entries
        .into_iter()
        .map(|(month, entry)| (Period::of(month, t), entry))
        .collect();

    let lines: Vec<ForecastLineResult> = ForecastLine::ALL
        .into_iter()
        .map(|line| {
            let spec = LineSpec::of(line);
            let sum = |period: Period, share: Share| -> i64 {
                entries
                    .iter()
                    .filter(|(p, _)| *p == period)
                    .map(|(_, entry)| spec.amount(entry, share))
                    .sum()
            };
            let typed = figures.typed_values.get(&line).copied().unwrap_or_default();
            let averaged_so_far = sum(Period::SoFar, Share::Averaged);
            let fixed_rest = sum(Period::Rest, Share::Fixed);

            let so_far_entries = averaged_so_far + sum(Period::SoFar, Share::Fixed);
            let so_far_rule = match (t.get(), spec.so_far) {
                (0, _) | (_, SoFarRule::Nothing) => (0, ValueSource::Nothing),
                (_, SoFarRule::Entries) => (so_far_entries, ValueSource::Entries),
                (_, SoFarRule::Schedule) => (so_far_entries, ValueSource::Schedule),
            };
            let so_far = fill(typed.so_far_minor, so_far_rule);

            let rest_rule = match (t.months_left(), spec.rest) {
                (0, _) => (0, ValueSource::Nothing),
                (_, RestRule::Schedule) => (fixed_rest, ValueSource::Schedule),
                (left, RestRule::MonthlyAmount) => (
                    figures.payroll_monthly_minor * left,
                    ValueSource::MonthlyAmount,
                ),
                (_, RestRule::Average) => {
                    let base = typed.so_far_minor.unwrap_or(averaged_so_far);
                    match t.average_rest(base) {
                        Some(0) if fixed_rest != 0 => (fixed_rest, ValueSource::Schedule),
                        Some(average) => (average + fixed_rest, ValueSource::Average),
                        None if fixed_rest != 0 => (fixed_rest, ValueSource::Schedule),
                        None => (0, ValueSource::Nothing),
                    }
                }
            };
            let rest = fill(typed.rest_minor, rest_rule);

            ForecastLineResult {
                line,
                so_far,
                rest,
                year_minor: so_far.amount_minor + rest.amount_minor,
            }
        })
        .collect();

    let totals = lines
        .iter()
        .fold(LineTotals::default(), |mut totals, line| {
            let role_total = match LineSpec::of(line.line).role {
                LineRole::Revenue => &mut totals.revenue_minor,
                LineRole::DeductibleCost => &mut totals.deductible_minor,
                LineRole::NonDeductibleCost => &mut totals.non_deductible_minor,
            };
            *role_total += line.year_minor;
            totals
        });

    let entries = entries
        .into_iter()
        .map(|(period, entry)| TaxEventBreakdown {
            not_counted: not_counted_reason(&lines, period, &entry),
            ..entry
        })
        .collect();

    Forecast {
        lines,
        totals,
        entries,
    }
}

fn fill(typed: Option<i64>, (untyped_minor, source): (i64, ValueSource)) -> ForecastCell {
    match typed {
        Some(amount_minor) => ForecastCell {
            amount_minor,
            source: ValueSource::Typed,
            untyped_minor,
        },
        None => ForecastCell {
            amount_minor: untyped_minor,
            source,
            untyped_minor,
        },
    }
}

fn not_counted_reason(
    lines: &[ForecastLineResult],
    period: Period,
    entry: &TaxEventBreakdown,
) -> Option<EntryNotCountedReason> {
    let outcomes: Vec<Option<EntryNotCountedReason>> = lines
        .iter()
        .flat_map(|line| {
            let spec = LineSpec::of(line.line);
            let cell = match period {
                Period::SoFar => line.so_far,
                Period::Rest => line.rest,
            };
            [Share::Averaged, Share::Fixed]
                .into_iter()
                .filter(move |&share| spec.amount(entry, share) != 0)
                .map(
                    move |share| match (spec.reads_entries(period, share), cell.source) {
                        (false, _) => Some(EntryNotCountedReason::AfterActualMonths),
                        (true, ValueSource::Typed) => Some(EntryNotCountedReason::TypedValue),
                        (true, _) => None,
                    },
                )
        })
        .collect();
    if outcomes.iter().any(Option::is_none) {
        return None;
    }
    let after = Some(EntryNotCountedReason::AfterActualMonths);
    if outcomes.contains(&after) {
        after
    } else {
        outcomes.into_iter().flatten().next()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::features::tax_models::models::TypedPair;

    fn entry(
        kind: EntryKind,
        month: u8,
        net: i64,
        deductible: i64,
        non_deductible: i64,
    ) -> (u8, TaxEventBreakdown) {
        let breakdown = TaxEventBreakdown {
            event_id: i64::from(month),
            kind,
            event_date: format!("2026-{month:02}-15T00:00:00"),
            note: None,
            amount_minor: net,
            net_amount_minor: net,
            vat_amount_minor: 0,
            reclaimable_vat_minor: 0,
            non_reclaimable_vat_minor: 0,
            tax_deductible_cost_minor: deductible,
            non_tax_deductible_cost_minor: non_deductible,
            not_counted: None,
        };
        (month, breakdown)
    }

    fn revenue(month: u8, net: i64) -> (u8, TaxEventBreakdown) {
        entry(EntryKind::Revenue, month, net, 0, 0)
    }

    fn expense(month: u8, deductible: i64, non_deductible: i64) -> (u8, TaxEventBreakdown) {
        entry(
            EntryKind::Expense,
            month,
            deductible + non_deductible,
            deductible,
            non_deductible,
        )
    }

    fn prepaid(month: u8, deductible: i64, non_deductible: i64) -> (u8, TaxEventBreakdown) {
        entry(
            EntryKind::Prepaid,
            month,
            deductible + non_deductible,
            deductible,
            non_deductible,
        )
    }

    fn depreciation_schedule(monthly: i64, non_deductible: i64) -> Vec<(u8, TaxEventBreakdown)> {
        (1..=12)
            .map(|month| {
                entry(
                    EntryKind::Depreciation,
                    month,
                    monthly + non_deductible,
                    monthly,
                    non_deductible,
                )
            })
            .collect()
    }

    fn figures(
        month: i64,
        payroll_monthly_minor: i64,
        typed: &[(ForecastLine, Option<i64>, Option<i64>)],
    ) -> ForecastFigures {
        ForecastFigures {
            actuals_through_month: ActualMonths::new(month).unwrap(),
            payroll_monthly_minor,
            typed_values: typed
                .iter()
                .map(|&(line, so_far_minor, rest_minor)| {
                    (
                        line,
                        TypedPair {
                            so_far_minor,
                            rest_minor,
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>(),
            earlier_profits: vec![],
        }
    }

    fn line(forecast: &Forecast, line: ForecastLine) -> ForecastLineResult {
        *forecast
            .lines
            .iter()
            .find(|result| result.line == line)
            .unwrap()
    }

    fn cell(amount_minor: i64, source: ValueSource) -> ForecastCell {
        ForecastCell {
            amount_minor,
            source,
            untyped_minor: amount_minor,
        }
    }

    fn typed(amount_minor: i64, untyped_minor: i64) -> ForecastCell {
        ForecastCell {
            amount_minor,
            source: ValueSource::Typed,
            untyped_minor,
        }
    }

    fn reasons(forecast: &Forecast) -> Vec<Option<EntryNotCountedReason>> {
        forecast.entries.iter().map(|e| e.not_counted).collect()
    }

    #[test]
    fn lines_come_out_in_sheet_order() {
        let forecast = build(&ForecastFigures::default(), vec![]);
        let order: Vec<ForecastLine> = forecast.lines.iter().map(|l| l.line).collect();
        assert_eq!(order, ForecastLine::ALL);
    }

    #[test]
    fn no_actual_months_project_only_payroll_and_the_schedule() {
        let mut entries = vec![revenue(3, 100_000), expense(5, 20_000, 5_000)];
        entries.extend(depreciation_schedule(35_000, 0));
        let forecast = build(&figures(0, 400_000, &[]), entries);

        let nothing = cell(0, ValueSource::Nothing);
        for result in &forecast.lines {
            assert_eq!(result.so_far, nothing, "{:?} so far", result.line);
        }
        assert_eq!(line(&forecast, ForecastLine::Revenue).rest, nothing);
        assert_eq!(line(&forecast, ForecastLine::DeductibleCosts).rest, nothing);
        assert_eq!(
            line(&forecast, ForecastLine::NonDeductibleCosts).rest,
            nothing
        );
        assert_eq!(
            line(&forecast, ForecastLine::Payroll).rest,
            cell(4_800_000, ValueSource::MonthlyAmount)
        );
        assert_eq!(
            line(&forecast, ForecastLine::Depreciation).rest,
            cell(420_000, ValueSource::Schedule)
        );
        assert_eq!(
            forecast.totals,
            LineTotals {
                revenue_minor: 0,
                deductible_minor: 5_220_000,
                non_deductible_minor: 0,
            }
        );

        let after = Some(EntryNotCountedReason::AfterActualMonths);
        let mut expected = vec![after, after];
        expected.extend([None; 12]);
        assert_eq!(reasons(&forecast), expected);
    }

    #[test]
    fn whole_year_actual_leaves_every_rest_at_nothing() {
        let mut entries = vec![revenue(3, 100_000), expense(12, 20_000, 5_000)];
        entries.extend(depreciation_schedule(35_000, 0));
        let forecast = build(&figures(12, 400_000, &[]), entries);

        for result in &forecast.lines {
            assert_eq!(
                result.rest,
                cell(0, ValueSource::Nothing),
                "{:?} rest",
                result.line
            );
        }
        assert_eq!(
            line(&forecast, ForecastLine::Revenue).so_far,
            cell(100_000, ValueSource::Entries)
        );
        assert_eq!(
            line(&forecast, ForecastLine::Payroll).so_far,
            cell(0, ValueSource::Nothing)
        );
        assert_eq!(
            line(&forecast, ForecastLine::Depreciation).so_far,
            cell(420_000, ValueSource::Schedule)
        );
        assert_eq!(
            line(&forecast, ForecastLine::DeductibleCosts).so_far,
            cell(20_000, ValueSource::Entries)
        );
        assert_eq!(
            line(&forecast, ForecastLine::NonDeductibleCosts).so_far,
            cell(5_000, ValueSource::Entries)
        );
        assert!(reasons(&forecast).iter().all(Option::is_none));
    }

    #[test]
    fn typed_so_far_replaces_the_entries_and_drives_the_average() {
        let forecast = build(
            &figures(6, 0, &[(ForecastLine::Revenue, Some(120_000), None)]),
            vec![revenue(2, 60_000)],
        );
        let revenue_line = line(&forecast, ForecastLine::Revenue);
        assert_eq!(revenue_line.so_far, typed(120_000, 60_000));
        assert_eq!(revenue_line.rest, cell(120_000, ValueSource::Average));
        assert_eq!(revenue_line.year_minor, 240_000);
        assert_eq!(
            reasons(&forecast),
            vec![Some(EntryNotCountedReason::TypedValue)]
        );
    }

    #[test]
    fn typed_rest_replaces_the_average() {
        let forecast = build(
            &figures(6, 0, &[(ForecastLine::Revenue, None, Some(500_000))]),
            vec![revenue(2, 60_000)],
        );
        let revenue_line = line(&forecast, ForecastLine::Revenue);
        assert_eq!(revenue_line.so_far, cell(60_000, ValueSource::Entries));
        assert_eq!(revenue_line.rest, typed(500_000, 60_000));
        assert_eq!(revenue_line.year_minor, 560_000);
        assert_eq!(reasons(&forecast), vec![None]);
    }

    #[test]
    fn the_average_rounds_to_the_nearest_minor_unit() {
        let forecast = build(&figures(9, 0, &[]), vec![revenue(1, 200)]);
        assert_eq!(
            line(&forecast, ForecastLine::Revenue).rest,
            cell(67, ValueSource::Average)
        );
    }

    #[test]
    fn payroll_without_typed_so_far_is_nothing_and_projects_the_monthly_amount() {
        let forecast = build(&figures(9, 400_000, &[]), vec![]);
        let payroll = line(&forecast, ForecastLine::Payroll);
        assert_eq!(payroll.so_far, cell(0, ValueSource::Nothing));
        assert_eq!(payroll.rest, cell(1_200_000, ValueSource::MonthlyAmount));
        assert_eq!(forecast.totals.deductible_minor, 1_200_000);
    }

    #[test]
    fn depreciation_splits_by_month_and_stays_out_of_deductible_costs() {
        let forecast = build(&figures(9, 0, &[]), depreciation_schedule(35_000, 5_000));

        let depreciation = line(&forecast, ForecastLine::Depreciation);
        assert_eq!(depreciation.so_far, cell(315_000, ValueSource::Schedule));
        assert_eq!(depreciation.rest, cell(105_000, ValueSource::Schedule));
        assert_eq!(
            line(&forecast, ForecastLine::DeductibleCosts).so_far,
            cell(0, ValueSource::Entries)
        );
        let non_deductible = line(&forecast, ForecastLine::NonDeductibleCosts);
        assert_eq!(non_deductible.so_far, cell(45_000, ValueSource::Entries));
        assert_eq!(non_deductible.rest, cell(15_000, ValueSource::Average));
        assert!(reasons(&forecast).iter().all(Option::is_none));
    }

    #[test]
    fn an_expense_after_the_actual_months_does_not_count() {
        let forecast = build(
            &figures(9, 0, &[]),
            vec![expense(3, 90_000, 0), expense(10, 50_000, 0)],
        );
        let deductible = line(&forecast, ForecastLine::DeductibleCosts);
        assert_eq!(deductible.so_far, cell(90_000, ValueSource::Entries));
        assert_eq!(deductible.rest, cell(30_000, ValueSource::Average));
        assert_eq!(
            reasons(&forecast),
            vec![None, Some(EntryNotCountedReason::AfterActualMonths)]
        );
    }

    #[test]
    fn an_entry_counts_when_any_non_zero_part_lands_in_an_entries_cell() {
        let forecast = build(
            &figures(
                9,
                0,
                &[(ForecastLine::DeductibleCosts, Some(100_000), None)],
            ),
            vec![expense(3, 40_000, 0), expense(4, 40_000, 10_000)],
        );
        assert_eq!(
            reasons(&forecast),
            vec![Some(EntryNotCountedReason::TypedValue), None]
        );
    }

    #[test]
    fn a_depreciation_entry_under_a_typed_rest_reports_the_typed_value() {
        let forecast = build(
            &figures(9, 0, &[(ForecastLine::Depreciation, None, Some(0))]),
            depreciation_schedule(35_000, 0),
        );
        let typed = Some(EntryNotCountedReason::TypedValue);
        let mut expected = vec![None; 9];
        expected.extend([typed; 3]);
        assert_eq!(reasons(&forecast), expected);
    }

    #[test]
    fn a_prepaid_share_counts_once_and_is_never_averaged() {
        let forecast = build(&figures(3, 0, &[]), vec![prepaid(1, 1_200_00, 400_00)]);

        let deductible = line(&forecast, ForecastLine::DeductibleCosts);
        assert_eq!(deductible.so_far, cell(1_200_00, ValueSource::Entries));
        assert_eq!(deductible.rest, cell(0, ValueSource::Average));
        assert_eq!(deductible.year_minor, 1_200_00);
        let non_deductible = line(&forecast, ForecastLine::NonDeductibleCosts);
        assert_eq!(non_deductible.so_far, cell(400_00, ValueSource::Entries));
        assert_eq!(non_deductible.rest, cell(0, ValueSource::Average));
        assert_eq!(non_deductible.year_minor, 400_00);
        assert_eq!(reasons(&forecast), vec![None]);
    }

    #[test]
    fn only_the_expenses_beside_a_prepaid_share_are_averaged() {
        let forecast = build(
            &figures(3, 0, &[]),
            vec![prepaid(1, 1_200_00, 0), expense(2, 300_00, 0)],
        );
        let deductible = line(&forecast, ForecastLine::DeductibleCosts);
        assert_eq!(deductible.so_far, cell(1_500_00, ValueSource::Entries));
        assert_eq!(deductible.rest, cell(900_00, ValueSource::Average));
        assert_eq!(deductible.year_minor, 2_400_00);
    }

    #[test]
    fn a_prepaid_share_after_the_actual_months_counts_in_the_rest() {
        let forecast = build(&figures(9, 0, &[]), vec![prepaid(11, 1_200_00, 0)]);
        let deductible = line(&forecast, ForecastLine::DeductibleCosts);
        assert_eq!(deductible.so_far, cell(0, ValueSource::Entries));
        assert_eq!(deductible.rest, cell(1_200_00, ValueSource::Schedule));
        assert_eq!(deductible.year_minor, 1_200_00);
        assert_eq!(reasons(&forecast), vec![None]);
    }

    #[test]
    fn a_typed_so_far_already_holds_the_monthly_depreciation() {
        let forecast = build(
            &figures(
                6,
                0,
                &[(ForecastLine::NonDeductibleCosts, Some(30_000), None)],
            ),
            depreciation_schedule(35_000, 5_000),
        );
        let non_deductible = line(&forecast, ForecastLine::NonDeductibleCosts);
        assert_eq!(non_deductible.rest, cell(30_000, ValueSource::Average));
        assert_eq!(non_deductible.year_minor, 60_000);
    }

    #[test]
    fn a_prepaid_bought_after_a_typed_so_far_adds_to_the_average() {
        let forecast = build(
            &figures(
                9,
                0,
                &[(ForecastLine::DeductibleCosts, Some(9_000_00), None)],
            ),
            vec![prepaid(11, 1_200_00, 0)],
        );
        let deductible = line(&forecast, ForecastLine::DeductibleCosts);
        assert_eq!(deductible.rest, cell(4_200_00, ValueSource::Average));
        assert_eq!(deductible.year_minor, 13_200_00);
    }

    #[test]
    fn a_prepaid_share_under_a_typed_so_far_reports_the_typed_value() {
        let forecast = build(
            &figures(
                3,
                0,
                &[(ForecastLine::DeductibleCosts, Some(2_000_00), None)],
            ),
            vec![prepaid(1, 1_200_00, 0)],
        );
        assert_eq!(
            reasons(&forecast),
            vec![Some(EntryNotCountedReason::TypedValue)]
        );
    }

    #[test]
    fn with_no_actual_months_a_prepaid_share_is_the_rest_from_the_schedule() {
        let forecast = build(&figures(0, 0, &[]), vec![prepaid(1, 1_200_00, 0)]);
        let deductible = line(&forecast, ForecastLine::DeductibleCosts);
        assert_eq!(deductible.so_far, cell(0, ValueSource::Nothing));
        assert_eq!(deductible.rest, cell(1_200_00, ValueSource::Schedule));
        assert_eq!(reasons(&forecast), vec![None]);
    }

    #[test]
    fn a_zero_entry_has_no_reason() {
        let forecast = build(&figures(9, 0, &[]), vec![expense(11, 0, 0)]);
        assert_eq!(reasons(&forecast), vec![None]);
    }
}
