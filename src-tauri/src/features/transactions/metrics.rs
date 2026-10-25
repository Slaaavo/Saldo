use std::cmp::Reverse;
use std::collections::HashSet;

use crate::features::currency::models::Currency;
use crate::features::currency::rate_age::stale_after_days;

use super::models::{
    AccountKind, CountedLine, LinePart, LineSubject, Metric, MetricKind, Money, NotCountedReason,
    RateProblem, RateWarning, SkippedLine, SnapshotRow, Valuation,
};

enum Role {
    Add,
    Subtract,
    Skip(NotCountedReason),
}

fn role(kind: MetricKind, row: &SnapshotRow) -> Role {
    use AccountKind::{Account, Asset, Bucket};
    use MetricKind::{LeftToSpend, Liquid, NetWorth, TotalBalance};
    match (kind, row.account_type) {
        (NetWorth, Account | Asset) => Role::Add,
        (NetWorth | Liquid | TotalBalance, Bucket) => Role::Skip(NotCountedReason::Earmark),
        (Liquid | TotalBalance | LeftToSpend, Account) if row.is_linked_to_asset => {
            Role::Skip(NotCountedReason::LinkedToAsset)
        }
        (Liquid | TotalBalance | LeftToSpend, Account) => Role::Add,
        (LeftToSpend, Bucket) => Role::Subtract,
        (Liquid | TotalBalance | LeftToSpend, Asset) => Role::Skip(NotCountedReason::NotLiquid),
    }
}

struct Part<'a> {
    line: LinePart,
    native: Money,
    valuation: &'a Valuation,
}

fn parts(row: &SnapshotRow) -> Vec<Part<'_>> {
    let balance = Part {
        line: LinePart::Balance,
        native: Money {
            amount_minor: row.balance_minor,
            currency_code: row.currency_code.clone(),
            minor_units: row.currency_minor_units,
        },
        valuation: &row.valuation,
    };
    let links = row.bucket_links.iter().map(|valued| Part {
        line: LinePart::LinkedAccount {
            source_account_id: valued.link.source_account_id,
            source_account_name: valued.link.source_account_name.clone(),
        },
        native: Money {
            amount_minor: valued.balance_minor,
            currency_code: valued.link.source_currency_code.clone(),
            minor_units: valued.link.source_currency_minor_units,
        },
        valuation: &valued.valuation,
    });
    let tagged = row.tagged_cashflows.iter().map(|tagged| Part {
        line: LinePart::TaggedCashflows {
            source_account_id: tagged.source_account_id,
            source_account_name: tagged.source_account_name.clone(),
        },
        native: Money {
            amount_minor: tagged.amount_minor,
            currency_code: tagged.currency_code.clone(),
            minor_units: tagged.currency_minor_units,
        },
        valuation: &tagged.valuation,
    });
    std::iter::once(balance)
        .chain(links)
        .chain(tagged)
        .collect()
}

fn subject(row: &SnapshotRow, part: LinePart) -> LineSubject {
    LineSubject {
        account_id: row.account_id,
        account_name: row.account_name.clone(),
        account_type: row.account_type,
        part,
    }
}

pub fn counted_minor(row: &SnapshotRow) -> i64 {
    parts(row)
        .iter()
        .filter_map(|part| part.valuation.value_minor())
        .sum()
}

pub fn equity_minor(asset: &SnapshotRow, rows: &[SnapshotRow]) -> Option<i64> {
    if asset.account_type != AccountKind::Asset || asset.linked_asset_ids.is_empty() {
        return None;
    }
    let linked: Option<i64> = rows
        .iter()
        .filter(|row| asset.linked_asset_ids.contains(&row.account_id))
        .map(|row| row.valuation.value_minor())
        .sum();
    Some(asset.valuation.value_minor()? + linked?)
}

pub fn headline_metrics(rows: &[SnapshotRow], consolidation: &Currency) -> Vec<Metric> {
    shown(rows)
        .into_iter()
        .map(|kind| metric(kind, rows, consolidation))
        .collect()
}

fn shown(rows: &[SnapshotRow]) -> Vec<MetricKind> {
    let any = |kind| rows.iter().any(|row| row.account_type == kind);
    let mut kinds = if any(AccountKind::Asset) {
        vec![MetricKind::NetWorth, MetricKind::Liquid]
    } else {
        vec![MetricKind::TotalBalance]
    };
    if any(AccountKind::Bucket) {
        kinds.push(MetricKind::LeftToSpend);
    }
    kinds
}

fn metric(kind: MetricKind, rows: &[SnapshotRow], consolidation: &Currency) -> Metric {
    let mut counted = Vec::new();
    let mut skipped = Vec::new();
    for row in rows {
        let sign = match role(kind, row) {
            Role::Add => 1,
            Role::Subtract => -1,
            Role::Skip(reason) => {
                skipped.push(skipped_row(row, reason, consolidation));
                continue;
            }
        };
        for part in parts(row) {
            let subject = subject(row, part.line);
            let (value_minor, rate) = match part.valuation {
                Valuation::SameCurrency { value_minor } => (*value_minor, None),
                Valuation::Converted { value_minor, rate } => (*value_minor, Some(rate.clone())),
                Valuation::NoRate => {
                    skipped.push(SkippedLine {
                        subject,
                        amount: part.native,
                        reason: NotCountedReason::NoRate,
                    });
                    continue;
                }
            };
            counted.push(CountedLine {
                subject,
                native: part.native,
                rate,
                signed_minor: sign * value_minor,
            });
        }
    }
    Metric::new(kind, counted, skipped)
}

fn skipped_row(
    row: &SnapshotRow,
    reason: NotCountedReason,
    consolidation: &Currency,
) -> SkippedLine {
    let (part, amount) = match row.account_type {
        AccountKind::Bucket => (
            LinePart::BucketTotal,
            Money {
                amount_minor: counted_minor(row),
                currency_code: consolidation.code.clone(),
                minor_units: consolidation.minor_units,
            },
        ),
        AccountKind::Account | AccountKind::Asset => (
            LinePart::Balance,
            Money {
                amount_minor: row.balance_minor,
                currency_code: row.currency_code.clone(),
                minor_units: row.currency_minor_units,
            },
        ),
    };
    SkippedLine {
        subject: subject(row, part),
        amount,
        reason,
    }
}

struct RatedAmount<'a> {
    account_id: i64,
    account_name: &'a str,
    native: Money,
    valuation: &'a Valuation,
    is_custom: bool,
}

/// Bucket links never warn. A link values its source account's balance at the same rate, and
/// that account is always a row of its own, so the row warns for it.
pub fn rate_warnings(rows: &[SnapshotRow]) -> Vec<RateWarning> {
    let balances = rows.iter().map(|row| RatedAmount {
        account_id: row.account_id,
        account_name: &row.account_name,
        native: Money {
            amount_minor: row.balance_minor,
            currency_code: row.currency_code.clone(),
            minor_units: row.currency_minor_units,
        },
        valuation: &row.valuation,
        is_custom: row.is_custom,
    });
    let tagged = rows
        .iter()
        .flat_map(|row| &row.tagged_cashflows)
        .map(|tagged| RatedAmount {
            account_id: tagged.source_account_id,
            account_name: &tagged.source_account_name,
            native: Money {
                amount_minor: tagged.amount_minor,
                currency_code: tagged.currency_code.clone(),
                minor_units: tagged.currency_minor_units,
            },
            valuation: &tagged.valuation,
            is_custom: tagged.is_custom,
        });
    let mut warned = HashSet::new();
    let mut warnings: Vec<RateWarning> = balances
        .chain(tagged)
        .filter(|amount| amount.native.amount_minor != 0)
        .filter_map(|amount| {
            let problem = rate_problem(amount.valuation, amount.is_custom)?;
            warned.insert(amount.account_id).then(|| RateWarning {
                account_id: amount.account_id,
                account_name: amount.account_name.to_owned(),
                native: amount.native,
                problem,
            })
        })
        .collect();
    warnings.sort_by_key(|warning| match &warning.problem {
        RateProblem::NoRate => (0, Reverse(0)),
        RateProblem::Stale { rate, .. } => (1, Reverse(rate.age_days)),
    });
    warnings
}

fn rate_problem(valuation: &Valuation, is_custom: bool) -> Option<RateProblem> {
    match valuation {
        Valuation::NoRate => Some(RateProblem::NoRate),
        Valuation::Converted { rate, .. } if rate.stale => Some(RateProblem::Stale {
            rate: rate.clone(),
            limit_days: stale_after_days(is_custom),
        }),
        Valuation::Converted { .. } | Valuation::SameCurrency { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::buckets::BucketLink;
    use crate::features::transactions::models::{RateUsed, TaggedCashflows, ValuedBucketLink};
    use AccountKind::{Account, Asset, Bucket};
    use MetricKind::{LeftToSpend, Liquid, NetWorth, TotalBalance};

    fn eur_currency() -> Currency {
        Currency {
            id: 1,
            code: "EUR".to_owned(),
            name: "Euro".to_owned(),
            minor_units: 2,
            is_custom: false,
        }
    }

    fn money(amount_minor: i64, currency_code: &str) -> Money {
        Money {
            amount_minor,
            currency_code: currency_code.to_owned(),
            minor_units: 2,
        }
    }

    fn rate(age_days: i64, stale: bool) -> RateUsed {
        RateUsed {
            mantissa: 125,
            exponent: -2,
            is_direct: false,
            date: "2026-10-01".to_owned(),
            is_manual: false,
            age_days,
            stale,
        }
    }

    fn fresh(value_minor: i64) -> Valuation {
        Valuation::Converted {
            value_minor,
            rate: rate(0, false),
        }
    }

    fn row(
        account_id: i64,
        account_type: AccountKind,
        currency_code: &str,
        balance_minor: i64,
        valuation: Valuation,
    ) -> SnapshotRow {
        SnapshotRow {
            account_id,
            account_name: format!("Account {account_id}"),
            account_type,
            iban: None,
            balance_minor,
            currency_code: currency_code.to_owned(),
            currency_minor_units: 2,
            is_custom: false,
            valuation,
            is_linked_to_asset: false,
            linked_asset_ids: vec![],
            is_bucket_linked: false,
            bucket_links: vec![],
            tagged_cashflows: vec![],
            counted_minor: 0,
            equity_minor: None,
            person_id: None,
            purchase_price_minor: None,
            purchase_date: None,
            depreciation_period_months: None,
        }
    }

    fn eur(account_id: i64, account_type: AccountKind, balance_minor: i64) -> SnapshotRow {
        row(
            account_id,
            account_type,
            "EUR",
            balance_minor,
            Valuation::SameCurrency {
                value_minor: balance_minor,
            },
        )
    }

    fn link_from(source: &SnapshotRow) -> ValuedBucketLink {
        ValuedBucketLink {
            link: BucketLink {
                id: source.account_id * 100,
                event_id: 1,
                source_account_id: source.account_id,
                source_account_name: source.account_name.clone(),
                source_currency_id: 9,
                source_currency_code: source.currency_code.clone(),
                source_currency_minor_units: source.currency_minor_units,
            },
            balance_minor: source.balance_minor,
            valuation: source.valuation.clone(),
        }
    }

    fn tagged_from(
        source_account_id: i64,
        currency_code: &str,
        amount_minor: i64,
        valuation: Valuation,
    ) -> TaggedCashflows {
        TaggedCashflows {
            source_account_id,
            source_account_name: format!("Account {source_account_id}"),
            currency_code: currency_code.to_owned(),
            currency_minor_units: 2,
            is_custom: false,
            amount_minor,
            valuation,
        }
    }

    fn find(metrics: &[Metric], kind: MetricKind) -> &Metric {
        metrics.iter().find(|m| m.kind() == kind).unwrap()
    }

    fn counted_signed(metric: &Metric, account_id: i64) -> Vec<(LinePart, i64)> {
        metric
            .counted()
            .iter()
            .filter(|line| line.subject.account_id == account_id)
            .map(|line| (line.subject.part.clone(), line.signed_minor))
            .collect()
    }

    fn skipped_of(metric: &Metric, account_id: i64) -> Vec<(LinePart, Money, NotCountedReason)> {
        metric
            .skipped()
            .iter()
            .filter(|line| line.subject.account_id == account_id)
            .map(|line| (line.subject.part.clone(), line.amount.clone(), line.reason))
            .collect()
    }

    fn household() -> Vec<SnapshotRow> {
        let checking = eur(1, Account, 1_000);
        let usd = row(2, Account, "USD", 500, fresh(400));
        let mut mortgage = eur(3, Account, -300);
        mortgage.is_linked_to_asset = true;
        mortgage.linked_asset_ids = vec![5];
        let gbp = row(4, Account, "GBP", 70, Valuation::NoRate);
        let mut house = eur(5, Asset, 5_000);
        house.linked_asset_ids = vec![3];
        let mut bucket = eur(6, Bucket, 100);
        bucket.bucket_links = vec![link_from(&usd)];
        vec![checking, usd, mortgage, gbp, house, bucket]
    }

    #[test]
    fn shown_metrics_follow_the_kinds_of_rows_present() {
        let kinds = |rows: Vec<SnapshotRow>| -> Vec<MetricKind> {
            headline_metrics(&rows, &eur_currency())
                .iter()
                .map(Metric::kind)
                .collect()
        };
        assert_eq!(kinds(vec![eur(1, Account, 0)]), [TotalBalance]);
        assert_eq!(
            kinds(vec![eur(1, Account, 0), eur(2, Asset, 0)]),
            [NetWorth, Liquid]
        );
        assert_eq!(
            kinds(vec![eur(1, Account, 0), eur(2, Bucket, 0)]),
            [TotalBalance, LeftToSpend]
        );
        assert_eq!(
            kinds(vec![
                eur(1, Account, 0),
                eur(2, Asset, 0),
                eur(3, Bucket, 0)
            ]),
            [NetWorth, Liquid, LeftToSpend]
        );
    }

    #[test]
    fn every_metric_adds_up_from_its_counted_lines_and_lists_every_row() {
        let rows = household();
        let metrics = headline_metrics(&rows, &eur_currency());

        let totals: Vec<(MetricKind, i64)> = metrics
            .iter()
            .map(|m| (m.kind(), m.total_minor()))
            .collect();
        assert_eq!(
            totals,
            [(NetWorth, 6_100), (Liquid, 1_400), (LeftToSpend, 900)]
        );
        for metric in &metrics {
            let sum: i64 = metric.counted().iter().map(|line| line.signed_minor).sum();
            assert_eq!(metric.total_minor(), sum);
            let mut listed: Vec<i64> = metric
                .counted()
                .iter()
                .map(|line| line.subject.account_id)
                .chain(metric.skipped().iter().map(|line| line.subject.account_id))
                .collect();
            listed.sort();
            listed.dedup();
            assert_eq!(listed, [1, 2, 3, 4, 5, 6], "{:?}", metric.kind());
        }
    }

    #[test]
    fn a_counted_line_carries_its_native_amount_and_the_rate_used() {
        let metrics = headline_metrics(&household(), &eur_currency());
        let liquid = find(&metrics, Liquid);

        let usd_line = liquid
            .counted()
            .iter()
            .find(|line| line.subject.account_id == 2)
            .unwrap();
        assert_eq!(usd_line.native, money(500, "USD"));
        assert_eq!(usd_line.rate, Some(rate(0, false)));
        assert_eq!(usd_line.signed_minor, 400);
        let eur_line = liquid
            .counted()
            .iter()
            .find(|line| line.subject.account_id == 1)
            .unwrap();
        assert_eq!(eur_line.rate, None);
    }

    #[test]
    fn an_account_without_a_rate_is_skipped_from_every_metric_with_its_native_amount() {
        let metrics = headline_metrics(&household(), &eur_currency());

        for metric in &metrics {
            assert_eq!(counted_signed(metric, 4), []);
            assert_eq!(
                skipped_of(metric, 4),
                [(
                    LinePart::Balance,
                    money(70, "GBP"),
                    NotCountedReason::NoRate
                )]
            );
        }
    }

    #[test]
    fn an_asset_linked_account_counts_in_net_worth_and_is_skipped_from_liquid_and_left_to_spend() {
        let metrics = headline_metrics(&household(), &eur_currency());

        assert_eq!(
            counted_signed(find(&metrics, NetWorth), 3),
            [(LinePart::Balance, -300)]
        );
        for kind in [Liquid, LeftToSpend] {
            assert_eq!(
                skipped_of(find(&metrics, kind), 3),
                [(
                    LinePart::Balance,
                    money(-300, "EUR"),
                    NotCountedReason::LinkedToAsset
                )]
            );
        }
    }

    #[test]
    fn an_asset_counts_in_net_worth_and_is_skipped_from_liquid_as_not_liquid() {
        let metrics = headline_metrics(&household(), &eur_currency());

        assert_eq!(
            counted_signed(find(&metrics, NetWorth), 5),
            [(LinePart::Balance, 5_000)]
        );
        assert_eq!(
            skipped_of(find(&metrics, Liquid), 5),
            [(
                LinePart::Balance,
                money(5_000, "EUR"),
                NotCountedReason::NotLiquid
            )]
        );
    }

    #[test]
    fn buckets_are_skipped_as_earmarks_with_their_total_in_the_consolidation_currency() {
        let metrics = headline_metrics(&household(), &eur_currency());
        let earmark = [(
            LinePart::BucketTotal,
            money(500, "EUR"),
            NotCountedReason::Earmark,
        )];

        assert_eq!(skipped_of(find(&metrics, NetWorth), 6), earmark);
        assert_eq!(skipped_of(find(&metrics, Liquid), 6), earmark);
        let without_assets: Vec<SnapshotRow> = household()
            .into_iter()
            .filter(|row| row.account_type != Asset)
            .collect();
        let metrics = headline_metrics(&without_assets, &eur_currency());
        assert_eq!(skipped_of(find(&metrics, TotalBalance), 6), earmark);
    }

    #[test]
    fn left_to_spend_subtracts_each_bucket_part_and_skips_a_no_rate_part_with_its_native_amount() {
        let usd = row(2, Account, "USD", 500, fresh(400));
        let mut bucket = eur(6, Bucket, 100);
        bucket.bucket_links = vec![link_from(&usd)];
        bucket.tagged_cashflows = vec![tagged_from(7, "GBP", 50, Valuation::NoRate)];
        let rows = vec![eur(1, Account, 1_000), usd, bucket];

        let metrics = headline_metrics(&rows, &eur_currency());
        let left = find(&metrics, LeftToSpend);

        let linked = LinePart::LinkedAccount {
            source_account_id: 2,
            source_account_name: "Account 2".to_owned(),
        };
        assert_eq!(
            counted_signed(left, 6),
            [(LinePart::Balance, -100), (linked, -400)]
        );
        assert_eq!(
            skipped_of(left, 6),
            [(
                LinePart::TaggedCashflows {
                    source_account_id: 7,
                    source_account_name: "Account 7".to_owned(),
                },
                money(50, "GBP"),
                NotCountedReason::NoRate
            )]
        );
        assert_eq!(left.total_minor(), 1_000 + 400 - 100 - 400);
    }

    #[test]
    fn counted_minor_adds_the_valued_parts_and_leaves_out_unrated_ones() {
        let usd = row(2, Account, "USD", 500, fresh(400));
        let gbp = row(4, Account, "GBP", 70, Valuation::NoRate);
        let mut bucket = eur(6, Bucket, 100);
        bucket.bucket_links = vec![link_from(&usd), link_from(&gbp)];
        bucket.tagged_cashflows = vec![
            tagged_from(8, "CHF", 30, fresh(32)),
            tagged_from(7, "GBP", 50, Valuation::NoRate),
        ];

        assert_eq!(counted_minor(&bucket), 100 + 400 + 32);
        assert_eq!(counted_minor(&gbp), 0);
    }

    #[test]
    fn equity_adds_the_asset_and_its_linked_accounts() {
        let rows = household();

        assert_eq!(equity_minor(&rows[4], &rows), Some(5_000 - 300));
        assert_eq!(equity_minor(&rows[2], &rows), None);
    }

    #[test]
    fn equity_is_none_without_linked_accounts() {
        let house = eur(5, Asset, 5_000);

        assert_eq!(equity_minor(&house, std::slice::from_ref(&house)), None);
    }

    #[test]
    fn equity_is_none_when_a_linked_account_or_the_asset_has_no_rate() {
        let mut gbp_loan = row(3, Account, "GBP", -300, Valuation::NoRate);
        gbp_loan.linked_asset_ids = vec![5];
        let mut house = eur(5, Asset, 5_000);
        house.linked_asset_ids = vec![3];
        let rows = vec![gbp_loan, house];
        assert_eq!(equity_minor(&rows[1], &rows), None);

        let mut loan = eur(3, Account, -300);
        loan.linked_asset_ids = vec![5];
        let mut unpriced = row(5, Asset, "ETN", 10, Valuation::NoRate);
        unpriced.linked_asset_ids = vec![3];
        let rows = vec![loan, unpriced];
        assert_eq!(equity_minor(&rows[1], &rows), None);
    }

    #[test]
    fn a_missing_rate_warns_with_the_account_and_its_native_amount() {
        let warnings = rate_warnings(&household());

        assert_eq!(
            warnings,
            [RateWarning {
                account_id: 4,
                account_name: "Account 4".to_owned(),
                native: money(70, "GBP"),
                problem: RateProblem::NoRate,
            }]
        );
    }

    #[test]
    fn a_no_rate_account_linked_to_a_bucket_warns_once() {
        let gbp = row(4, Account, "GBP", 70, Valuation::NoRate);
        let mut bucket = eur(6, Bucket, 0);
        bucket.bucket_links = vec![link_from(&gbp)];

        let warnings = rate_warnings(&[gbp, bucket]);

        let accounts: Vec<i64> = warnings.iter().map(|w| w.account_id).collect();
        assert_eq!(accounts, [4]);
    }

    #[test]
    fn a_tagged_cashflow_warning_names_the_source_account() {
        let mut bucket = eur(6, Bucket, 0);
        bucket.tagged_cashflows = vec![tagged_from(9, "GBP", 50, Valuation::NoRate)];

        let warnings = rate_warnings(&[bucket]);

        assert_eq!(
            warnings,
            [RateWarning {
                account_id: 9,
                account_name: "Account 9".to_owned(),
                native: money(50, "GBP"),
                problem: RateProblem::NoRate,
            }]
        );
    }

    #[test]
    fn an_account_that_is_a_row_and_a_tagged_source_warns_once_with_its_own_balance() {
        let gbp = row(4, Account, "GBP", 70, Valuation::NoRate);
        let mut bucket = eur(6, Bucket, 0);
        bucket.tagged_cashflows = vec![tagged_from(4, "GBP", 50, Valuation::NoRate)];

        let warnings = rate_warnings(&[gbp, bucket]);

        let warned: Vec<(i64, i64)> = warnings
            .iter()
            .map(|w| (w.account_id, w.native.amount_minor))
            .collect();
        assert_eq!(warned, [(4, 70)]);
    }

    #[test]
    fn zero_amounts_raise_no_warning() {
        let empty_gbp = row(4, Account, "GBP", 0, Valuation::NoRate);
        let old_usd = row(
            2,
            Account,
            "USD",
            0,
            Valuation::Converted {
                value_minor: 0,
                rate: rate(30, true),
            },
        );
        let mut bucket = eur(6, Bucket, 0);
        bucket.tagged_cashflows = vec![tagged_from(7, "CHF", 0, Valuation::NoRate)];
        assert_eq!(rate_warnings(&[empty_gbp.clone(), old_usd, bucket]), []);

        let mut bucket = eur(6, Bucket, 0);
        bucket.tagged_cashflows = vec![tagged_from(4, "GBP", 50, Valuation::NoRate)];
        let warned: Vec<(i64, i64)> = rate_warnings(&[empty_gbp, bucket])
            .iter()
            .map(|w| (w.account_id, w.native.amount_minor))
            .collect();
        assert_eq!(warned, [(4, 50)]);
    }

    #[test]
    fn fresh_rates_and_the_consolidation_currency_raise_no_warning() {
        let rows = vec![
            eur(1, Account, 1_000),
            row(2, Account, "USD", 500, fresh(400)),
        ];

        assert_eq!(rate_warnings(&rows), []);
    }

    #[test]
    fn no_rate_warnings_come_first_then_stale_ones_from_the_oldest_rate() {
        let stale = |value_minor, age_days| Valuation::Converted {
            value_minor,
            rate: rate(age_days, true),
        };
        let rows = vec![
            row(1, Account, "USD", 100, stale(80, 10)),
            row(2, Account, "GBP", 100, Valuation::NoRate),
            row(3, Account, "CHF", 100, stale(90, 40)),
            row(4, Account, "JPY", 100, Valuation::NoRate),
        ];

        let order: Vec<i64> = rate_warnings(&rows).iter().map(|w| w.account_id).collect();

        assert_eq!(order, [2, 4, 3, 1]);
    }

    #[test]
    fn a_stale_warning_carries_the_limit_for_its_currency_kind() {
        let usd = row(
            1,
            Account,
            "USD",
            100,
            Valuation::Converted {
                value_minor: 80,
                rate: rate(9, true),
            },
        );
        let mut etn = row(
            2,
            Asset,
            "ETN",
            10,
            Valuation::Converted {
                value_minor: 1_250,
                rate: rate(40, true),
            },
        );
        etn.is_custom = true;

        let limits: Vec<(i64, RateProblem)> = rate_warnings(&[usd, etn])
            .into_iter()
            .map(|w| (w.account_id, w.problem))
            .collect();

        assert_eq!(
            limits,
            [
                (
                    2,
                    RateProblem::Stale {
                        rate: rate(40, true),
                        limit_days: 31
                    }
                ),
                (
                    1,
                    RateProblem::Stale {
                        rate: rate(9, true),
                        limit_days: 7
                    }
                ),
            ]
        );
    }

    #[test]
    fn the_wire_shape_tags_variants_and_flattens_line_subjects() {
        let metrics = headline_metrics(&household(), &eur_currency());
        let line = find(&metrics, Liquid)
            .counted()
            .iter()
            .find(|line| line.subject.account_id == 2)
            .unwrap();

        assert_eq!(
            serde_json::to_value(line).unwrap(),
            serde_json::json!({
                "accountId": 2,
                "accountName": "Account 2",
                "accountType": "account",
                "part": { "kind": "balance" },
                "native": { "amountMinor": 500, "currencyCode": "USD", "minorUnits": 2 },
                "rate": {
                    "mantissa": 125, "exponent": -2, "isDirect": false, "date": "2026-10-01",
                    "isManual": false, "ageDays": 0, "stale": false
                },
                "signedMinor": 400
            })
        );
        assert_eq!(
            serde_json::to_value(&household()[1].valuation).unwrap()["kind"],
            "converted"
        );
        assert_eq!(
            serde_json::to_value(RateProblem::Stale {
                rate: rate(9, true),
                limit_days: 7
            })
            .unwrap()["limitDays"],
            7
        );
    }
}
