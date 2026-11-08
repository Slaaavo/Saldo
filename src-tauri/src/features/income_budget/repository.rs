use chrono::NaiveDate;
use rusqlite::types::Type;
use rusqlite::{named_params, params, Connection, OptionalExtension};

use super::calculation::{
    evaluate, first_month_needed, BudgetCell, Evaluation, GrossInflows, YearMonth,
};
use super::models::CountedAccount;
use crate::error::AppError;

/// The accounts whose incoming money is the budget's Received: the person's own accounts in the
/// budget's currency that Left to Spend counts. `gross_inflows` and `counted_accounts` share it,
/// so the dialog's list of accounts and its numbers cannot disagree.
const COUNTED_ACCOUNT: &str = "a.account_type = 'account'
    AND a.person_id = :person_id
    AND a.currency_id = :currency_id
    AND NOT EXISTS (SELECT 1 FROM account_asset_link l WHERE l.account_id = a.id)";

pub struct ReplaceCellsParams {
    pub bucket_id: i64,
    pub cells: Vec<BudgetCell>,
}

pub struct CountedAccountsParams {
    pub person_id: i64,
    pub currency_id: i64,
}

pub struct EvaluateDraftParams {
    pub person_id: i64,
    pub currency_id: i64,
    pub cells: Vec<BudgetCell>,
    pub lines_for: Option<(YearMonth, YearMonth)>,
    pub events_until: String,
    pub day: NaiveDate,
}

pub struct ValueAtParams {
    pub bucket_id: i64,
    pub person_id: i64,
    pub currency_id: i64,
    pub events_until: String,
    pub day: NaiveDate,
}

struct InflowsParams {
    person_id: i64,
    currency_id: i64,
    events_until: String,
    from: YearMonth,
    to: YearMonth,
}

/// The person's income budget. There is at most one, because the unique index allows no second.
pub fn income_budget_of(conn: &Connection, person_id: i64) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM account WHERE person_id = ?1 AND bucket_kind = 'income_budget'",
        params![person_id],
        |row| row.get(0),
    )
    .optional()
}

pub fn list_cells(conn: &Connection, bucket_id: i64) -> rusqlite::Result<Vec<BudgetCell>> {
    let mut stmt = conn.prepare(
        "SELECT month, budget_minor, vat_bps FROM income_budget_month
         WHERE bucket_id = ?1 ORDER BY month",
    )?;
    let cells = stmt
        .query_map(params![bucket_id], |row| {
            let month: String = row.get(0)?;
            let budget_minor: Option<i64> = row.get(1)?;
            let vat_bps: Option<i64> = row.get(2)?;
            YearMonth::parse(&month)
                .and_then(|month| BudgetCell::new(month, budget_minor, vat_bps))
                .map_err(stored)
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(cells)
}

/// Deletes every month of the bucket and inserts `cells`, so the stored set is exactly `cells`.
/// The caller owns the savepoint.
pub fn replace_cells(conn: &Connection, params: ReplaceCellsParams) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM income_budget_month WHERE bucket_id = ?1",
        params![params.bucket_id],
    )?;
    let mut insert = conn.prepare(
        "INSERT INTO income_budget_month (bucket_id, month, budget_minor, vat_bps)
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for cell in &params.cells {
        insert.execute(params![
            params.bucket_id,
            cell.month().to_string(),
            cell.budget_minor(),
            cell.vat_bps()
        ])?;
    }
    Ok(())
}

pub fn counted_accounts(
    conn: &Connection,
    params: CountedAccountsParams,
) -> rusqlite::Result<Vec<CountedAccount>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT a.id, a.name FROM account a WHERE {COUNTED_ACCOUNT} ORDER BY a.sort_order, a.id"
    ))?;
    let accounts = stmt
        .query_map(
            named_params! {
                ":person_id": params.person_id,
                ":currency_id": params.currency_id,
            },
            |row| {
                Ok(CountedAccount {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            },
        )?
        .collect::<rusqlite::Result<_>>()?;
    Ok(accounts)
}

/// Months are compared through `substr`, never as date ranges, because stored dates mix
/// date-only and timed values.
fn gross_inflows(conn: &Connection, params: InflowsParams) -> rusqlite::Result<GrossInflows> {
    let mut stmt = conn.prepare(&format!(
        "SELECT substr(ed.event_date, 1, 7) AS month, SUM(ed.amount_minor)
         FROM account a
         JOIN event e       ON e.account_id = a.id
         JOIN event_data ed ON ed.id = e.latest_data_id
         WHERE {COUNTED_ACCOUNT}
           AND e.event_type = 'cashflow'
           AND e.deleted_at IS NULL
           AND ed.amount_minor > 0
           AND ed.event_date <= :events_until
           AND substr(ed.event_date, 1, 7) BETWEEN :from AND :to
         GROUP BY month"
    ))?;
    let inflows = stmt
        .query_map(
            named_params! {
                ":person_id": params.person_id,
                ":currency_id": params.currency_id,
                ":events_until": params.events_until,
                ":from": params.from.to_string(),
                ":to": params.to.to_string(),
            },
            |row| {
                let month: String = row.get(0)?;
                Ok((YearMonth::parse(&month).map_err(stored)?, row.get(1)?))
            },
        )?
        .collect::<rusqlite::Result<_>>()?;
    Ok(inflows)
}

/// Evaluates `cells` against the inflows stored up to the moment. `day` decides which months
/// have started and `events_until` which cashflows count, so live counts a payment dated later in
/// the current month.
pub fn evaluate_draft(
    conn: &Connection,
    params: EvaluateDraftParams,
) -> rusqlite::Result<Evaluation> {
    let current = YearMonth::of(params.day);
    let gross = match first_month_needed(&params.cells, params.lines_for) {
        Some(from) if from <= current => gross_inflows(
            conn,
            InflowsParams {
                person_id: params.person_id,
                currency_id: params.currency_id,
                events_until: params.events_until,
                from,
                to: current,
            },
        )?,
        _ => GrossInflows::new(),
    };
    Ok(evaluate(&params.cells, &gross, current, params.lines_for))
}

/// The stored budget's value at the moment.
pub fn value_at(conn: &Connection, params: ValueAtParams) -> rusqlite::Result<i64> {
    let cells = list_cells(conn, params.bucket_id)?;
    let evaluation = evaluate_draft(
        conn,
        EvaluateDraftParams {
            person_id: params.person_id,
            currency_id: params.currency_id,
            cells,
            lines_for: None,
            events_until: params.events_until,
            day: params.day,
        },
    )?;
    Ok(evaluation.value_minor)
}

/// The table's CHECKs keep stored months valid, so this fires only on a damaged database.
fn stored(err: AppError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, Type::Text, err.message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{
        create_account, delete_account, CreateAccountParams,
    };
    use crate::features::assets::repository::set_account_asset_links;
    use crate::features::buckets::BucketKind;
    use crate::features::persons::repository::{create_person, delete_person, CreatePersonParams};
    use crate::features::transactions::models::{AccountKind, MetricKind};
    use crate::features::transactions::repository::{
        create_cashflow, create_transfer, delete_event, get_accounts_snapshot, update_event,
        CashflowEntry, GetSnapshotParams, Moment, UpdateEventParams,
    };

    const EUR: i64 = 1;
    const DEFAULT_PERSON: i64 = 1;
    const AT_NOV_8: &str = "2026-11-08T23:59:59";

    fn nov_8() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 8).unwrap()
    }

    fn month(text: &str) -> YearMonth {
        YearMonth::parse(text).unwrap()
    }

    fn currency_id(conn: &Connection, code: &str) -> i64 {
        conn.query_row(
            "SELECT id FROM currency WHERE code = ?1",
            params![code],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn other_person(conn: &Connection) -> i64 {
        create_person(
            conn,
            CreatePersonParams {
                name: "Other".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .unwrap()
    }

    fn mk(conn: &Connection, name: &str, account_type: &str, person_id: i64, currency: i64) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: name.to_owned(),
                currency_id: currency,
                account_type: account_type.to_owned(),
                person_id: Some(person_id),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn income_budget(conn: &Connection, person_id: i64) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: "Income".to_owned(),
                currency_id: EUR,
                account_type: "bucket".to_owned(),
                person_id: Some(person_id),
                bucket_kind: BucketKind::IncomeBudget,
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn entry(account_id: i64, amount_minor: i64, date: &str) -> CashflowEntry {
        CashflowEntry {
            account_id,
            amount_minor,
            event_date: date.to_owned(),
            note: None,
            counterpart_account_id: None,
            bucket_id: None,
            original_currency_id: None,
            original_amount_minor: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
        }
    }

    fn cashflow(conn: &Connection, account_id: i64, amount_minor: i64, date: &str) -> i64 {
        create_cashflow(conn, &entry(account_id, amount_minor, date)).unwrap()
    }

    fn october_gross(conn: &Connection, person_id: i64) -> i64 {
        gross_inflows(
            conn,
            InflowsParams {
                person_id,
                currency_id: EUR,
                events_until: AT_NOV_8.to_owned(),
                from: month("2026-10"),
                to: month("2026-10"),
            },
        )
        .unwrap()
        .get(&month("2026-10"))
        .copied()
        .unwrap_or(0)
    }

    struct Fixture {
        checking: i64,
        savings: i64,
    }

    #[test]
    fn inflows_count_only_positive_cashflows_on_the_persons_counted_accounts() {
        type Write = fn(&Connection, &Fixture) -> i64;
        let cases: [(&str, Write); 10] = [
            ("a cashflow on another counted account", |conn, f| {
                cashflow(conn, f.savings, 7_000, "2026-10-15");
                7_000
            }),
            ("a transfer between the person's accounts", |conn, f| {
                let mut transfer = entry(f.savings, -7_000, "2026-10-15");
                transfer.counterpart_account_id = Some(f.checking);
                create_transfer(conn, &transfer).unwrap();
                0
            }),
            ("a transfer from another person", |conn, f| {
                let theirs = mk(conn, "Theirs", "account", other_person(conn), EUR);
                let mut transfer = entry(theirs, -7_000, "2026-10-15");
                transfer.counterpart_account_id = Some(f.checking);
                create_transfer(conn, &transfer).unwrap();
                0
            }),
            ("another person's account", |conn, _| {
                let theirs = mk(conn, "Theirs", "account", other_person(conn), EUR);
                cashflow(conn, theirs, 7_000, "2026-10-15");
                0
            }),
            ("another currency", |conn, _| {
                let usd = mk(
                    conn,
                    "USD",
                    "account",
                    DEFAULT_PERSON,
                    currency_id(conn, "USD"),
                );
                cashflow(conn, usd, 7_000, "2026-10-15");
                0
            }),
            ("an asset-linked account", |conn, _| {
                let loan = mk(conn, "Loan", "account", DEFAULT_PERSON, EUR);
                let car = mk(conn, "Car", "asset", DEFAULT_PERSON, EUR);
                set_account_asset_links(conn, loan, &[car]).unwrap();
                cashflow(conn, loan, 7_000, "2026-10-15");
                0
            }),
            ("a deleted cashflow", |conn, f| {
                let id = cashflow(conn, f.checking, 7_000, "2026-10-15");
                delete_event(conn, id).unwrap();
                0
            }),
            ("a negative cashflow", |conn, f| {
                cashflow(conn, f.checking, -7_000, "2026-10-15");
                0
            }),
            ("an edited cashflow at its latest version", |conn, f| {
                let id = cashflow(conn, f.checking, 7_000, "2026-10-15");
                update_event(
                    conn,
                    UpdateEventParams {
                        event_id: id,
                        amount_minor: 2_000,
                        event_date: "2026-10-16".to_owned(),
                        note: None,
                    },
                )
                .unwrap();
                2_000
            }),
            ("a cashflow dated in another month", |conn, f| {
                cashflow(conn, f.checking, 7_000, "2026-09-30T23:59:59");
                cashflow(conn, f.checking, 7_000, "2026-11-01");
                0
            }),
        ];

        for (case, write) in cases {
            let conn = initialize_in_memory().unwrap();
            let fixture = Fixture {
                checking: mk(&conn, "Checking", "account", DEFAULT_PERSON, EUR),
                savings: mk(&conn, "Savings", "account", DEFAULT_PERSON, EUR),
            };
            cashflow(&conn, fixture.checking, 10_000, "2026-10-02T09:00:00");

            let added = write(&conn, &fixture);

            assert_eq!(
                october_gross(&conn, DEFAULT_PERSON),
                10_000 + added,
                "{case}"
            );
        }
    }

    fn november_budget(conn: &Connection, events_until: &str) -> Evaluation {
        let checking = mk(conn, "Checking", "account", DEFAULT_PERSON, EUR);
        cashflow(conn, checking, 50_000, "2026-11-20T10:00:00");
        evaluate_draft(
            conn,
            EvaluateDraftParams {
                person_id: DEFAULT_PERSON,
                currency_id: EUR,
                cells: vec![BudgetCell::new(month("2026-11"), Some(80_000), None).unwrap()],
                lines_for: None,
                events_until: events_until.to_owned(),
                day: nov_8(),
            },
        )
        .unwrap()
    }

    #[test]
    fn inflows_stop_at_the_moment_and_live_counts_a_payment_dated_later_this_month() {
        let at = november_budget(&initialize_in_memory().unwrap(), AT_NOV_8);
        let live = november_budget(
            &initialize_in_memory().unwrap(),
            Moment::live(nov_8()).events_until(),
        );

        assert_eq!(at.value_minor, -80_000);
        assert_eq!(live.value_minor, 50_000 - 80_000);
    }

    #[test]
    fn counted_accounts_use_the_same_predicate_as_inflows() {
        let conn = initialize_in_memory().unwrap();
        let theirs_person = other_person(&conn);
        let car = mk(&conn, "Car", "asset", DEFAULT_PERSON, EUR);
        let accounts = [
            mk(&conn, "Checking", "account", DEFAULT_PERSON, EUR),
            mk(&conn, "Savings", "account", DEFAULT_PERSON, EUR),
            mk(&conn, "Theirs", "account", theirs_person, EUR),
            mk(
                &conn,
                "USD",
                "account",
                DEFAULT_PERSON,
                currency_id(&conn, "USD"),
            ),
            mk(&conn, "Loan", "account", DEFAULT_PERSON, EUR),
        ];
        set_account_asset_links(&conn, accounts[4], &[car]).unwrap();
        for (bit, &account) in accounts.iter().enumerate() {
            cashflow(&conn, account, 1 << bit, "2026-10-15");
        }

        let gross = october_gross(&conn, DEFAULT_PERSON);
        let counted: Vec<i64> = counted_accounts(
            &conn,
            CountedAccountsParams {
                person_id: DEFAULT_PERSON,
                currency_id: EUR,
            },
        )
        .unwrap()
        .into_iter()
        .map(|account| account.id)
        .collect();

        let from_inflows: Vec<i64> = accounts
            .iter()
            .enumerate()
            .filter(|&(bit, _)| gross & (1 << bit) != 0)
            .map(|(_, &id)| id)
            .collect();
        assert_eq!(counted, from_inflows);
        assert_eq!(counted, [accounts[0], accounts[1]]);
    }

    #[test]
    fn counted_accounts_match_left_to_spends_account_roles() {
        let conn = initialize_in_memory().unwrap();
        let theirs_person = other_person(&conn);
        let car = mk(&conn, "Car", "asset", DEFAULT_PERSON, EUR);
        let loan = mk(&conn, "Loan", "account", DEFAULT_PERSON, EUR);
        set_account_asset_links(&conn, loan, &[car]).unwrap();
        mk(&conn, "Checking", "account", DEFAULT_PERSON, EUR);
        mk(&conn, "Savings", "account", DEFAULT_PERSON, EUR);
        mk(&conn, "Theirs", "account", theirs_person, EUR);
        mk(
            &conn,
            "USD",
            "account",
            DEFAULT_PERSON,
            currency_id(&conn, "USD"),
        );
        mk(&conn, "Holiday", "bucket", DEFAULT_PERSON, EUR);

        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                moment: Moment::at("2026-11-08").unwrap(),
                person_id: Some(DEFAULT_PERSON),
            },
        )
        .unwrap();
        let left_to_spend = snapshot
            .metrics
            .iter()
            .find(|metric| metric.kind() == MetricKind::LeftToSpend)
            .unwrap();
        let mut added_by_left_to_spend: Vec<i64> = left_to_spend
            .counted()
            .iter()
            .filter(|line| {
                line.subject.account_type == AccountKind::Account
                    && line.native.currency_code == "EUR"
            })
            .map(|line| line.subject.account_id)
            .collect();
        added_by_left_to_spend.sort_unstable();

        let mut counted: Vec<i64> = counted_accounts(
            &conn,
            CountedAccountsParams {
                person_id: DEFAULT_PERSON,
                currency_id: EUR,
            },
        )
        .unwrap()
        .into_iter()
        .map(|account| account.id)
        .collect();
        counted.sort_unstable();

        assert_eq!(counted.len(), 2);
        assert_eq!(counted, added_by_left_to_spend);
    }

    fn cell(text: &str, budget_minor: i64) -> BudgetCell {
        BudgetCell::new(month(text), Some(budget_minor), None).unwrap()
    }

    fn replace(conn: &Connection, bucket_id: i64, cells: &[BudgetCell]) {
        replace_cells(
            conn,
            ReplaceCellsParams {
                bucket_id,
                cells: cells.to_vec(),
            },
        )
        .unwrap();
    }

    #[test]
    fn replace_cells_is_whole_set_and_idempotent() {
        let conn = initialize_in_memory().unwrap();
        let budget = income_budget(&conn, DEFAULT_PERSON);
        let vat_only = BudgetCell::new(month("2026-12"), None, Some(2_300)).unwrap();

        replace(&conn, budget, &[cell("2026-10", 1), cell("2026-11", 2)]);
        replace(&conn, budget, &[cell("2026-11", 3), vat_only]);
        let once = list_cells(&conn, budget).unwrap();
        replace(&conn, budget, &[cell("2026-11", 3), vat_only]);

        assert_eq!(once, [cell("2026-11", 3), vat_only]);
        assert_eq!(list_cells(&conn, budget).unwrap(), once);
        replace(&conn, budget, &[]);
        assert_eq!(list_cells(&conn, budget).unwrap(), []);
    }

    fn stored_months(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM income_budget_month", [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    #[test]
    fn deleting_the_bucket_or_its_person_deletes_its_months() {
        let conn = initialize_in_memory().unwrap();
        let mine = income_budget(&conn, DEFAULT_PERSON);
        let person = other_person(&conn);
        let theirs = income_budget(&conn, person);
        replace(&conn, mine, &[cell("2026-11", 1)]);
        replace(&conn, theirs, &[cell("2026-11", 1), cell("2026-12", 1)]);
        assert_eq!(stored_months(&conn), 3);

        delete_account(&conn, mine).unwrap();
        assert_eq!(stored_months(&conn), 2);
        delete_person(&conn, person).unwrap();
        assert_eq!(stored_months(&conn), 0);
    }
}
