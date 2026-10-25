use chrono::NaiveDate;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use rusqlite::{params, Connection};
use std::collections::HashMap;

use crate::error::AppError;
use crate::features::assets::repository::get_all_account_asset_link_ids;
use crate::features::buckets::repository::list_all_latest_bucket_links;
use crate::features::currency::models::Currency;
use crate::features::currency::rate_age::{is_stale, rate_age_days};
use crate::features::currency::repository::{
    get_consolidation_currency, get_fx_rate_for_conversion, GetFxRateForConversionParams,
};
use crate::features::transactions::metrics;
use crate::features::transactions::models::{
    AccountKind, RateUsed, Snapshot, SnapshotRow, TaggedCashflows, Valuation, ValuedBucketLink,
};
use crate::shared::{convert_balance, is_iso_date, validate_event_date};

// Validated dates always compare below it, so the live cutoff keeps every row and every rate.
const LIVE_EVENTS_UNTIL: &str = "9999-12-31T23:59:59";

pub struct Moment {
    events_until: String,
    age_reference: NaiveDate,
}

impl Moment {
    /// Everything Saldo knows. Rows and rates dated after today count, and rates age from today.
    pub fn live(today: NaiveDate) -> Moment {
        Moment {
            events_until: LIVE_EVENTS_UNTIL.to_owned(),
            age_reference: today,
        }
    }

    pub fn at(until: &str) -> Result<Moment, AppError> {
        validate_event_date(until)?;
        let events_until = if is_iso_date(until) {
            format!("{until}T23:59:59")
        } else {
            until.to_owned()
        };
        let age_reference = NaiveDate::parse_from_str(&events_until[..10], "%Y-%m-%d")
            .map_err(|e| AppError::from(e.to_string()))?;
        Ok(Moment {
            events_until,
            age_reference,
        })
    }

    fn rates_until(&self) -> &str {
        &self.events_until[..10]
    }
}

pub struct GetSnapshotParams {
    pub moment: Moment,
    pub person_id: Option<i64>,
}

impl FromSql for AccountKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "account" => Ok(AccountKind::Account),
            "bucket" => Ok(AccountKind::Bucket),
            "asset" => Ok(AccountKind::Asset),
            other => Err(FromSqlError::Other(
                format!("not a snapshot account type: {other}").into(),
            )),
        }
    }
}

#[derive(Clone, Copy)]
struct Held {
    currency_id: i64,
    minor_units: i64,
    is_custom: bool,
}

struct ConsolidationConverter<'a> {
    conn: &'a Connection,
    consolidation: &'a Currency,
    moment: &'a Moment,
    rates: HashMap<i64, Option<RateUsed>>,
}

impl ConsolidationConverter<'_> {
    fn value(&mut self, amount_minor: i64, held: Held) -> rusqlite::Result<Valuation> {
        if held.currency_id == self.consolidation.id {
            return Ok(Valuation::SameCurrency {
                value_minor: amount_minor,
            });
        }
        let rate = match self.rates.get(&held.currency_id) {
            Some(cached) => cached.clone(),
            None => {
                let rate = self.look_up(held)?;
                self.rates.insert(held.currency_id, rate.clone());
                rate
            }
        };
        Ok(match rate {
            Some(rate) => Valuation::Converted {
                value_minor: convert_balance(
                    amount_minor,
                    rate.mantissa,
                    rate.exponent,
                    held.minor_units,
                    self.consolidation.minor_units,
                    rate.is_direct,
                ),
                rate,
            },
            None => Valuation::NoRate,
        })
    }

    fn look_up(&self, held: Held) -> rusqlite::Result<Option<RateUsed>> {
        let stored = get_fx_rate_for_conversion(
            self.conn,
            GetFxRateForConversionParams {
                from_currency_id: self.consolidation.id,
                to_currency_id: held.currency_id,
                date: self.moment.rates_until().to_owned(),
            },
        )?;
        Ok(stored.map(|stored| {
            let age_days = rate_age_days(stored.date, self.moment.age_reference);
            RateUsed {
                mantissa: stored.mantissa,
                exponent: stored.exponent,
                is_direct: stored.is_direct,
                date: stored.date.format("%Y-%m-%d").to_string(),
                is_manual: stored.is_manual,
                age_days,
                stale: is_stale(age_days, held.is_custom),
            }
        }))
    }
}

pub fn get_accounts_snapshot(
    conn: &Connection,
    params: GetSnapshotParams,
) -> rusqlite::Result<Snapshot> {
    let GetSnapshotParams { moment, person_id } = params;
    let events_until = moment.events_until.as_str();
    let consolidation = get_consolidation_currency(conn)?;
    let mut converter = ConsolidationConverter {
        conn,
        consolidation: &consolidation,
        moment: &moment,
        rates: HashMap::new(),
    };

    let mut stmt = conn.prepare(
        "SELECT
           a.id AS account_id,
           a.name AS account_name,
           a.account_type,
           a.iban,
           c.id AS currency_id,
           c.code AS currency_code,
           c.minor_units AS currency_minor_units,
           c.is_custom AS currency_is_custom,
           COALESCE(
             (SELECT ed.amount_minor
              FROM event e
              JOIN event_data ed ON ed.id = e.latest_data_id
              WHERE e.account_id = a.id
                AND e.deleted_at IS NULL
                AND e.event_type = 'balance_update'
                AND ed.event_date <= ?1
              ORDER BY ed.event_date DESC, e.created_at DESC
              LIMIT 1),
             0
           ) + COALESCE(
             (SELECT SUM(ed2.amount_minor)
              FROM event e2
              JOIN event_data ed2 ON ed2.id = e2.latest_data_id
              WHERE e2.account_id = a.id
                AND e2.deleted_at IS NULL
                AND e2.event_type IN ('cashflow', 'transfer')
                AND ed2.event_date <= ?1
                AND ed2.event_date > COALESCE(
                  (SELECT ed3.event_date
                   FROM event e3
                   JOIN event_data ed3 ON ed3.id = e3.latest_data_id
                   WHERE e3.account_id = a.id
                     AND e3.deleted_at IS NULL
                     AND e3.event_type = 'balance_update'
                     AND ed3.event_date <= ?1
                   ORDER BY ed3.event_date DESC, e3.created_at DESC
                   LIMIT 1),
                  ''
                )),
             0
           ) AS balance_minor,
           a.person_id,
           a.purchase_price_minor,
           a.purchase_date,
           a.depreciation_period_months
         FROM account a
         JOIN currency c ON c.id = a.currency_id
         WHERE a.account_type IN ('account', 'bucket', 'asset')
           AND (?2 IS NULL OR a.person_id = ?2)
         ORDER BY a.account_type, a.sort_order, a.id",
    )?;

    let mut rows: Vec<SnapshotRow> = stmt
        .query_map(params![events_until, person_id], |row| {
            let held = Held {
                currency_id: row.get(4)?,
                minor_units: row.get(6)?,
                is_custom: row.get::<_, i64>(7)? != 0,
            };
            let balance_minor: i64 = row.get(8)?;
            Ok(SnapshotRow {
                account_id: row.get(0)?,
                account_name: row.get(1)?,
                account_type: row.get(2)?,
                iban: row.get(3)?,
                balance_minor,
                currency_code: row.get(5)?,
                currency_minor_units: held.minor_units,
                is_custom: held.is_custom,
                valuation: converter.value(balance_minor, held)?,
                is_linked_to_asset: false,
                linked_asset_ids: vec![],
                is_bucket_linked: false,
                bucket_links: vec![],
                tagged_cashflows: vec![],
                counted_minor: 0,
                equity_minor: None,
                person_id: row.get(9)?,
                purchase_price_minor: row.get(10)?,
                purchase_date: row.get(11)?,
                depreciation_period_months: row.get(12)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // Populate asset-link fields using a single bulk query to avoid N+1.
    let (linked_account_ids_set, account_to_assets, asset_to_accounts) =
        get_all_account_asset_link_ids(conn)?;
    for row in &mut rows {
        match row.account_type {
            AccountKind::Account => {
                if linked_account_ids_set.contains(&row.account_id) {
                    row.is_linked_to_asset = true;
                    if let Some(asset_ids) = account_to_assets.get(&row.account_id) {
                        row.linked_asset_ids = asset_ids.clone();
                    }
                }
            }
            AccountKind::Asset => {
                if let Some(account_ids) = asset_to_accounts.get(&row.account_id) {
                    row.linked_asset_ids = account_ids.clone();
                }
            }
            AccountKind::Bucket => {}
        }
    }

    // Pass 2: Event-bound bucket links
    let index_by_id: HashMap<i64, usize> = rows
        .iter()
        .enumerate()
        .map(|(idx, row)| (row.account_id, idx))
        .collect();
    for (bucket_id, link) in list_all_latest_bucket_links(conn, events_until)? {
        let (Some(&bucket_idx), Some(&source_idx)) = (
            index_by_id.get(&bucket_id),
            index_by_id.get(&link.source_account_id),
        ) else {
            continue;
        };
        let source = &rows[source_idx];
        let valued = ValuedBucketLink {
            balance_minor: source.balance_minor,
            valuation: source.valuation.clone(),
            link,
        };
        rows[bucket_idx].bucket_links.push(valued);
        rows[source_idx].is_bucket_linked = true;
    }

    // Pass 3: Cashflows tagged to a bucket via event_data.bucket_id, one entry per source account.
    let mut stmt = conn.prepare(
        "SELECT
           ed.bucket_id AS bucket_account_id,
           a.id AS source_account_id,
           a.name AS source_account_name,
           c.id AS currency_id,
           c.code AS currency_code,
           c.minor_units,
           c.is_custom,
           SUM(ed.amount_minor) AS tagged_total
         FROM event e
         JOIN event_data ed ON ed.id = e.latest_data_id
         JOIN account a ON a.id = e.account_id
         JOIN currency c ON c.id = a.currency_id
         WHERE e.deleted_at IS NULL
           AND ed.bucket_id IS NOT NULL
           AND ed.event_date <= ?1
           AND ed.event_date > COALESCE(
             (SELECT ed_bu.event_date
              FROM event e_bu
              JOIN event_data ed_bu ON ed_bu.id = e_bu.latest_data_id
              WHERE e_bu.account_id = ed.bucket_id
                AND e_bu.deleted_at IS NULL
                AND e_bu.event_type = 'balance_update'
                AND ed_bu.event_date <= ?1
              ORDER BY ed_bu.event_date DESC, e_bu.created_at DESC
              LIMIT 1),
             '')
         GROUP BY ed.bucket_id, e.account_id
         ORDER BY ed.bucket_id, e.account_id",
    )?;
    let tagged: Vec<(i64, TaggedCashflows)> = stmt
        .query_map(params![events_until], |row| {
            let held = Held {
                currency_id: row.get(3)?,
                minor_units: row.get(5)?,
                is_custom: row.get::<_, i64>(6)? != 0,
            };
            let amount_minor: i64 = row.get(7)?;
            let tagged = TaggedCashflows {
                source_account_id: row.get(1)?,
                source_account_name: row.get(2)?,
                currency_code: row.get(4)?,
                currency_minor_units: held.minor_units,
                is_custom: held.is_custom,
                amount_minor,
                valuation: converter.value(amount_minor, held)?,
            };
            Ok((row.get(0)?, tagged))
        })?
        .collect::<rusqlite::Result<_>>()?;
    for (bucket_id, tagged) in tagged {
        // TODO: This double-counts if the same source account is also contributing via
        // bucket_event_link (Pass 2). The UI wizard should prevent this scenario.
        if let Some(&bucket_idx) = index_by_id.get(&bucket_id) {
            rows[bucket_idx].tagged_cashflows.push(tagged);
        }
    }

    let equities: Vec<Option<i64>> = rows
        .iter()
        .map(|row| metrics::equity_minor(row, &rows))
        .collect();
    for (row, equity_minor) in rows.iter_mut().zip(equities) {
        row.counted_minor = metrics::counted_minor(row);
        row.equity_minor = equity_minor;
    }

    Ok(Snapshot {
        metrics: metrics::headline_metrics(&rows, &consolidation),
        rate_warnings: metrics::rate_warnings(&rows),
        rows,
    })
}

#[cfg(test)]
pub(crate) fn rows_at(conn: &Connection, until: &str) -> Vec<SnapshotRow> {
    get_accounts_snapshot(
        conn,
        GetSnapshotParams {
            moment: Moment::at(until).unwrap(),
            person_id: None,
        },
    )
    .unwrap()
    .rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::assets::repository::set_account_asset_links;
    use crate::features::buckets::repository::{set_bucket_event_links, SetBucketEventLinksParams};
    use crate::features::currency::repository::{
        set_fx_rate_manual, upsert_fx_rates, SetFxRateManualParams,
    };
    use crate::features::transactions::models::{
        Metric, MetricKind, Money, RateProblem, RateWarning,
    };
    use crate::features::transactions::repository::{
        create_balance_update, create_cashflow, CashflowEntry, CreateBalanceUpdateParams,
    };

    fn mk_account(conn: &Connection, account_type: &str) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: format!("Test {account_type}"),
                currency_id: 1,
                account_type: account_type.to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn day(date: &str) -> NaiveDate {
        NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()
    }

    fn live(conn: &Connection, today: &str) -> Snapshot {
        get_accounts_snapshot(
            conn,
            GetSnapshotParams {
                moment: Moment::live(day(today)),
                person_id: None,
            },
        )
        .unwrap()
    }

    fn at(conn: &Connection, until: &str) -> Snapshot {
        get_accounts_snapshot(
            conn,
            GetSnapshotParams {
                moment: Moment::at(until).unwrap(),
                person_id: None,
            },
        )
        .unwrap()
    }

    fn metric(snapshot: &Snapshot, kind: MetricKind) -> &Metric {
        snapshot.metrics.iter().find(|m| m.kind() == kind).unwrap()
    }

    #[test]
    fn bucket_links_on_a_timed_balance_update_show_on_that_day() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn, "account");
        let bucket_id = mk_account(&conn, "bucket");
        let event_id = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: bucket_id,
                amount_minor: 0,
                event_date: "2026-10-23T23:59:00".to_owned(),
                note: None,
            },
        )
        .unwrap();
        set_bucket_event_links(
            &conn,
            SetBucketEventLinksParams {
                event_id,
                account_ids: vec![source_id],
            },
        )
        .unwrap();

        let snapshot = rows_at(&conn, "2026-10-23T23:59:59");

        let bucket = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();
        assert_eq!(bucket.bucket_links.len(), 1);
        assert_eq!(bucket.bucket_links[0].link.source_account_id, source_id);
        let source = snapshot.iter().find(|r| r.account_id == source_id).unwrap();
        assert!(source.is_bucket_linked);
    }

    fn currency_id(conn: &Connection, code: &str) -> i64 {
        conn.query_row(
            "SELECT id FROM currency WHERE code = ?1",
            params![code],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn mk_account_in(conn: &Connection, currency: &str, account_type: &str) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: format!("{currency} {account_type}"),
                currency_id: currency_id(conn, currency),
                account_type: account_type.to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn balance_update_on(conn: &Connection, account_id: i64, amount_minor: i64, date: &str) -> i64 {
        create_balance_update(
            conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor,
                event_date: date.to_owned(),
                note: None,
            },
        )
        .unwrap()
    }

    fn balance_update(conn: &Connection, account_id: i64, amount_minor: i64) -> i64 {
        balance_update_on(conn, account_id, amount_minor, "2026-03-01")
    }

    fn cashflow(
        conn: &Connection,
        account_id: i64,
        amount_minor: i64,
        date: &str,
        bucket_id: Option<i64>,
    ) {
        create_cashflow(
            conn,
            &CashflowEntry {
                account_id,
                amount_minor,
                event_date: date.to_owned(),
                note: None,
                counterpart_account_id: None,
                bucket_id,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
    }

    fn tagged_cashflow(conn: &Connection, account_id: i64, bucket_id: i64, amount_minor: i64) {
        cashflow(
            conn,
            account_id,
            amount_minor,
            "2026-03-02",
            Some(bucket_id),
        );
    }

    fn link_sources(conn: &Connection, bucket_id: i64, sources: &[i64]) {
        let event_id = balance_update(conn, bucket_id, 0);
        set_bucket_event_links(
            conn,
            SetBucketEventLinksParams {
                event_id,
                account_ids: sources.to_vec(),
            },
        )
        .unwrap();
    }

    fn manual_rate(conn: &Connection, code: &str, date: &str, mantissa: i64) {
        set_fx_rate_manual(
            conn,
            SetFxRateManualParams {
                from_currency_id: currency_id(conn, "EUR"),
                to_currency_id: currency_id(conn, code),
                date: date.to_owned(),
                rate_mantissa: mantissa,
                rate_exponent: -2,
                is_direct: false,
            },
        )
        .unwrap();
    }

    fn snapshot_on_march_5th(conn: &Connection) -> Vec<SnapshotRow> {
        rows_at(conn, "2026-03-05T23:59:59")
    }

    fn row(snapshot: &[SnapshotRow], account_id: i64) -> &SnapshotRow {
        snapshot
            .iter()
            .find(|r| r.account_id == account_id)
            .unwrap()
    }

    fn value(snapshot: &[SnapshotRow], account_id: i64) -> Option<i64> {
        row(snapshot, account_id).valuation.value_minor()
    }

    fn gbp(amount_minor: i64) -> Money {
        Money {
            amount_minor,
            currency_code: "GBP".to_owned(),
            minor_units: 2,
        }
    }

    #[test]
    fn live_counts_a_cashflow_dated_tomorrow_and_at_today_does_not() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account = mk_account_in(&conn, "EUR", "account");
        balance_update_on(&conn, account, 10_000, "2026-10-20T09:00:00");
        cashflow(&conn, account, 2_500, "2026-10-26T08:00:00", None);

        let live = live(&conn, "2026-10-25");
        let at_today = at(&conn, "2026-10-25");

        assert_eq!(row(&live.rows, account).balance_minor, 12_500);
        assert_eq!(
            metric(&live, MetricKind::TotalBalance).total_minor(),
            12_500
        );
        assert_eq!(row(&at_today.rows, account).balance_minor, 10_000);
        assert_eq!(
            metric(&at_today, MetricKind::TotalBalance).total_minor(),
            10_000
        );
    }

    #[test]
    fn live_ages_the_newest_rate_from_today() {
        let conn = initialize_in_memory().expect("DB init failed");
        let usd = mk_account_in(&conn, "USD", "account");
        balance_update_on(&conn, usd, 12_500, "2026-10-01");
        manual_rate(&conn, "USD", "2026-10-01", 100);
        manual_rate(&conn, "USD", "2026-10-10", 125);

        let snapshot = live(&conn, "2026-10-25");

        let Valuation::Converted { value_minor, rate } = &row(&snapshot.rows, usd).valuation else {
            panic!("USD account should be converted");
        };
        assert_eq!(*value_minor, 10_000);
        assert_eq!(
            (rate.date.as_str(), rate.age_days, rate.stale),
            ("2026-10-10", 15, true)
        );
        assert_eq!(
            snapshot.rate_warnings,
            [RateWarning {
                account_id: usd,
                account_name: "USD account".to_owned(),
                native: Money {
                    amount_minor: 12_500,
                    currency_code: "USD".to_owned(),
                    minor_units: 2
                },
                problem: RateProblem::Stale {
                    rate: rate.clone(),
                    limit_days: 7
                },
            }]
        );
    }

    #[test]
    fn time_machine_ages_the_rate_from_the_picked_day() {
        let conn = initialize_in_memory().expect("DB init failed");
        let usd = mk_account_in(&conn, "USD", "account");
        balance_update_on(&conn, usd, 12_500, "2026-10-01");
        manual_rate(&conn, "USD", "2026-10-01", 100);
        manual_rate(&conn, "USD", "2026-10-10", 125);

        let snapshot = at(&conn, "2026-10-12");

        let Valuation::Converted { rate, .. } = &row(&snapshot.rows, usd).valuation else {
            panic!("USD account should be converted");
        };
        assert_eq!(
            (rate.date.as_str(), rate.age_days, rate.stale),
            ("2026-10-10", 2, false)
        );
        assert_eq!(snapshot.rate_warnings, []);
    }

    #[test]
    fn rate_used_reports_date_and_manual_flag() {
        let conn = initialize_in_memory().expect("DB init failed");
        let usd = mk_account_in(&conn, "USD", "account");
        let gbp_account = mk_account_in(&conn, "GBP", "account");
        manual_rate(&conn, "USD", "2026-03-01", 125);
        upsert_fx_rates(
            &conn,
            &[(
                "2026-03-02".to_owned(),
                currency_id(&conn, "EUR"),
                currency_id(&conn, "GBP"),
                85,
                -2,
            )],
        )
        .unwrap();

        let rows = snapshot_on_march_5th(&conn);

        let rate_of = |id| match &row(&rows, id).valuation {
            Valuation::Converted { rate, .. } => (rate.date.clone(), rate.is_manual),
            other => panic!("expected a converted value, got {other:?}"),
        };
        assert_eq!(rate_of(usd), ("2026-03-01".to_owned(), true));
        assert_eq!(rate_of(gbp_account), ("2026-03-02".to_owned(), false));
    }

    #[test]
    fn account_without_a_rate_is_no_rate_and_counts_nothing() {
        let conn = initialize_in_memory().expect("DB init failed");
        let eur_account = mk_account_in(&conn, "EUR", "account");
        let gbp_account = mk_account_in(&conn, "GBP", "account");
        balance_update(&conn, eur_account, 5_000);
        balance_update(&conn, gbp_account, 1_000);

        let snapshot = at(&conn, "2026-03-05");

        let gbp_row = row(&snapshot.rows, gbp_account);
        assert_eq!(gbp_row.valuation, Valuation::NoRate);
        assert_eq!(gbp_row.counted_minor, 0);
        let total = metric(&snapshot, MetricKind::TotalBalance);
        assert_eq!(total.total_minor(), 5_000);
        assert!(total
            .counted()
            .iter()
            .all(|line| line.subject.account_id != gbp_account));
        assert_eq!(
            snapshot.rate_warnings,
            [RateWarning {
                account_id: gbp_account,
                account_name: "GBP account".to_owned(),
                native: gbp(1_000),
                problem: RateProblem::NoRate,
            }]
        );
    }

    #[test]
    fn bucket_link_from_an_account_without_a_rate_adds_nothing_to_the_bucket() {
        let conn = initialize_in_memory().expect("DB init failed");
        let gbp_account = mk_account_in(&conn, "GBP", "account");
        let bucket = mk_account_in(&conn, "EUR", "bucket");
        balance_update(&conn, gbp_account, 1_000);
        let event_id = balance_update(&conn, bucket, 2_000);
        set_bucket_event_links(
            &conn,
            SetBucketEventLinksParams {
                event_id,
                account_ids: vec![gbp_account],
            },
        )
        .unwrap();

        let rows = snapshot_on_march_5th(&conn);

        let bucket_row = row(&rows, bucket);
        let link = &bucket_row.bucket_links[0];
        assert_eq!(
            (link.balance_minor, &link.valuation),
            (1_000, &Valuation::NoRate)
        );
        assert_eq!(bucket_row.counted_minor, 2_000);
    }

    #[test]
    fn tagged_cashflows_without_a_rate_add_nothing_to_the_bucket() {
        let conn = initialize_in_memory().expect("DB init failed");
        let gbp_account = mk_account_in(&conn, "GBP", "account");
        let bucket = mk_account_in(&conn, "EUR", "bucket");
        balance_update(&conn, bucket, 2_000);
        tagged_cashflow(&conn, gbp_account, bucket, 500);

        let rows = snapshot_on_march_5th(&conn);

        let bucket_row = row(&rows, bucket);
        let tagged = &bucket_row.tagged_cashflows[0];
        assert_eq!(
            (tagged.amount_minor, &tagged.valuation),
            (500, &Valuation::NoRate)
        );
        assert_eq!(bucket_row.counted_minor, 2_000);
    }

    #[test]
    fn several_accounts_in_one_foreign_currency_convert_at_the_same_rate() {
        let conn = initialize_in_memory().expect("DB init failed");
        manual_rate(&conn, "USD", "2026-03-01", 125);
        let eur_account = mk_account_in(&conn, "EUR", "account");
        let (a1, a2, a3, a4) = (
            mk_account_in(&conn, "USD", "account"),
            mk_account_in(&conn, "USD", "account"),
            mk_account_in(&conn, "USD", "account"),
            mk_account_in(&conn, "USD", "account"),
        );
        let bucket = mk_account_in(&conn, "EUR", "bucket");
        balance_update(&conn, eur_account, 7000);
        balance_update(&conn, a1, 12_500);
        balance_update(&conn, a2, 5_000);
        balance_update(&conn, a3, 333);
        // a4 has no balance update; its balance is the two tagged cashflows.
        tagged_cashflow(&conn, a4, bucket, 2_500);
        tagged_cashflow(&conn, a4, bucket, 1_000);
        link_sources(&conn, bucket, &[a1, a2, a3]);

        let snapshot = snapshot_on_march_5th(&conn);

        assert_eq!(
            row(&snapshot, eur_account).valuation,
            Valuation::SameCurrency { value_minor: 7_000 }
        );
        assert_eq!(value(&snapshot, a1), Some(10_000));
        assert_eq!(value(&snapshot, a2), Some(4_000));
        assert_eq!(value(&snapshot, a3), Some(266));
        assert_eq!(row(&snapshot, a4).balance_minor, 3_500);
        assert_eq!(value(&snapshot, a4), Some(2_800));
        let bucket_row = row(&snapshot, bucket);
        let links: Vec<(i64, i64, Option<i64>)> = bucket_row
            .bucket_links
            .iter()
            .map(|l| {
                (
                    l.link.source_account_id,
                    l.balance_minor,
                    l.valuation.value_minor(),
                )
            })
            .collect();
        assert_eq!(
            links,
            [
                (a1, 12_500, Some(10_000)),
                (a2, 5_000, Some(4_000)),
                (a3, 333, Some(266))
            ]
        );
        let tagged: Vec<(i64, i64, Option<i64>)> = bucket_row
            .tagged_cashflows
            .iter()
            .map(|t| {
                (
                    t.source_account_id,
                    t.amount_minor,
                    t.valuation.value_minor(),
                )
            })
            .collect();
        assert_eq!(tagged, [(a4, 3_500, Some(2_800))]);
        assert_eq!(bucket_row.counted_minor, 17_066);
    }

    #[test]
    fn several_accounts_in_a_currency_without_a_rate_are_left_out_and_each_warns_once() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (g1, g2, g3) = (
            mk_account_in(&conn, "GBP", "account"),
            mk_account_in(&conn, "GBP", "account"),
            mk_account_in(&conn, "GBP", "account"),
        );
        let bucket = mk_account_in(&conn, "EUR", "bucket");
        balance_update(&conn, g1, 1_000);
        balance_update(&conn, g2, 2_500);
        tagged_cashflow(&conn, g3, bucket, 300);
        tagged_cashflow(&conn, g3, bucket, 200);
        link_sources(&conn, bucket, &[g1, g2]);

        let snapshot = at(&conn, "2026-03-05");

        for id in [g1, g2, g3] {
            assert_eq!(row(&snapshot.rows, id).valuation, Valuation::NoRate);
        }
        let bucket_row = row(&snapshot.rows, bucket);
        assert_eq!(bucket_row.counted_minor, 0);
        assert_eq!(
            bucket_row.tagged_cashflows[0].source_account_name,
            "GBP account"
        );
        for metric in &snapshot.metrics {
            assert_eq!(metric.total_minor(), 0, "{:?}", metric.kind());
        }
        let warned: Vec<(i64, Money)> = snapshot
            .rate_warnings
            .iter()
            .map(|w| (w.account_id, w.native.clone()))
            .collect();
        assert_eq!(warned, [(g1, gbp(1_000)), (g2, gbp(2_500)), (g3, gbp(500))]);
    }

    #[test]
    fn bucket_links_do_not_show_before_the_balance_update() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn, "account");
        let bucket_id = mk_account(&conn, "bucket");
        let event_id = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: bucket_id,
                amount_minor: 0,
                event_date: "2026-10-23T09:00:00".to_owned(),
                note: None,
            },
        )
        .unwrap();
        set_bucket_event_links(
            &conn,
            SetBucketEventLinksParams {
                event_id,
                account_ids: vec![source_id],
            },
        )
        .unwrap();

        let snapshot = rows_at(&conn, "2026-10-22T23:59:59");

        let bucket = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();
        assert!(bucket.bucket_links.is_empty());
    }

    #[test]
    fn moment_at_reads_a_bare_date_as_end_of_day() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account = mk_account_in(&conn, "EUR", "account");
        cashflow(&conn, account, 100, "2026-03-01T22:00:00", None);
        cashflow(&conn, account, 40, "2026-03-02T00:00:00", None);

        assert_eq!(
            row(&rows_at(&conn, "2026-03-01"), account).balance_minor,
            100
        );
        assert_eq!(
            row(&rows_at(&conn, "2026-03-01T21:59:59"), account).balance_minor,
            0
        );
    }

    #[test]
    fn moment_at_rejects_an_unpadded_date() {
        for until in ["2026-3-1", "2026-03-01 10:00:00", "", "live"] {
            let err = Moment::at(until).err().expect(until);
            assert_eq!(err.code, "VALIDATION", "{until}");
        }
    }

    #[test]
    fn asset_equity_sums_counted_values_of_the_asset_and_its_linked_accounts() {
        let conn = initialize_in_memory().expect("DB init failed");
        manual_rate(&conn, "USD", "2026-03-01", 125);
        let house = mk_account_in(&conn, "EUR", "asset");
        let mortgage = mk_account_in(&conn, "EUR", "account");
        let usd_loan = mk_account_in(&conn, "USD", "account");
        balance_update(&conn, house, 400_000);
        balance_update(&conn, mortgage, -150_000);
        balance_update(&conn, usd_loan, -12_500);
        set_account_asset_links(&conn, mortgage, &[house]).unwrap();
        set_account_asset_links(&conn, usd_loan, &[house]).unwrap();

        let rows = snapshot_on_march_5th(&conn);

        assert_eq!(
            row(&rows, house).equity_minor,
            Some(400_000 - 150_000 - 10_000)
        );
        assert_eq!(row(&rows, mortgage).equity_minor, None);
    }
}
