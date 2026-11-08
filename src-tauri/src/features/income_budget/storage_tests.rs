use rusqlite::{params, Connection};

use crate::db::initialize_in_memory;
use crate::error::AppError;
use crate::features::accounts::repository::{create_account, CreateAccountParams};
use crate::features::buckets::BucketKind;
use crate::features::persons::repository::{create_person, CreatePersonParams};
use crate::features::transactions::models::SplitGroupEntry;
use crate::features::transactions::repository::{
    bulk_create_balance_updates, bulk_create_cashflows, create_balance_update, create_cashflow,
    create_split_group_with_legs, rows_at, BulkCreateBalanceUpdatesParams, CashflowEntry,
    CreateBalanceUpdateParams, CreateSplitGroupWithLegsParams,
};

const DEFAULT_PERSON: i64 = 1;
const NO_ENTRIES: &str = "An income budget is computed from its budget and takes no entries.";
const NO_TAGS: &str = "A cashflow cannot be tagged to an income budget.";

fn mk(conn: &Connection, account_type: &str, bucket_kind: BucketKind) -> Result<i64, AppError> {
    create_account(
        conn,
        CreateAccountParams {
            name: format!("{account_type} {}", bucket_kind.as_sql()),
            currency_id: 1,
            account_type: account_type.to_owned(),
            person_id: Some(DEFAULT_PERSON),
            bucket_kind,
            ..Default::default()
        },
    )
}

fn account(conn: &Connection) -> i64 {
    mk(conn, "account", BucketKind::Balance).unwrap()
}

fn income_budget(conn: &Connection) -> i64 {
    mk(conn, "bucket", BucketKind::IncomeBudget).unwrap()
}

fn events_on(conn: &Connection, account_id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM event WHERE account_id = ?1",
        params![account_id],
        |row| row.get(0),
    )
    .unwrap()
}

fn balance_update(account_id: i64) -> CreateBalanceUpdateParams {
    CreateBalanceUpdateParams {
        account_id,
        amount_minor: 10_000,
        event_date: "2026-11-08".to_owned(),
        note: None,
    }
}

fn cashflow(account_id: i64, bucket_id: Option<i64>) -> CashflowEntry {
    CashflowEntry {
        account_id,
        amount_minor: 5_000,
        event_date: "2026-11-08T10:00:00".to_owned(),
        note: None,
        counterpart_account_id: None,
        bucket_id,
        original_currency_id: None,
        original_amount_minor: None,
        fx_rate_mantissa: None,
        fx_rate_exponent: None,
    }
}

fn leg(bucket_id: Option<i64>) -> SplitGroupEntry {
    SplitGroupEntry {
        amount_minor: -2_500,
        event_date: "2026-11-08T10:00:00".to_owned(),
        note: None,
        counterpart_account_id: None,
        bucket_id,
        original_currency_id: None,
        original_amount_minor: None,
        fx_rate_mantissa: None,
        fx_rate_exponent: None,
    }
}

fn refusal(err: impl Into<AppError>) -> (String, String) {
    let err = err.into();
    (err.code, err.message)
}

fn validation(message: &str) -> (String, String) {
    ("VALIDATION".to_owned(), message.to_owned())
}

#[test]
fn an_income_budget_refuses_every_balance_update_path() {
    let conn = initialize_in_memory().unwrap();
    let budget = income_budget(&conn);

    let single = create_balance_update(&conn, balance_update(budget)).unwrap_err();
    let bulk = bulk_create_balance_updates(
        &conn,
        BulkCreateBalanceUpdatesParams {
            entries: vec![(budget, 10_000)],
            event_date: "2026-11-08".to_owned(),
            note: None,
        },
    )
    .unwrap_err();
    let with_initial_balance = create_account(
        &conn,
        CreateAccountParams {
            name: "Second".to_owned(),
            currency_id: 1,
            account_type: "bucket".to_owned(),
            person_id: Some(create_person(&conn, person("Other")).unwrap()),
            bucket_kind: BucketKind::IncomeBudget,
            initial_balance_minor: Some(0),
            ..Default::default()
        },
    )
    .unwrap_err();

    assert_eq!(refusal(single), validation(NO_ENTRIES));
    assert_eq!(refusal(bulk), validation(NO_ENTRIES));
    assert_eq!(refusal(with_initial_balance), validation(NO_ENTRIES));
    assert_eq!(events_on(&conn, budget), 0);
    let budgets: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM account WHERE bucket_kind = 'income_budget'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(budgets, 1);
}

#[test]
fn a_bulk_balance_update_naming_an_income_budget_writes_nothing() {
    let conn = initialize_in_memory().unwrap();
    let checking = account(&conn);
    let budget = income_budget(&conn);

    let result = bulk_create_balance_updates(
        &conn,
        BulkCreateBalanceUpdatesParams {
            entries: vec![(checking, 10_000), (budget, 10_000)],
            event_date: "2026-11-08".to_owned(),
            note: None,
        },
    );

    assert!(result.is_err());
    assert_eq!(events_on(&conn, checking), 0);
}

#[test]
fn an_income_budget_refuses_a_cashflow_on_itself() {
    let conn = initialize_in_memory().unwrap();
    let checking = account(&conn);
    let budget = income_budget(&conn);

    let single = create_cashflow(&conn, &cashflow(budget, None)).unwrap_err();
    let bulk = bulk_create_cashflows(&conn, &[cashflow(checking, None), cashflow(budget, None)])
        .unwrap_err();

    assert_eq!(refusal(single), validation(NO_ENTRIES));
    assert_eq!(refusal(bulk), validation(NO_ENTRIES));
    assert_eq!(events_on(&conn, checking), 0);
    assert_eq!(events_on(&conn, budget), 0);
}

#[test]
fn a_cashflow_or_split_leg_cannot_be_tagged_to_an_income_budget() {
    let conn = initialize_in_memory().unwrap();
    let checking = account(&conn);
    let budget = income_budget(&conn);

    let tagged = create_cashflow(&conn, &cashflow(checking, Some(budget))).unwrap_err();
    let split = create_split_group_with_legs(
        &conn,
        CreateSplitGroupWithLegsParams {
            account_id: checking,
            group_note: None,
            legs: vec![leg(None), leg(Some(budget))],
        },
    )
    .unwrap_err();

    assert_eq!(refusal(tagged), validation(NO_TAGS));
    assert_eq!(refusal(split), validation(NO_TAGS));
    assert_eq!(events_on(&conn, checking), 0);
    let groups: i64 = conn
        .query_row("SELECT COUNT(*) FROM split_group", [], |row| row.get(0))
        .unwrap();
    assert_eq!(groups, 0);
}

#[test]
fn a_bucket_keeps_its_kind() {
    let conn = initialize_in_memory().unwrap();
    let balance = mk(&conn, "bucket", BucketKind::Balance).unwrap();
    let budget = income_budget(&conn);
    let set_kind = |id: i64, kind: &str| {
        conn.execute(
            "UPDATE account SET bucket_kind = ?2 WHERE id = ?1",
            params![id, kind],
        )
    };

    let to_budget = set_kind(balance, "income_budget").unwrap_err();
    let to_balance = set_kind(budget, "balance").unwrap_err();

    let kept = validation("A bucket keeps the kind it was created with.");
    assert_eq!(refusal(to_budget), kept);
    assert_eq!(refusal(to_balance), kept);
    set_kind(budget, "income_budget").unwrap();
    let kinds: Vec<Option<BucketKind>> = [balance, budget]
        .iter()
        .map(|&id| {
            rows_at(&conn, "2026-11-08")
                .into_iter()
                .find(|row| row.account_id == id)
                .and_then(|row| row.bucket)
                .map(|bucket| bucket.kind)
        })
        .collect();
    assert_eq!(
        kinds,
        [Some(BucketKind::Balance), Some(BucketKind::IncomeBudget)]
    );
}

fn person(name: &str) -> CreatePersonParams {
    CreatePersonParams {
        name: name.to_owned(),
        person_type: "physical".to_owned(),
        vat_payer: false,
    }
}

#[test]
fn a_person_has_at_most_one_income_budget() {
    let conn = initialize_in_memory().unwrap();
    income_budget(&conn);
    let other = create_person(&conn, person("Other")).unwrap();

    let second = mk(&conn, "bucket", BucketKind::IncomeBudget);
    let for_other = create_account(
        &conn,
        CreateAccountParams {
            name: "Other's budget".to_owned(),
            currency_id: 1,
            account_type: "bucket".to_owned(),
            person_id: Some(other),
            bucket_kind: BucketKind::IncomeBudget,
            ..Default::default()
        },
    );

    assert_eq!(second.unwrap_err().code, "DB_ERROR");
    assert!(for_other.is_ok());
}

#[test]
fn an_income_budget_is_a_bucket_with_a_person() {
    let conn = initialize_in_memory().unwrap();

    let not_a_bucket = mk(&conn, "account", BucketKind::IncomeBudget);
    let no_person = create_account(
        &conn,
        CreateAccountParams {
            name: "Nobody's budget".to_owned(),
            currency_id: 1,
            account_type: "bucket".to_owned(),
            bucket_kind: BucketKind::IncomeBudget,
            ..Default::default()
        },
    );

    assert!(not_a_bucket.is_err());
    assert!(no_person.is_err());
}

#[test]
fn a_refused_write_is_a_validation_error_with_the_triggers_message() {
    let conn = initialize_in_memory().unwrap();
    let checking = account(&conn);
    let budget = income_budget(&conn);

    let on_budget: AppError = create_balance_update(&conn, balance_update(budget))
        .unwrap_err()
        .into();
    let tagged: AppError = create_cashflow(&conn, &cashflow(checking, Some(budget)))
        .unwrap_err()
        .into();
    let other_constraint = mk(&conn, "bucket", BucketKind::IncomeBudget).unwrap_err();

    assert_eq!(refusal(on_budget), validation(NO_ENTRIES));
    assert_eq!(refusal(tagged), validation(NO_TAGS));
    assert_eq!(other_constraint.code, "DB_ERROR");
}

#[test]
fn a_normal_bucket_still_takes_balance_updates_and_tags() {
    let conn = initialize_in_memory().unwrap();
    let checking = account(&conn);
    let bucket = mk(&conn, "bucket", BucketKind::Balance).unwrap();

    create_balance_update(&conn, balance_update(bucket)).unwrap();
    create_cashflow(&conn, &cashflow(checking, Some(bucket))).unwrap();

    let rows = rows_at(&conn, "2026-11-08");
    let row = rows.iter().find(|row| row.account_id == bucket).unwrap();
    assert_eq!(row.counted_minor, 10_000 + 5_000);
}
