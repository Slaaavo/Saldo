use chrono::NaiveDate;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use rusqlite::{params, Connection};
use std::collections::HashMap;

use crate::error::AppError;
use crate::features::assets::repository::get_all_account_asset_link_ids;
use crate::features::buckets::repository::list_all_latest_bucket_links;
use crate::features::buckets::{BucketKind, BucketState};
use crate::features::currency::models::Currency;
use crate::features::currency::rate_age::{is_stale, rate_age_days};
use crate::features::currency::repository::{
    get_consolidation_currency, get_fx_rate_for_conversion, GetFxRateForConversionParams,
};
use crate::features::income_budget;
use crate::features::transactions::metrics;
use crate::features::transactions::models::{
    AccountKind, InTransit, RateUsed, Snapshot, SnapshotRow, TaggedCashflows, Valuation,
    ValuedBucketLink,
};
use crate::shared::{convert_balance, is_iso_date, validate_event_date};

use super::balance_sql::{balance_sql, AS_STORED};

// Validated dates always compare below it, so the live cutoff keeps every row and every rate.
const LIVE_EVENTS_UNTIL: &str = "9999-12-31T23:59:59";

pub struct Moment {
    events_until: String,
    age_reference: NaiveDate,
    live: bool,
}

impl Moment {
    /// Everything Saldo knows. Rows and rates dated after today count, and rates age from today.
    pub fn live(today: NaiveDate) -> Moment {
        Moment {
            events_until: LIVE_EVENTS_UNTIL.to_owned(),
            age_reference: today,
            live: true,
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
            live: false,
        })
    }

    fn rates_until(&self) -> &str {
        &self.events_until[..10]
    }

    /// True only for `Moment::live`, never for a picked moment, however late.
    pub(crate) fn is_live(&self) -> bool {
        self.live
    }

    pub(crate) fn day(&self) -> NaiveDate {
        self.age_reference
    }

    pub(crate) fn events_until(&self) -> &str {
        &self.events_until
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

pub struct ValueAmountParams {
    pub moment: Moment,
    pub currency_id: i64,
    pub amount_minor: i64,
}

pub(crate) fn value_amount(
    conn: &Connection,
    params: ValueAmountParams,
) -> rusqlite::Result<Valuation> {
    let consolidation = get_consolidation_currency(conn)?;
    let held = conn.query_row(
        "SELECT minor_units, is_custom FROM currency WHERE id = ?1",
        params![params.currency_id],
        |row| {
            Ok(Held {
                currency_id: params.currency_id,
                minor_units: row.get(0)?,
                is_custom: row.get::<_, i64>(1)? != 0,
            })
        },
    )?;
    ConsolidationConverter {
        conn,
        consolidation: &consolidation,
        moment: &params.moment,
        rates: HashMap::new(),
    }
    .value(params.amount_minor, held)
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

    let balance = balance_sql("a.id", |_, ed| format!("{ed}.event_date <= ?1"), &AS_STORED);
    let mut stmt = conn.prepare(&format!(
        "SELECT
           a.id AS account_id,
           a.name AS account_name,
           a.account_type,
           a.iban,
           c.id AS currency_id,
           c.code AS currency_code,
           c.minor_units AS currency_minor_units,
           c.is_custom AS currency_is_custom,
           {balance} AS balance_minor,
           a.person_id,
           a.purchase_price_minor,
           a.purchase_date,
           a.depreciation_period_months,
           a.bucket_enabled,
           a.bucket_kind
         FROM account a
         JOIN currency c ON c.id = a.currency_id
         WHERE a.account_type IN ('account', 'bucket', 'asset')
           AND (?2 IS NULL OR a.person_id = ?2)
         ORDER BY a.account_type, a.sort_order, a.id",
    ))?;

    let mut rows: Vec<SnapshotRow> = stmt
        .query_map(params![events_until, person_id], |row| {
            let held = Held {
                currency_id: row.get(4)?,
                minor_units: row.get(6)?,
                is_custom: row.get::<_, i64>(7)? != 0,
            };
            let account_id: i64 = row.get(0)?;
            let account_type: AccountKind = row.get(2)?;
            let person_id: Option<i64> = row.get(9)?;
            let bucket = match account_type {
                AccountKind::Bucket => Some(BucketState {
                    kind: row.get(14)?,
                    enabled: row.get(13)?,
                }),
                AccountKind::Account | AccountKind::Asset => None,
            };
            let balance_minor: i64 = match (bucket, person_id) {
                (
                    Some(BucketState {
                        kind: BucketKind::IncomeBudget,
                        ..
                    }),
                    Some(person_id),
                ) => income_budget::value_at(
                    conn,
                    income_budget::ValueAtParams {
                        bucket_id: account_id,
                        person_id,
                        currency_id: held.currency_id,
                        events_until: moment.events_until().to_owned(),
                        day: moment.day(),
                    },
                )?,
                _ => row.get(8)?,
            };
            Ok(SnapshotRow {
                account_id,
                account_name: row.get(1)?,
                account_type,
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
                person_id,
                purchase_price_minor: row.get(10)?,
                purchase_date: row.get(11)?,
                depreciation_period_months: row.get(12)?,
                bucket,
                verification: None,
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

    let mut in_transit = transfers_in_transit(&mut converter, events_until)?;
    in_transit.retain(|transit| index_by_id.contains_key(&transit.account_id));

    let equities: Vec<Option<i64>> = rows
        .iter()
        .map(|row| metrics::equity_minor(row, &rows))
        .collect();
    for (row, equity_minor) in rows.iter_mut().zip(equities) {
        row.counted_minor = metrics::counted_minor(row);
        row.equity_minor = equity_minor;
    }

    Ok(Snapshot {
        metrics: metrics::headline_metrics(&rows, &in_transit, &consolidation),
        rate_warnings: metrics::rate_warnings(&rows, &in_transit),
        rows,
    })
}

fn transfers_in_transit(
    converter: &mut ConsolidationConverter,
    events_until: &str,
) -> rusqlite::Result<Vec<InTransit>> {
    let conn = converter.conn;
    let mut stmt = conn.prepare(
        "SELECT
           e.account_id,
           twin.account_id AS counterpart_account_id,
           counterpart.name AS counterpart_account_name,
           c.id AS currency_id,
           c.code AS currency_code,
           c.minor_units,
           c.is_custom,
           SUM(ed.amount_minor) AS in_transit_total
         FROM event e
         JOIN event_data ed ON ed.id = e.latest_data_id
         JOIN event twin ON twin.id = e.linked_event_id
         JOIN event_data twin_ed ON twin_ed.id = twin.latest_data_id
         JOIN account a ON a.id = e.account_id
         JOIN currency c ON c.id = a.currency_id
         JOIN account counterpart ON counterpart.id = twin.account_id
         WHERE e.deleted_at IS NULL
           AND e.event_type = 'transfer'
           AND ed.event_date > ?1
           AND twin.deleted_at IS NULL
           AND twin.event_type = 'transfer'
           AND twin_ed.event_date <= ?1
         GROUP BY e.account_id, twin.account_id
         HAVING SUM(ed.amount_minor) <> 0
         ORDER BY e.account_id, twin.account_id",
    )?;
    let in_transit = stmt
        .query_map(params![events_until], |row| {
            let held = Held {
                currency_id: row.get(3)?,
                minor_units: row.get(5)?,
                is_custom: row.get::<_, i64>(6)? != 0,
            };
            let amount_minor: i64 = row.get(7)?;
            Ok(InTransit {
                account_id: row.get(0)?,
                counterpart_account_id: row.get(1)?,
                counterpart_account_name: row.get(2)?,
                currency_code: row.get(4)?,
                currency_minor_units: held.minor_units,
                is_custom: held.is_custom,
                amount_minor,
                valuation: converter.value(amount_minor, held)?,
            })
        })?
        .collect();
    in_transit
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
        LinePart, Metric, MetricKind, Money, NotCountedReason, RateProblem, RateWarning,
    };
    use crate::features::transactions::repository::{
        create_balance_update, create_cashflow, create_transfer, delete_event, update_event,
        CashflowEntry, CreateBalanceUpdateParams, UpdateEventParams,
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
    fn only_the_live_moment_is_live() {
        assert!(Moment::live(day("2026-11-14")).is_live());
        assert!(!Moment::at("9999-12-31").unwrap().is_live());
        assert!(!Moment::at("9999-12-31T23:59:59").unwrap().is_live());
    }

    fn switch_off(conn: &Connection, bucket_id: i64) {
        crate::features::buckets::repository::set_bucket_enabled(
            conn,
            crate::features::buckets::repository::SetBucketEnabledParams {
                bucket_id,
                enabled: false,
            },
        )
        .unwrap();
    }

    #[test]
    fn a_bucket_row_carries_kind_and_switch_and_an_account_row_none() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account = mk_account(&conn, "account");
        let asset = mk_account(&conn, "asset");
        let on = mk_account(&conn, "bucket");
        let off = mk_account(&conn, "bucket");
        switch_off(&conn, off);
        let income = create_account(
            &conn,
            CreateAccountParams {
                name: "Income".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                person_id: Some(1),
                bucket_kind: BucketKind::IncomeBudget,
                ..Default::default()
            },
        )
        .unwrap();

        let rows = rows_at(&conn, "2026-03-05");

        assert_eq!(row(&rows, account).bucket, None);
        assert_eq!(row(&rows, asset).bucket, None);
        assert_eq!(
            row(&rows, on).bucket,
            Some(BucketState {
                kind: BucketKind::Balance,
                enabled: true
            })
        );
        assert_eq!(
            row(&rows, off).bucket,
            Some(BucketState {
                kind: BucketKind::Balance,
                enabled: false
            })
        );
        assert_eq!(
            serde_json::to_value(row(&rows, off)).unwrap()["bucket"],
            serde_json::json!({ "kind": "balance", "enabled": false })
        );
        assert_eq!(
            serde_json::to_value(row(&rows, income)).unwrap()["bucket"],
            serde_json::json!({ "kind": "incomeBudget", "enabled": true })
        );
        assert_eq!(
            serde_json::to_value(row(&rows, account)).unwrap()["bucket"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn a_switch_is_the_same_at_every_moment() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account = mk_account_in(&conn, "EUR", "account");
        let bucket = mk_account_in(&conn, "EUR", "bucket");
        balance_update(&conn, account, 10_000);
        balance_update_on(&conn, bucket, 4_000, "2026-03-01");
        switch_off(&conn, bucket);

        for snapshot in [
            at(&conn, "2026-02-01"),
            at(&conn, "2026-03-05"),
            live(&conn, "2026-11-08"),
        ] {
            assert!(row(&snapshot.rows, bucket).is_switched_off());
            let left = metric(&snapshot, MetricKind::LeftToSpend);
            assert!(left
                .counted()
                .iter()
                .all(|line| line.subject.account_id != bucket));
        }
        assert_eq!(
            metric(&at(&conn, "2026-03-05"), MetricKind::LeftToSpend).total_minor(),
            10_000
        );
    }

    fn income_budget_in(conn: &Connection, currency: &str, person_id: i64) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: format!("Income {currency}"),
                currency_id: currency_id(conn, currency),
                account_type: "bucket".to_owned(),
                person_id: Some(person_id),
                bucket_kind: BucketKind::IncomeBudget,
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn budget_month(conn: &Connection, bucket_id: i64, month: &str, budget: i64, vat: i64) {
        conn.execute(
            "INSERT INTO income_budget_month (bucket_id, month, budget_minor, vat_bps)
             VALUES (?1, ?2, ?3, ?4)",
            params![bucket_id, month, budget, vat],
        )
        .unwrap();
    }

    fn account_of(conn: &Connection, currency: &str, person_id: i64) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: format!("{currency} account of {person_id}"),
                currency_id: currency_id(conn, currency),
                account_type: "account".to_owned(),
                person_id: Some(person_id),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn snapshot_of(conn: &Connection, until: &str, person_id: Option<i64>) -> Snapshot {
        get_accounts_snapshot(
            conn,
            GetSnapshotParams {
                moment: Moment::at(until).unwrap(),
                person_id,
            },
        )
        .unwrap()
    }

    #[test]
    fn an_income_budget_row_holds_its_computed_value_live_and_at_a_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let consulting = account_of(&conn, "EUR", 1);
        let budget = income_budget_in(&conn, "EUR", 1);
        budget_month(&conn, budget, "2026-10", 564_000, 2_300);
        budget_month(&conn, budget, "2026-11", 612_000, 2_300);
        cashflow(&conn, consulting, 720_411, "2026-10-12T09:00:00", None);
        cashflow(&conn, consulting, 500_000, "2026-11-20T09:00:00", None);

        let november_8th = at(&conn, "2026-11-08");
        let october_31st = at(&conn, "2026-10-31");
        let live = live(&conn, "2026-11-08");

        let row_at_8th = row(&november_8th.rows, budget);
        assert_eq!(row_at_8th.balance_minor, -612_000);
        assert_eq!(
            row_at_8th.valuation,
            Valuation::SameCurrency {
                value_minor: -612_000
            }
        );
        assert_eq!(row_at_8th.counted_minor, -612_000);
        assert_eq!(
            metric(&november_8th, MetricKind::LeftToSpend).total_minor(),
            720_411 + 612_000
        );
        assert_eq!(
            row(&october_31st.rows, budget).balance_minor,
            585_700 - 564_000
        );
        assert_eq!(row(&live.rows, budget).balance_minor, 406_504 - 612_000);
    }

    #[test]
    fn the_person_filter_does_not_change_an_income_budgets_value() {
        let conn = initialize_in_memory().expect("DB init failed");
        let other = crate::features::persons::repository::create_person(
            &conn,
            crate::features::persons::repository::CreatePersonParams {
                name: "Other".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .unwrap();
        let mine = account_of(&conn, "EUR", 1);
        let theirs = account_of(&conn, "EUR", other);
        let budget = income_budget_in(&conn, "EUR", 1);
        budget_month(&conn, budget, "2026-10", 100_000, 0);
        cashflow(&conn, mine, 80_000, "2026-10-12", None);
        cashflow(&conn, theirs, 900_000, "2026-10-12", None);

        let everyone = snapshot_of(&conn, "2026-10-31", None);
        let only_mine = snapshot_of(&conn, "2026-10-31", Some(1));

        assert_eq!(row(&everyone.rows, budget).balance_minor, -20_000);
        assert_eq!(row(&only_mine.rows, budget).balance_minor, -20_000);
    }

    #[test]
    fn an_income_budget_in_a_currency_without_a_rate_counts_nothing_and_warns() {
        let conn = initialize_in_memory().expect("DB init failed");
        let dollars = account_of(&conn, "USD", 1);
        let budget = income_budget_in(&conn, "USD", 1);
        budget_month(&conn, budget, "2026-10", 50_000, 0);
        cashflow(&conn, dollars, 100_000, "2026-10-12", None);

        let snapshot = at(&conn, "2026-10-31");

        let budget_row = row(&snapshot.rows, budget);
        assert_eq!(budget_row.balance_minor, 50_000);
        assert_eq!(budget_row.valuation, Valuation::NoRate);
        assert_eq!(budget_row.counted_minor, 0);
        let left = metric(&snapshot, MetricKind::LeftToSpend);
        let skipped: Vec<(LinePart, Money, NotCountedReason)> = left
            .skipped()
            .iter()
            .filter(|line| line.subject.account_id == budget)
            .map(|line| (line.subject.part.clone(), line.amount.clone(), line.reason))
            .collect();
        let usd = |amount_minor| Money {
            amount_minor,
            currency_code: "USD".to_owned(),
            minor_units: 2,
        };
        assert_eq!(
            skipped,
            [(
                LinePart::IncomeBudget,
                usd(50_000),
                NotCountedReason::NoRate
            )]
        );
        let warned: Vec<(i64, Money)> = snapshot
            .rate_warnings
            .iter()
            .map(|w| (w.account_id, w.native.clone()))
            .collect();
        assert!(warned.contains(&(budget, usd(50_000))), "{warned:?}");
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

    fn own_account(conn: &Connection, name: &str, currency: &str) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: name.to_owned(),
                currency_id: currency_id(conn, currency),
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn transfer_entry(from: i64, to: i64, amount_minor: i64, day: &str) -> CashflowEntry {
        CashflowEntry {
            account_id: from,
            amount_minor: -amount_minor,
            event_date: day.to_owned(),
            note: None,
            counterpart_account_id: Some(to),
            bucket_id: None,
            original_currency_id: None,
            original_amount_minor: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
        }
    }

    fn transfer(conn: &Connection, from: i64, to: i64, amount_minor: i64, day: &str) -> (i64, i64) {
        create_transfer(conn, &transfer_entry(from, to, amount_minor, day)).unwrap()
    }

    fn move_leg(conn: &Connection, leg_id: i64, amount_minor: i64, day: &str) {
        update_event(
            conn,
            UpdateEventParams {
                event_id: leg_id,
                amount_minor,
                event_date: day.to_owned(),
                note: None,
            },
        )
        .unwrap();
    }

    fn in_transit(counterpart_account_id: i64, counterpart_account_name: &str) -> LinePart {
        LinePart::InTransit {
            counterpart_account_id,
            counterpart_account_name: counterpart_account_name.to_owned(),
        }
    }

    fn counted_in_transit(snapshot: &Snapshot, kind: MetricKind) -> Vec<(i64, LinePart, i64)> {
        metric(snapshot, kind)
            .counted()
            .iter()
            .filter(|line| matches!(line.subject.part, LinePart::InTransit { .. }))
            .map(|line| {
                (
                    line.subject.account_id,
                    line.subject.part.clone(),
                    line.signed_minor,
                )
            })
            .collect()
    }

    fn checking_and_savings(conn: &Connection) -> (i64, i64) {
        let checking = own_account(conn, "Checking", "EUR");
        let savings = own_account(conn, "Savings", "EUR");
        balance_update_on(conn, checking, 100_000, "2026-02-01");
        balance_update_on(conn, savings, 20_000, "2026-02-01");
        (checking, savings)
    }

    #[test]
    fn a_transfer_received_two_days_after_it_was_sent_is_in_transit_in_between() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (checking, savings) = checking_and_savings(&conn);
        let (_, received) = transfer(&conn, checking, savings, 30_000, "2026-03-01");
        move_leg(&conn, received, 30_000, "2026-03-03");

        let march_2nd = at(&conn, "2026-03-02");

        assert_eq!(row(&march_2nd.rows, checking).balance_minor, 70_000);
        assert_eq!(row(&march_2nd.rows, savings).balance_minor, 20_000);
        assert_eq!(row(&march_2nd.rows, savings).counted_minor, 20_000);
        assert_eq!(
            counted_in_transit(&march_2nd, MetricKind::TotalBalance),
            [(savings, in_transit(checking, "Checking"), 30_000)]
        );
        for day in ["2026-02-28", "2026-03-01", "2026-03-02", "2026-03-03"] {
            let snapshot = at(&conn, day);
            assert_eq!(
                metric(&snapshot, MetricKind::TotalBalance).total_minor(),
                120_000,
                "{day}"
            );
        }
        assert_eq!(
            counted_in_transit(&at(&conn, "2026-03-03"), MetricKind::TotalBalance),
            []
        );
        let live = live(&conn, "2026-03-02");
        assert_eq!(counted_in_transit(&live, MetricKind::TotalBalance), []);
    }

    #[test]
    fn a_sending_leg_booked_after_its_twin_is_in_transit_with_its_negative_amount() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (checking, savings) = checking_and_savings(&conn);
        let (sent, _) = transfer(&conn, checking, savings, 30_000, "2026-03-01");
        move_leg(&conn, sent, -30_000, "2026-03-03");

        let march_2nd = at(&conn, "2026-03-02");

        assert_eq!(row(&march_2nd.rows, checking).balance_minor, 100_000);
        assert_eq!(
            counted_in_transit(&march_2nd, MetricKind::TotalBalance),
            [(checking, in_transit(savings, "Savings"), -30_000)]
        );
        assert_eq!(
            metric(&march_2nd, MetricKind::TotalBalance).total_minor(),
            120_000
        );
    }

    #[test]
    fn a_deleted_transfer_is_not_in_transit() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (checking, savings) = checking_and_savings(&conn);
        let (sent, received) = transfer(&conn, checking, savings, 30_000, "2026-03-01");
        move_leg(&conn, received, 30_000, "2026-03-03");
        delete_event(&conn, sent).unwrap();

        let march_2nd = at(&conn, "2026-03-02");

        assert_eq!(counted_in_transit(&march_2nd, MetricKind::TotalBalance), []);
        assert_eq!(
            metric(&march_2nd, MetricKind::TotalBalance).total_minor(),
            120_000
        );
    }

    #[test]
    fn transfers_between_the_same_two_accounts_are_one_in_transit_line() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (checking, savings) = checking_and_savings(&conn);
        let (_, first) = transfer(&conn, checking, savings, 30_000, "2026-03-01");
        move_leg(&conn, first, 30_000, "2026-03-03");
        let (_, second) = transfer(&conn, checking, savings, 5_000, "2026-02-27");
        move_leg(&conn, second, 5_000, "2026-03-04");

        let march_2nd = at(&conn, "2026-03-02");

        assert_eq!(
            counted_in_transit(&march_2nd, MetricKind::TotalBalance),
            [(savings, in_transit(checking, "Checking"), 35_000)]
        );
        assert_eq!(
            metric(&march_2nd, MetricKind::TotalBalance).total_minor(),
            120_000
        );
    }

    #[test]
    fn a_transfer_to_an_account_outside_the_person_filter_adds_no_line() {
        let conn = initialize_in_memory().expect("DB init failed");
        let other = crate::features::persons::repository::create_person(
            &conn,
            crate::features::persons::repository::CreatePersonParams {
                name: "Other".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .unwrap();
        let mine = account_of(&conn, "EUR", 1);
        let theirs = account_of(&conn, "EUR", other);
        balance_update_on(&conn, mine, 100_000, "2026-02-01");
        let (_, received) = transfer(&conn, mine, theirs, 30_000, "2026-03-01");
        move_leg(&conn, received, 30_000, "2026-03-03");

        let everyone = snapshot_of(&conn, "2026-03-02", None);
        let only_mine = snapshot_of(&conn, "2026-03-02", Some(1));

        assert_eq!(
            counted_in_transit(&everyone, MetricKind::TotalBalance),
            [(theirs, in_transit(mine, "EUR account of 1"), 30_000)]
        );
        assert_eq!(counted_in_transit(&only_mine, MetricKind::TotalBalance), []);
        assert_eq!(
            metric(&only_mine, MetricKind::TotalBalance).total_minor(),
            70_000
        );
    }

    #[test]
    fn money_in_transit_in_a_currency_without_a_rate_is_not_counted_and_warns() {
        let conn = initialize_in_memory().expect("DB init failed");
        let checking = own_account(&conn, "Checking", "EUR");
        let pounds = own_account(&conn, "Pounds", "GBP");
        balance_update_on(&conn, checking, 100_000, "2026-02-01");
        let (_, received) = create_transfer(
            &conn,
            &CashflowEntry {
                original_currency_id: Some(currency_id(&conn, "GBP")),
                original_amount_minor: Some(8_500),
                ..transfer_entry(checking, pounds, 10_000, "2026-03-01")
            },
        )
        .unwrap();
        move_leg(&conn, received, 8_500, "2026-03-03");

        let march_2nd = at(&conn, "2026-03-02");

        let total = metric(&march_2nd, MetricKind::TotalBalance);
        let skipped: Vec<(LinePart, Money, NotCountedReason)> = total
            .skipped()
            .iter()
            .filter(|line| line.subject.account_id == pounds)
            .map(|line| (line.subject.part.clone(), line.amount.clone(), line.reason))
            .collect();
        assert_eq!(
            skipped,
            [
                (LinePart::Balance, gbp(0), NotCountedReason::NoRate),
                (
                    in_transit(checking, "Checking"),
                    gbp(8_500),
                    NotCountedReason::NoRate
                ),
            ]
        );
        assert_eq!(total.total_minor(), 90_000);
        assert_eq!(
            march_2nd.rate_warnings,
            [RateWarning {
                account_id: pounds,
                account_name: "Pounds".to_owned(),
                native: gbp(8_500),
                problem: RateProblem::NoRate,
            }]
        );
    }

    fn left_to_spend_by_day(conn: &Connection) -> Vec<i64> {
        ["2026-02-28", "2026-03-01", "2026-03-02", "2026-03-03"]
            .into_iter()
            .map(|day| metric(&at(conn, day), MetricKind::LeftToSpend).total_minor())
            .collect()
    }

    #[test]
    fn a_bucket_earmarks_the_money_in_transit_to_an_account_it_links() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (checking, savings) = checking_and_savings(&conn);
        let both = mk_account(&conn, "bucket");
        let only_savings = mk_account(&conn, "bucket");
        for (bucket, linked) in [
            (both, vec![checking, savings]),
            (only_savings, vec![savings]),
        ] {
            let link_event = balance_update_on(&conn, bucket, 0, "2026-02-01");
            set_bucket_event_links(
                &conn,
                SetBucketEventLinksParams {
                    event_id: link_event,
                    account_ids: linked,
                },
            )
            .unwrap();
        }
        let (_, received) = transfer(&conn, checking, savings, 30_000, "2026-03-01");
        move_leg(&conn, received, 30_000, "2026-03-03");

        assert_eq!(
            left_to_spend_by_day(&conn),
            [-20_000, -50_000, -50_000, -50_000]
        );
        let bucket_lines: Vec<LinePart> = metric(&at(&conn, "2026-03-02"), MetricKind::LeftToSpend)
            .counted()
            .iter()
            .filter(|line| line.subject.account_id == only_savings)
            .map(|line| line.subject.part.clone())
            .collect();
        assert!(bucket_lines.contains(&LinePart::LinkedInTransit {
            source_account_id: savings,
            source_account_name: "Savings".to_owned(),
            counterpart_account_id: checking,
            counterpart_account_name: "Checking".to_owned(),
        }));
    }
}
