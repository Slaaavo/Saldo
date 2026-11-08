use std::collections::BTreeSet;

use rusqlite::Connection;
use serde::{Deserialize, Deserializer};
use tauri::State;

use super::calculation::{BudgetCell, YearMonth};
use super::models::{IncomeBudget, IncomeBudgetPreview};
use super::repository::{self, CountedAccountsParams, EvaluateDraftParams, ReplaceCellsParams};
use crate::error::AppError;
use crate::features::accounts::repository::{create_account, CreateAccountParams};
use crate::features::buckets::repository::{get_bucket_facts, BucketFacts};
use crate::features::buckets::BucketKind;
use crate::features::currency::repository::list_currencies;
use crate::features::persons::repository::list_persons;
use crate::features::transactions::repository::{value_amount, ValueAmountParams};
use crate::features::transactions::AsOfInput;
use crate::shared::with_savepoint_app;
use crate::AppState;

const MAX_PREVIEW_YEARS: i32 = 40;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIncomeBudgetInput {
    pub name: String,
    pub currency_id: i64,
    pub person_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetIncomeBudgetInput {
    pub bucket_id: i64,
    pub as_of: AsOfInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetCellInput {
    pub month: String,
    #[serde(deserialize_with = "present")]
    pub budget_minor: Option<i64>,
    #[serde(deserialize_with = "present")]
    pub vat_bps: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewIncomeBudgetInput {
    pub bucket_id: i64,
    pub as_of: AsOfInput,
    pub from_year: i32,
    pub to_year: i32,
    pub months: Vec<BudgetCellInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveIncomeBudgetInput {
    pub bucket_id: i64,
    pub months: Vec<BudgetCellInput>,
}

/// The field must be sent, null when empty. Serde fills a missing `Option` with None unless the
/// field has `deserialize_with`, and a client that dropped an amount would store the month
/// without it.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    Option::deserialize(deserializer)
}

#[tauri::command]
pub fn create_income_budget(
    state: State<'_, AppState>,
    input: CreateIncomeBudgetInput,
) -> Result<i64, AppError> {
    let conn = state.conn()?;
    create_income_budget_inner(&conn, input)
}

#[tauri::command]
pub fn get_income_budget(
    state: State<'_, AppState>,
    input: GetIncomeBudgetInput,
) -> Result<IncomeBudget, AppError> {
    let conn = state.conn()?;
    get_income_budget_inner(&conn, input)
}

#[tauri::command]
pub fn preview_income_budget(
    state: State<'_, AppState>,
    input: PreviewIncomeBudgetInput,
) -> Result<IncomeBudgetPreview, AppError> {
    let conn = state.conn()?;
    preview_income_budget_inner(&conn, input)
}

#[tauri::command]
pub fn save_income_budget(
    state: State<'_, AppState>,
    input: SaveIncomeBudgetInput,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    save_income_budget_inner(&conn, input)
}

pub(crate) fn create_income_budget_inner(
    conn: &Connection,
    input: CreateIncomeBudgetInput,
) -> Result<i64, AppError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(validation("Name the income budget."));
    }
    with_savepoint_app(conn, || {
        if !list_persons(conn)?
            .iter()
            .any(|person| person.id == input.person_id)
        {
            return Err(validation("Pick the person whose income this budgets."));
        }
        if !list_currencies(conn, Some(false))?
            .iter()
            .any(|currency| currency.id == input.currency_id)
        {
            return Err(validation(
                "An income budget needs a real currency, not a custom unit.",
            ));
        }
        if repository::income_budget_of(conn, input.person_id)?.is_some() {
            return Err(one_per_person());
        }
        create_account(
            conn,
            CreateAccountParams {
                name: name.to_owned(),
                currency_id: input.currency_id,
                account_type: "bucket".to_owned(),
                person_id: Some(input.person_id),
                bucket_kind: BucketKind::IncomeBudget,
                ..Default::default()
            },
        )
    })
}

pub(crate) fn get_income_budget_inner(
    conn: &Connection,
    input: GetIncomeBudgetInput,
) -> Result<IncomeBudget, AppError> {
    let owner = require_income_budget(conn, input.bucket_id)?;
    let day = input.as_of.into_moment()?.day();
    Ok(IncomeBudget {
        day: day.format("%Y-%m-%d").to_string(),
        moment_month: YearMonth::of(day),
        months: repository::list_cells(conn, input.bucket_id)?,
        counted_accounts: repository::counted_accounts(
            conn,
            CountedAccountsParams {
                person_id: owner.person_id,
                currency_id: owner.currency_id,
            },
        )?,
    })
}

pub(crate) fn preview_income_budget_inner(
    conn: &Connection,
    input: PreviewIncomeBudgetInput,
) -> Result<IncomeBudgetPreview, AppError> {
    let owner = require_income_budget(conn, input.bucket_id)?;
    let span = year_span(input.from_year, input.to_year)?;
    let cells = parse_cells(input.months)?;
    let moment = input.as_of.into_moment()?;
    let evaluation = repository::evaluate_draft(
        conn,
        EvaluateDraftParams {
            person_id: owner.person_id,
            currency_id: owner.currency_id,
            cells,
            lines_for: Some(span),
            events_until: moment.events_until().to_owned(),
            day: moment.day(),
        },
    )?;
    let valuation = value_amount(
        conn,
        ValueAmountParams {
            moment,
            currency_id: owner.currency_id,
            amount_minor: evaluation.value_minor,
        },
    )?;
    Ok(IncomeBudgetPreview {
        value_minor: evaluation.value_minor,
        valuation,
        lines: evaluation.lines,
    })
}

pub(crate) fn save_income_budget_inner(
    conn: &Connection,
    input: SaveIncomeBudgetInput,
) -> Result<(), AppError> {
    let cells = parse_cells(input.months)?;
    with_savepoint_app(conn, || {
        require_income_budget(conn, input.bucket_id)?;
        repository::replace_cells(
            conn,
            ReplaceCellsParams {
                bucket_id: input.bucket_id,
                cells,
            },
        )?;
        Ok(())
    })
}

pub(crate) fn check_person_change(
    conn: &Connection,
    account_id: i64,
    person_id: i64,
) -> Result<(), AppError> {
    let is_income_budget = matches!(
        get_bucket_facts(conn, account_id)?,
        Some(BucketFacts {
            kind: BucketKind::IncomeBudget,
            ..
        })
    );
    match repository::income_budget_of(conn, person_id)? {
        Some(other) if is_income_budget && other != account_id => Err(one_per_person()),
        _ => Ok(()),
    }
}

struct Owner {
    person_id: i64,
    currency_id: i64,
}

fn require_income_budget(conn: &Connection, bucket_id: i64) -> Result<Owner, AppError> {
    match get_bucket_facts(conn, bucket_id)? {
        Some(BucketFacts {
            kind: BucketKind::IncomeBudget,
            person_id: Some(person_id),
            currency_id,
        }) => Ok(Owner {
            person_id,
            currency_id,
        }),
        _ => Err(validation("This bucket is not an income budget.")),
    }
}

fn year_span(from_year: i32, to_year: i32) -> Result<(YearMonth, YearMonth), AppError> {
    let known = YearMonth::FIRST_YEAR..=YearMonth::LAST_YEAR;
    if !known.contains(&from_year) || !known.contains(&to_year) || from_year > to_year {
        return Err(validation(format!(
            "The years {from_year} to {to_year} are not a span Saldo can show."
        )));
    }
    if to_year - from_year >= MAX_PREVIEW_YEARS {
        return Err(validation(format!(
            "Show at most {MAX_PREVIEW_YEARS} years at once."
        )));
    }
    Ok((
        YearMonth::first_of_year(from_year),
        YearMonth::last_of_year(to_year),
    ))
}

fn parse_cells(months: Vec<BudgetCellInput>) -> Result<Vec<BudgetCell>, AppError> {
    let mut seen = BTreeSet::new();
    months
        .into_iter()
        .map(|input| {
            let month = YearMonth::parse(&input.month)?;
            if !seen.insert(month) {
                return Err(validation(format!("{month} is typed twice.")));
            }
            BudgetCell::new(month, input.budget_minor, input.vat_bps)
        })
        .collect()
}

fn one_per_person() -> AppError {
    validation("This person already has an income budget.")
}

fn validation(message: impl Into<String>) -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::commands::{update_account_with_conn, UpdateAccountInput};
    use crate::features::assets::repository::{create_custom_unit, CreateCustomUnitParams};
    use crate::features::persons::repository::{create_person, CreatePersonParams};
    use crate::features::transactions::models::SnapshotRow;
    use crate::features::transactions::repository::{create_cashflow, rows_at, CashflowEntry};
    use serde_json::json;

    const EUR: i64 = 1;
    const DEFAULT_PERSON: i64 = 1;
    const ONE_PER_PERSON: &str = "This person already has an income budget.";

    fn at(until: &str) -> AsOfInput {
        AsOfInput::At {
            until: until.to_owned(),
        }
    }

    fn create(conn: &Connection, person_id: i64) -> Result<i64, AppError> {
        create_income_budget_inner(
            conn,
            CreateIncomeBudgetInput {
                name: "Income".to_owned(),
                currency_id: EUR,
                person_id,
            },
        )
    }

    fn person(conn: &Connection, name: &str) -> i64 {
        create_person(
            conn,
            CreatePersonParams {
                name: name.to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .unwrap()
    }

    fn account(conn: &Connection, name: &str, account_type: &str, currency_id: i64) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: name.to_owned(),
                currency_id,
                account_type: account_type.to_owned(),
                person_id: Some(DEFAULT_PERSON),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn cashflow(conn: &Connection, account_id: i64, amount_minor: i64, date: &str) {
        create_cashflow(
            conn,
            &CashflowEntry {
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
            },
        )
        .unwrap();
    }

    fn cell(month: &str, budget_minor: Option<i64>, vat_bps: Option<i64>) -> BudgetCellInput {
        BudgetCellInput {
            month: month.to_owned(),
            budget_minor,
            vat_bps,
        }
    }

    fn spec_months() -> Vec<BudgetCellInput> {
        [
            ("2026-07", 285_000),
            ("2026-08", 564_000),
            ("2026-09", 564_000),
            ("2026-10", 564_000),
            ("2026-11", 612_000),
            ("2026-12", 612_000),
        ]
        .into_iter()
        .map(|(month, budget)| cell(month, Some(budget), Some(2_300)))
        .collect()
    }

    fn save(
        conn: &Connection,
        bucket_id: i64,
        months: Vec<BudgetCellInput>,
    ) -> Result<(), AppError> {
        save_income_budget_inner(conn, SaveIncomeBudgetInput { bucket_id, months })
    }

    fn preview(
        conn: &Connection,
        bucket_id: i64,
        years: (i32, i32),
        months: Vec<BudgetCellInput>,
    ) -> Result<IncomeBudgetPreview, AppError> {
        preview_income_budget_inner(
            conn,
            PreviewIncomeBudgetInput {
                bucket_id,
                as_of: at("2026-11-08"),
                from_year: years.0,
                to_year: years.1,
                months,
            },
        )
    }

    fn row_on_nov_8(conn: &Connection, account_id: i64) -> SnapshotRow {
        rows_at(conn, "2026-11-08")
            .into_iter()
            .find(|row| row.account_id == account_id)
            .unwrap()
    }

    fn stored(conn: &Connection, bucket_id: i64) -> Vec<BudgetCell> {
        repository::list_cells(conn, bucket_id).unwrap()
    }

    fn code_and_message(err: AppError) -> (String, String) {
        (err.code, err.message)
    }

    #[test]
    fn create_requires_a_person_and_refuses_a_second_for_that_person() {
        let conn = initialize_in_memory().unwrap();

        let unknown_person = create(&conn, 99_999).unwrap_err();
        let unnamed = create_income_budget_inner(
            &conn,
            CreateIncomeBudgetInput {
                name: "  ".to_owned(),
                currency_id: EUR,
                person_id: DEFAULT_PERSON,
            },
        )
        .unwrap_err();
        let first = create(&conn, DEFAULT_PERSON).unwrap();
        let second = create(&conn, DEFAULT_PERSON).unwrap_err();
        let for_another_person = create(&conn, person(&conn, "Other"));

        assert_eq!(unknown_person.code, "VALIDATION");
        assert_eq!(unnamed.code, "VALIDATION");
        assert_eq!(
            code_and_message(second),
            ("VALIDATION".to_owned(), ONE_PER_PERSON.to_owned())
        );
        assert!(for_another_person.is_ok());
        let row = row_on_nov_8(&conn, first);
        assert_eq!(
            row.bucket.map(|bucket| bucket.kind),
            Some(BucketKind::IncomeBudget)
        );
        assert_eq!(row.person_id, Some(DEFAULT_PERSON));
        assert_eq!(row.balance_minor, 0);
    }

    #[test]
    fn create_refuses_a_custom_unit() {
        let conn = initialize_in_memory().unwrap();
        let gold = create_custom_unit(
            &conn,
            CreateCustomUnitParams {
                name: "Gold".to_owned(),
                minor_units: 3,
            },
        )
        .unwrap();

        let refused = create_income_budget_inner(
            &conn,
            CreateIncomeBudgetInput {
                name: "Income".to_owned(),
                currency_id: gold,
                person_id: DEFAULT_PERSON,
            },
        )
        .unwrap_err();

        assert_eq!(
            code_and_message(refused),
            (
                "VALIDATION".to_owned(),
                "An income budget needs a real currency, not a custom unit.".to_owned()
            )
        );
        assert!(repository::income_budget_of(&conn, DEFAULT_PERSON)
            .unwrap()
            .is_none());
    }

    #[test]
    fn changing_an_income_budgets_person_to_one_that_has_one_is_refused() {
        let conn = initialize_in_memory().unwrap();
        let other = person(&conn, "Other");
        let third = person(&conn, "Third");
        let mine = create(&conn, DEFAULT_PERSON).unwrap();
        create(&conn, other).unwrap();
        let holiday = account(&conn, "Holiday", "bucket", EUR);
        let move_to = |account_id: i64, person_id: Option<i64>| {
            update_account_with_conn(
                &conn,
                UpdateAccountInput {
                    account_id,
                    name: "Renamed".to_owned(),
                    iban: None,
                    person_id,
                    purchase_price_minor: None,
                    purchase_date: None,
                    depreciation_period_months: None,
                    asset_use: None,
                },
            )
        };

        let refused = move_to(mine, Some(other)).unwrap_err();
        let to_nobody = move_to(mine, None).unwrap_err();

        assert_eq!(
            code_and_message(refused),
            ("VALIDATION".to_owned(), ONE_PER_PERSON.to_owned())
        );
        assert_eq!(to_nobody.code, "VALIDATION");
        assert_eq!(row_on_nov_8(&conn, mine).person_id, Some(DEFAULT_PERSON));
        move_to(mine, Some(DEFAULT_PERSON)).unwrap();
        move_to(holiday, Some(other)).unwrap();
        move_to(mine, Some(third)).unwrap();
        assert_eq!(row_on_nov_8(&conn, mine).person_id, Some(third));
    }

    #[test]
    fn preview_of_the_saved_cells_matches_the_snapshot_value() {
        let conn = initialize_in_memory().unwrap();
        let consulting = account(&conn, "Consulting", "account", EUR);
        let budget = create(&conn, DEFAULT_PERSON).unwrap();
        for (amount, date) in [
            (350_550, "2026-07-09"),
            (662_601, "2026-08-14"),
            (674_163, "2026-09-10"),
            (720_411, "2026-10-12"),
        ] {
            cashflow(&conn, consulting, amount, date);
        }
        save(&conn, budget, spec_months()).unwrap();

        let saved = preview(&conn, budget, (2026, 2026), spec_months()).unwrap();
        let mut draft = spec_months();
        draft[4] = cell("2026-11", Some(0), Some(2_300));
        let changed = preview(&conn, budget, (2026, 2026), draft).unwrap();

        let row = row_on_nov_8(&conn, budget);
        assert_eq!(saved.value_minor, -631_500);
        assert_eq!(saved.value_minor, row.balance_minor);
        assert_eq!(saved.valuation, row.valuation);
        assert_eq!(changed.value_minor, -631_500 + 612_000);
        assert_eq!(stored(&conn, budget).len(), 6);
        assert_eq!(saved.lines.len(), 12);
        assert_eq!(
            serde_json::to_value(&saved).unwrap()["lines"][7],
            json!({
                "month": "2026-08",
                "started": true,
                "budgeted": true,
                "receivedMinor": 538_700,
                "differenceMinor": -25_300
            })
        );
        assert_eq!(
            serde_json::to_value(&saved).unwrap()["valuation"],
            json!({ "kind": "sameCurrency", "valueMinor": -631_500 })
        );
    }

    #[test]
    fn get_returns_the_moments_day_month_and_counted_accounts() {
        let conn = initialize_in_memory().unwrap();
        let consulting = account(&conn, "Consulting", "account", EUR);
        let usd: i64 = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |row| {
                row.get(0)
            })
            .unwrap();
        account(&conn, "Dollars", "account", usd);
        let budget = create(&conn, DEFAULT_PERSON).unwrap();
        save(
            &conn,
            budget,
            vec![cell("2026-11", Some(612_000), Some(2_300))],
        )
        .unwrap();

        let opened = get_income_budget_inner(
            &conn,
            GetIncomeBudgetInput {
                bucket_id: budget,
                as_of: at("2026-11-08T10:00:00"),
            },
        )
        .unwrap();

        assert_eq!(
            serde_json::to_value(&opened).unwrap(),
            json!({
                "day": "2026-11-08",
                "momentMonth": "2026-11",
                "months": [{ "month": "2026-11", "budgetMinor": 612_000, "vatBps": 2_300 }],
                "countedAccounts": [{ "id": consulting, "name": "Consulting" }]
            })
        );
    }

    #[test]
    fn save_refuses_a_balance_bucket() {
        let conn = initialize_in_memory().unwrap();
        let holiday = account(&conn, "Holiday", "bucket", EUR);
        let months = || vec![cell("2026-11", Some(1), None)];

        let on_balance = save(&conn, holiday, months()).unwrap_err();
        let on_unknown = save(&conn, 99_999, months()).unwrap_err();
        let preview_on_balance = preview(&conn, holiday, (2026, 2026), months()).unwrap_err();

        assert_eq!(on_balance.code, "VALIDATION");
        assert_eq!(on_unknown.code, "VALIDATION");
        assert_eq!(preview_on_balance.code, "VALIDATION");
        let months_stored: i64 = conn
            .query_row("SELECT COUNT(*) FROM income_budget_month", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(months_stored, 0);
    }

    #[test]
    fn save_refuses_a_repeated_or_bad_month_and_keeps_what_was_stored() {
        let conn = initialize_in_memory().unwrap();
        let budget = create(&conn, DEFAULT_PERSON).unwrap();
        save(&conn, budget, vec![cell("2026-11", Some(5), None)]).unwrap();
        let kept = stored(&conn, budget);

        let refusals = [
            vec![
                cell("2026-11", Some(1), None),
                cell("2026-11", Some(2), None),
            ],
            vec![cell("2026-1", Some(1), None)],
            vec![cell("2026-12", None, None)],
            vec![cell("2026-12", Some(-1), None)],
            vec![cell("2026-12", None, Some(10_001))],
        ]
        .map(|months| save(&conn, budget, months).unwrap_err());

        for err in &refusals {
            assert_eq!(err.code, "VALIDATION", "{}", err.message);
        }
        assert_eq!(refusals[0].message, "2026-11 is typed twice.");
        assert_eq!(stored(&conn, budget), kept);
    }

    #[test]
    fn a_month_sent_without_one_of_its_amounts_is_refused() {
        let missing = serde_json::from_value::<BudgetCellInput>(
            json!({ "month": "2026-11", "vatBps": 2_300 }),
        );
        let empty = serde_json::from_value::<BudgetCellInput>(
            json!({ "month": "2026-11", "budgetMinor": null, "vatBps": 2_300 }),
        )
        .unwrap();

        assert!(missing.is_err());
        assert_eq!((empty.budget_minor, empty.vat_bps), (None, Some(2_300)));
    }

    #[test]
    fn preview_shows_at_most_40_years_in_order() {
        let conn = initialize_in_memory().unwrap();
        let budget = create(&conn, DEFAULT_PERSON).unwrap();

        let forty = preview(&conn, budget, (2000, 2039), vec![]).unwrap();
        let too_many = preview(&conn, budget, (2000, 2040), vec![]).unwrap_err();
        let reversed = preview(&conn, budget, (2027, 2026), vec![]).unwrap_err();
        let unknown_year = preview(&conn, budget, (1989, 1990), vec![]).unwrap_err();

        assert_eq!(forty.lines.len(), 40 * 12);
        assert_eq!(forty.lines[0].month.to_string(), "2000-01");
        for err in [too_many, reversed, unknown_year] {
            assert_eq!(err.code, "VALIDATION");
        }
    }
}
