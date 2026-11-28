use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde_json::json;

use super::checks::verification_of;
use super::commands::{
    book_unexplained_difference_inner, confirm_bank_match_inner, delete_balance_check_inner,
    find_difference_causes_inner, BookUnexplainedDifferenceInput, ConfirmBankMatchInput,
    FindDifferenceCausesInput,
};
use super::guard::{guard_verified_checks, BreakingChecks, BrokenCheck, WriteError};
use super::models::{
    BankLine, CashflowTarget, DifferenceCause, RowTarget, TransferTarget,
    UnexplainedDifferenceTarget, Verification,
};
use super::repository::{
    balance_seen_by_check, clock_now, insert_check, insert_not_arrived,
    insert_unexplained_difference, BalanceSeenByCheckParams, InsertCheckParams,
    InsertNotArrivedParams, InsertUnexplainedDifferenceParams,
};
use crate::db::initialize_in_memory;
use crate::error::AppError;
use crate::features::accounts::repository::{create_account, CreateAccountParams};
use crate::features::buckets::BucketKind;
use crate::features::csv_import::commands::{apply_csv_import_inner, preview_csv_import_inner};
use crate::features::csv_import::models::{
    ApplyCsvImportInput, CancelledHoldChoice, IncomingRowInput, PreviewCsvImportInput, RowChoice,
    SplitLegChoice, Verdict as ImportVerdict,
};
use crate::features::csv_profiles::repository::{
    create_import_profile, set_preferred_profile, CreateImportProfileParams,
    SetPreferredProfileParams,
};
use crate::features::transactions::commands::{
    bulk_create_balance_updates_inner, create_balance_update_inner, delete_event_inner,
    delete_split_group_inner, get_accounts_snapshot_inner, update_event_inner, BulkBalanceEntry,
    BulkCreateBalanceUpdatesInput, CreateBalanceUpdateInput, UpdateEventInput,
};
use crate::features::transactions::models::{MetricKind, Snapshot, SnapshotRow, SplitGroupEntry};
use crate::features::transactions::repository::{
    create_balance_update, create_cashflow, create_split_group_with_legs, create_transfer,
    delete_event, get_accounts_snapshot, update_event, CashflowEntry, CreateBalanceUpdateParams,
    CreateSplitGroupWithLegsParams, GetSnapshotParams, Moment, UpdateEventParams,
};
use crate::shared::is_iso_datetime;

const DEFAULT_PERSON: i64 = 1;
const CHECK_DAY: &str = "2026-11-14";
const BEFORE_THE_CHECK: &str = "2026-11-14T08:00:00.000";
const CHECKED_AT: &str = "2026-11-14T09:00:00.000";
const AFTER_THE_CHECK: &str = "2026-11-14T10:00:00.000";

fn mk(conn: &Connection, account_type: &str) -> i64 {
    create_account(
        conn,
        CreateAccountParams {
            name: format!("Test {account_type}"),
            currency_id: 1,
            account_type: account_type.to_owned(),
            person_id: Some(DEFAULT_PERSON),
            ..Default::default()
        },
    )
    .unwrap()
}

fn written_at(conn: &Connection, event_id: i64, created_at: &str) {
    conn.execute(
        "UPDATE event SET created_at = ?2 WHERE id = ?1",
        params![event_id, created_at],
    )
    .unwrap();
    conn.execute(
        "UPDATE event_data SET created_at = ?2
         WHERE id = (SELECT latest_data_id FROM event WHERE id = ?1)",
        params![event_id, created_at],
    )
    .unwrap();
}

fn balance_update(
    conn: &Connection,
    account_id: i64,
    amount_minor: i64,
    date: &str,
    created_at: &str,
) -> i64 {
    let event_id = create_balance_update(
        conn,
        CreateBalanceUpdateParams {
            account_id,
            amount_minor,
            event_date: date.to_owned(),
            note: None,
        },
    )
    .unwrap();
    written_at(conn, event_id, created_at);
    event_id
}

fn cashflow(
    conn: &Connection,
    account_id: i64,
    amount_minor: i64,
    date: &str,
    created_at: &str,
) -> i64 {
    let event_id = create_cashflow(
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
    written_at(conn, event_id, created_at);
    event_id
}

fn check_at(
    conn: &Connection,
    account_id: i64,
    bank_balance_minor: i64,
    checked_at: &str,
    day: &str,
) -> rusqlite::Result<i64> {
    let check_id = insert_check(
        conn,
        InsertCheckParams {
            account_id,
            day: day.to_owned(),
            bank_balance_minor,
        },
    )?;
    conn.execute(
        "UPDATE balance_check SET checked_at = ?2 WHERE id = ?1",
        params![check_id, checked_at],
    )?;
    Ok(check_id)
}

fn check(conn: &Connection, account_id: i64, bank_balance_minor: i64) -> i64 {
    check_at(conn, account_id, bank_balance_minor, CHECKED_AT, CHECK_DAY).unwrap()
}

fn verification(conn: &Connection, account_id: i64) -> Option<Verification> {
    verification_of(conn, account_id).unwrap()
}

fn verified(check_id: i64) -> Option<Verification> {
    Some(Verification::Verified {
        check_id,
        day: CHECK_DAY.to_owned(),
    })
}

fn verified_today(check_id: i64) -> Option<Verification> {
    Some(Verification::Verified {
        check_id,
        day: crate::shared::local_now(),
    })
}

fn differs(check_id: i64, difference_minor: i64) -> Option<Verification> {
    Some(Verification::Differs {
        check_id,
        day: CHECK_DAY.to_owned(),
        difference_minor,
    })
}

struct Checked {
    conn: Connection,
    account: i64,
    spending: i64,
}

fn checked_at_7_500() -> (Checked, i64) {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(&conn, account, 10_000, "2026-11-01", BEFORE_THE_CHECK);
    let spending = cashflow(
        &conn,
        account,
        -2_500,
        "2026-11-10T12:00:00",
        BEFORE_THE_CHECK,
    );
    let check_id = check(&conn, account, 7_500);
    (
        Checked {
            conn,
            account,
            spending,
        },
        check_id,
    )
}

#[test]
fn a_check_recorded_at_the_live_balance_is_verified() {
    let (checked, check_id) = checked_at_7_500();

    assert_eq!(
        verification(&checked.conn, checked.account),
        verified(check_id)
    );
}

#[test]
fn an_account_never_checked_has_no_verification() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(&conn, account, 10_000, "2026-11-01", BEFORE_THE_CHECK);

    assert_eq!(verification(&conn, account), None);
}

#[test]
fn editing_a_counted_row_after_the_check_differs_by_exactly_the_change() {
    let (checked, check_id) = checked_at_7_500();

    update_event(
        &checked.conn,
        UpdateEventParams {
            event_id: checked.spending,
            amount_minor: -3_000,
            event_date: "2026-11-10T12:00:00".to_owned(),
            note: None,
        },
    )
    .unwrap();

    assert_eq!(
        verification(&checked.conn, checked.account),
        differs(check_id, 500),
        "Saldo now says 7 000 against the bank's 7 500"
    );
}

#[test]
fn a_row_written_later_and_dated_on_or_after_the_check_day_leaves_it_verified() {
    let (checked, check_id) = checked_at_7_500();

    cashflow(
        &checked.conn,
        checked.account,
        -1_000,
        "2026-11-14T00:00:00",
        AFTER_THE_CHECK,
    );
    cashflow(
        &checked.conn,
        checked.account,
        -400,
        "2026-11-20",
        AFTER_THE_CHECK,
    );

    assert_eq!(
        verification(&checked.conn, checked.account),
        verified(check_id)
    );
}

#[test]
fn a_row_written_later_and_dated_before_the_check_day_makes_it_differ() {
    let (checked, check_id) = checked_at_7_500();

    cashflow(
        &checked.conn,
        checked.account,
        -1_000,
        "2026-11-13T23:59:59",
        AFTER_THE_CHECK,
    );

    assert_eq!(
        verification(&checked.conn, checked.account),
        differs(check_id, 1_000),
        "Saldo was missing a row of the day before the check"
    );
}

#[test]
fn a_row_dated_after_the_check_day_that_existed_at_the_check_counts() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(&conn, account, 10_000, "2026-11-01", BEFORE_THE_CHECK);
    cashflow(&conn, account, -1_000, "2026-11-20", BEFORE_THE_CHECK);
    let check_id = check(&conn, account, 9_000);

    assert_eq!(
        verification(&conn, account),
        verified(check_id),
        "the live balance counts rows dated after today"
    );
}

#[test]
fn deleting_a_row_after_the_check_makes_it_differ() {
    let (checked, check_id) = checked_at_7_500();

    delete_event(&checked.conn, checked.spending).unwrap();

    assert_eq!(
        verification(&checked.conn, checked.account),
        differs(check_id, -2_500)
    );
}

#[test]
fn a_balance_update_written_later_counts_only_when_dated_before_the_check_day() {
    let (checked, check_id) = checked_at_7_500();

    balance_update(
        &checked.conn,
        checked.account,
        4_000,
        "2026-11-14T20:00:00",
        AFTER_THE_CHECK,
    );
    let on_the_check_day = verification(&checked.conn, checked.account);
    balance_update(
        &checked.conn,
        checked.account,
        9_000,
        "2026-11-12",
        AFTER_THE_CHECK,
    );
    let before_the_check_day = verification(&checked.conn, checked.account);

    assert_eq!(on_the_check_day, verified(check_id));
    assert_eq!(
        before_the_check_day,
        differs(check_id, -1_500),
        "9 000 on 12 Nov replaces the 1 Nov anchor and the 10 Nov spending is before it"
    );
}

#[test]
fn only_the_newest_check_decides() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(
        &conn,
        account,
        10_000,
        "2026-10-31",
        "2026-10-31T08:00:00.000",
    );
    check_at(
        &conn,
        account,
        10_000,
        "2026-11-01T09:00:00.000",
        "2026-11-01",
    )
    .unwrap();
    cashflow(
        &conn,
        account,
        -2_500,
        "2026-11-10",
        "2026-11-10T08:00:00.000",
    );
    let newest = check(&conn, account, 7_500);
    let verified_by_the_newest = verification(&conn, account);

    cashflow(&conn, account, -1_000, "2026-11-12", AFTER_THE_CHECK);

    assert_eq!(verified_by_the_newest, verified(newest));
    assert_eq!(
        verification(&conn, account),
        differs(newest, 1_000),
        "the 1 Nov check still matches, yet the newest check alone decides"
    );
}

#[test]
fn a_check_made_now_sees_the_live_snapshot_balance() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    let other = mk(&conn, "account");
    let today = chrono::Local::now().date_naive();
    let yesterday = (today - chrono::Duration::days(1)).to_string();
    let tomorrow = (today + chrono::Duration::days(1)).to_string();
    let earlier = "2000-01-01T08:00:00.000";
    let later = "2000-01-01T09:00:00.000";
    balance_update(&conn, account, 10_000, &yesterday, earlier);
    balance_update(
        &conn,
        account,
        12_000,
        &format!("{yesterday}T18:00:00"),
        earlier,
    );
    balance_update(
        &conn,
        account,
        13_000,
        &format!("{yesterday}T18:00:00"),
        later,
    );
    cashflow(&conn, account, -700, &yesterday, earlier);
    cashflow(
        &conn,
        account,
        -300,
        &format!("{yesterday}T18:00:00"),
        later,
    );
    let (sent, received) = create_transfer(
        &conn,
        &CashflowEntry {
            account_id: account,
            amount_minor: -1_500,
            event_date: format!("{today}T09:00:00"),
            note: None,
            counterpart_account_id: Some(other),
            bucket_id: None,
            original_currency_id: None,
            original_amount_minor: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
        },
    )
    .unwrap();
    written_at(&conn, sent, later);
    written_at(&conn, received, later);
    cashflow(&conn, account, -400, &tomorrow, later);

    let snapshot = live(&conn);
    let checked_at = clock_now(&conn).unwrap();
    for (id, expected_minor) in [(account, 11_100), (other, 1_500)] {
        let seen_by_check = balance_seen_by_check(
            &conn,
            BalanceSeenByCheckParams {
                account_id: id,
                check_id: None,
                checked_at: checked_at.clone(),
                day: crate::shared::local_now(),
            },
        )
        .unwrap();
        assert_eq!(row_of(&snapshot, id).balance_minor, expected_minor);
        assert_eq!(seen_by_check, expected_minor, "account {id}");
    }
}

fn events_of_type(conn: &Connection, account_id: i64, event_type: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM event WHERE account_id = ?1 AND event_type = ?2",
        params![account_id, event_type],
        |row| row.get(0),
    )
    .unwrap()
}

fn checks_of(conn: &Connection, account_id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM balance_check WHERE account_id = ?1",
        params![account_id],
        |row| row.get(0),
    )
    .unwrap()
}

fn live(conn: &Connection) -> Snapshot {
    get_accounts_snapshot_inner(
        conn,
        GetSnapshotParams {
            moment: Moment::live(chrono::Local::now().date_naive()),
            person_id: None,
        },
    )
    .unwrap()
}

fn row_of(snapshot: &Snapshot, account_id: i64) -> &SnapshotRow {
    snapshot
        .rows
        .iter()
        .find(|row| row.account_id == account_id)
        .unwrap()
}

fn book(
    conn: &Connection,
    account_id: i64,
    seen_balance_minor: i64,
    bank_balance_minor: i64,
) -> Result<super::models::BookedDifference, AppError> {
    book_unexplained_difference_inner(
        conn,
        BookUnexplainedDifferenceInput {
            account_id,
            bank_balance_minor,
            seen_balance_minor,
        },
    )
}

fn confirm(conn: &Connection, account_id: i64, seen_balance_minor: i64) -> Result<i64, AppError> {
    confirm_bank_match_inner(
        conn,
        ConfirmBankMatchInput {
            account_id,
            seen_balance_minor,
            not_arrived_event_ids: vec![],
        },
    )
}

fn confirm_not_arrived(
    conn: &Connection,
    account_id: i64,
    seen_balance_minor: i64,
    not_arrived_event_ids: Vec<i64>,
) -> Result<i64, AppError> {
    confirm_bank_match_inner(
        conn,
        ConfirmBankMatchInput {
            account_id,
            seen_balance_minor,
            not_arrived_event_ids,
        },
    )
}

fn account_at_10_000(conn: &Connection) -> i64 {
    let account = mk(conn, "account");
    create_balance_update(
        conn,
        CreateBalanceUpdateParams {
            account_id: account,
            amount_minor: 10_000,
            event_date: "2026-01-01".to_owned(),
            note: None,
        },
    )
    .unwrap();
    account
}

#[test]
fn matches_bank_records_the_live_balance_as_a_verified_check() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);

    let check_id = confirm(&conn, account, 10_000).unwrap();

    let snapshot = live(&conn);
    assert!(matches!(
        row_of(&snapshot, account).verification,
        Some(Verification::Verified { check_id: id, .. }) if id == check_id
    ));
    let stored: (i64, String) = conn
        .query_row(
            "SELECT bank_balance_minor, day FROM balance_check WHERE id = ?1",
            params![check_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(stored, (10_000, crate::shared::local_now()));
}

#[test]
fn matches_bank_refuses_a_balance_that_moved_since_the_user_looked() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);

    let err = confirm(&conn, account, 9_999).unwrap_err();

    assert_eq!(err.code, "CHECK_CHANGED");
    assert_eq!(checks_of(&conn, account), 0);
}

#[test]
fn only_an_own_account_can_be_checked() {
    let conn = initialize_in_memory().unwrap();
    let bucket = mk(&conn, "bucket");
    let asset = mk(&conn, "asset");

    for account in [bucket, asset] {
        assert_eq!(confirm(&conn, account, 0).unwrap_err().code, "VALIDATION");
        assert_eq!(book(&conn, account, 0, 100).unwrap_err().code, "VALIDATION");
    }
    assert_eq!(confirm(&conn, 9_999, 0).unwrap_err().code, "NOT_FOUND");
}

#[test]
fn undo_deletes_the_check_and_the_account_is_unchecked_again() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);
    let check_id = confirm(&conn, account, 10_000).unwrap();

    delete_balance_check_inner(&conn, check_id).unwrap();

    assert_eq!(verification(&conn, account), None);
    assert_eq!(
        delete_balance_check_inner(&conn, check_id)
            .unwrap_err()
            .code,
        "NOT_FOUND"
    );
}

#[test]
fn booking_the_difference_books_bank_minus_saldo_and_a_matching_check() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);

    let booked = book(&conn, account, 10_000, 11_250).unwrap();

    let entry: (String, i64, Option<String>, String) = conn
        .query_row(
            "SELECT e.event_type, ed.amount_minor, ed.note, ed.event_date
             FROM event e JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.id = ?1",
            params![booked.event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        (entry.0.as_str(), entry.1, entry.2),
        ("unexplained_difference", 1_250, None)
    );
    assert!(is_iso_datetime(&entry.3), "dated now: {}", entry.3);
    let snapshot = live(&conn);
    let row = row_of(&snapshot, account);
    assert_eq!(row.balance_minor, 11_250);
    assert_eq!(
        row.verification,
        Some(Verification::Verified {
            check_id: booked.check_id,
            day: crate::shared::local_now(),
        })
    );
}

#[test]
fn booking_refuses_a_moved_balance_and_a_balance_that_already_matches() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);

    assert_eq!(
        book(&conn, account, 9_999, 11_250).unwrap_err().code,
        "CHECK_CHANGED"
    );
    assert_eq!(
        book(&conn, account, 10_000, 10_000).unwrap_err().code,
        "NOTHING_TO_BOOK"
    );
    assert_eq!(events_of_type(&conn, account, "unexplained_difference"), 0);
    assert_eq!(checks_of(&conn, account), 0);
}

#[test]
fn a_second_identical_booking_books_nothing() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);
    book(&conn, account, 10_000, 11_250).unwrap();

    let again = book(&conn, account, 10_000, 11_250).unwrap_err();
    let after_a_refresh = book(&conn, account, 11_250, 11_250).unwrap_err();

    assert_eq!(again.code, "CHECK_CHANGED");
    assert_eq!(after_a_refresh.code, "NOTHING_TO_BOOK");
    assert_eq!(events_of_type(&conn, account, "unexplained_difference"), 1);
    assert_eq!(checks_of(&conn, account), 1);
}

fn unexplained_entry_dates(conn: &Connection, account_id: i64) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT ed.event_date FROM event e JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.account_id = ?1 AND e.event_type = 'unexplained_difference'",
        )
        .unwrap();
    stmt.query_map([account_id], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn booked_after_balance_update_on(
    event_date: &str,
) -> (Connection, i64, super::models::BookedDifference) {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    create_balance_update(
        &conn,
        CreateBalanceUpdateParams {
            account_id: account,
            amount_minor: 10_000,
            event_date: event_date.to_owned(),
            note: None,
        },
    )
    .unwrap();
    let booked = book(&conn, account, 10_000, 11_250).unwrap();
    (conn, account, booked)
}

#[test]
fn booking_after_a_later_balance_update_dates_the_entry_at_it() {
    let (conn, account, booked) = booked_after_balance_update_on("2099-01-01");

    assert_eq!(
        unexplained_entry_dates(&conn, account),
        vec!["2099-01-01".to_owned()]
    );
    assert_eq!(
        verification(&conn, account),
        verified_today(booked.check_id)
    );
    assert_eq!(row_of(&live(&conn), account).balance_minor, 11_250);
}

#[test]
fn booking_on_a_day_with_a_balance_update_at_23_59_counts_the_entry() {
    let tomorrow = chrono::Local::now().date_naive() + chrono::Duration::days(1);
    let (conn, account, booked) = booked_after_balance_update_on(&format!("{tomorrow}T23:59:00"));

    assert_eq!(
        unexplained_entry_dates(&conn, account),
        vec![format!("{tomorrow}T23:59:00")],
        "the entry shares the picker's 23:59 balance update and counts because it came later"
    );
    assert_eq!(
        verification(&conn, account),
        verified_today(booked.check_id)
    );
    assert_eq!(row_of(&live(&conn), account).balance_minor, 11_250);
}

#[test]
fn a_balance_update_entered_after_a_booking_replaces_it() {
    let tomorrow = chrono::Local::now().date_naive() + chrono::Duration::days(1);
    let date = format!("{tomorrow}T23:59:00");
    let (conn, account, _) = booked_after_balance_update_on(&date);

    create_balance_update(
        &conn,
        CreateBalanceUpdateParams {
            account_id: account,
            amount_minor: 12_000,
            event_date: date,
            note: None,
        },
    )
    .unwrap();

    assert_eq!(row_of(&live(&conn), account).balance_minor, 12_000);
}

#[test]
fn a_cashflow_at_the_balance_update_moment_does_not_count() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(
        &conn,
        account,
        10_000,
        "2026-11-14T23:59:00",
        "2026-11-14T10:00:00.000",
    );
    cashflow(
        &conn,
        account,
        -500,
        "2026-11-14T23:59:00",
        "2026-11-14T11:00:00.000",
    );

    assert_eq!(row_of(&live(&conn), account).balance_minor, 10_000);
}

#[test]
fn an_unexplained_difference_moves_the_balance_and_the_totals_but_is_not_income() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);
    let budget = create_account(
        &conn,
        CreateAccountParams {
            name: "Income".to_owned(),
            currency_id: 1,
            account_type: "bucket".to_owned(),
            person_id: Some(DEFAULT_PERSON),
            bucket_kind: BucketKind::IncomeBudget,
            ..Default::default()
        },
    )
    .unwrap();
    conn.execute(
        "INSERT INTO income_budget_month (bucket_id, month, budget_minor, vat_bps)
         VALUES (?1, ?2, 100000, 0)",
        params![budget, chrono::Local::now().format("%Y-%m").to_string()],
    )
    .unwrap();
    let before = live(&conn);

    book(&conn, account, 10_000, 60_000).unwrap();

    let after = live(&conn);
    assert_eq!(row_of(&after, account).balance_minor, 60_000);
    let total = |snapshot: &Snapshot| {
        snapshot
            .metrics
            .iter()
            .find(|metric| metric.kind() == MetricKind::TotalBalance)
            .unwrap()
            .total_minor()
    };
    assert_eq!(total(&after) - total(&before), 50_000);
    assert_eq!(
        row_of(&after, budget).balance_minor,
        row_of(&before, budget).balance_minor,
        "an unexplained difference is not income"
    );
    assert_eq!(row_of(&after, budget).balance_minor, -100_000);
}

#[test]
fn an_unexplained_difference_can_be_deleted_but_not_edited() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);
    let booked = book(&conn, account, 10_000, 11_250).unwrap();

    let edit = update_event(
        &conn,
        UpdateEventParams {
            event_id: booked.event_id,
            amount_minor: 1,
            event_date: "2026-11-14T10:00:00".to_owned(),
            note: None,
        },
    );
    delete_event(&conn, booked.event_id).unwrap();

    let err = edit.unwrap_err();
    assert_eq!(
        (err.code.as_str(), err.message.as_str()),
        (
            "VALIDATION",
            "An unexplained difference cannot be edited. Delete it and find the difference again."
        )
    );
    assert_eq!(row_of(&live(&conn), account).balance_minor, 10_000);
}

#[test]
fn a_live_snapshot_carries_the_verification_and_a_past_moment_does_not() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    let bucket = mk(&conn, "bucket");
    balance_update(&conn, account, 10_000, "2026-11-01", BEFORE_THE_CHECK);
    let check_id = check(&conn, account, 10_000);

    let snapshot = |moment| {
        get_accounts_snapshot_inner(
            &conn,
            GetSnapshotParams {
                moment,
                person_id: None,
            },
        )
        .unwrap()
    };
    let at = snapshot(Moment::at(CHECK_DAY).unwrap());
    let at_the_end_of_time = snapshot(Moment::at("9999-12-31").unwrap());
    let live = snapshot(Moment::live(NaiveDate::from_ymd_opt(2026, 11, 14).unwrap()));
    let from_the_repository = get_accounts_snapshot(
        &conn,
        GetSnapshotParams {
            moment: Moment::live(NaiveDate::from_ymd_opt(2026, 11, 14).unwrap()),
            person_id: None,
        },
    )
    .unwrap();

    assert_eq!(row_of(&at, account).verification, None);
    assert_eq!(row_of(&at_the_end_of_time, account).verification, None);
    assert_eq!(
        row_of(&from_the_repository, account).verification,
        None,
        "the snapshot repository leaves checks to the command"
    );
    assert_eq!(row_of(&live, bucket).verification, None);
    assert_eq!(
        serde_json::to_value(row_of(&live, account)).unwrap()["verification"],
        json!({ "state": "verified", "checkId": check_id, "day": CHECK_DAY })
    );
}

#[test]
fn differs_goes_on_the_wire_in_camel_case() {
    let differs = Verification::Differs {
        check_id: 3,
        day: CHECK_DAY.to_owned(),
        difference_minor: -1_250,
    };

    assert_eq!(
        serde_json::to_value(differs).unwrap(),
        json!({
            "state": "differs",
            "checkId": 3,
            "day": CHECK_DAY,
            "differenceMinor": -1_250
        })
    );
}

#[test]
fn deleting_an_account_deletes_its_checks() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);
    confirm(&conn, account, 10_000).unwrap();

    crate::features::accounts::repository::delete_account(&conn, account).unwrap();

    assert_eq!(checks_of(&conn, account), 0);
}

const NOT_AN_OWN_ACCOUNT: &str = "Only an own account can be checked against the bank.";
const NOT_SINGLE: &str = "An unexplained difference is a single entry on an own account.";

fn refusal(result: rusqlite::Result<impl std::fmt::Debug>) -> AppError {
    AppError::from(result.unwrap_err())
}

#[test]
fn the_storage_refuses_a_check_on_anything_but_an_own_account() {
    let conn = initialize_in_memory().unwrap();
    let bucket = mk(&conn, "bucket");
    let asset = mk(&conn, "asset");

    for account in [bucket, asset] {
        let err = refusal(check_at(&conn, account, 0, CHECKED_AT, CHECK_DAY));
        assert_eq!(
            (err.code.as_str(), err.message.as_str()),
            ("VALIDATION", NOT_AN_OWN_ACCOUNT)
        );
    }
}

#[test]
fn the_storage_keeps_an_unexplained_difference_a_single_entry_on_an_own_account() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    let other = mk(&conn, "account");
    let bucket = mk(&conn, "bucket");
    conn.execute("INSERT INTO split_group (note) VALUES (NULL)", [])
        .unwrap();
    let split_group_id = conn.last_insert_rowid();
    let entry = cashflow(&conn, other, 100, "2026-11-14", BEFORE_THE_CHECK);

    let in_a_split = conn.execute(
        "INSERT INTO event (account_id, event_type, split_group_id)
         VALUES (?1, 'unexplained_difference', ?2)",
        params![account, split_group_id],
    );
    let linked = conn.execute(
        "INSERT INTO event (account_id, event_type, linked_event_id)
         VALUES (?1, 'unexplained_difference', ?2)",
        params![account, entry],
    );
    let on_a_bucket = insert_unexplained_difference(
        &conn,
        InsertUnexplainedDifferenceParams {
            account_id: bucket,
            amount_minor: 100,
            event_date: "2026-11-14T10:00:00".to_owned(),
        },
    );
    let single = insert_unexplained_difference(
        &conn,
        InsertUnexplainedDifferenceParams {
            account_id: account,
            amount_minor: 100,
            event_date: "2026-11-14T10:00:00".to_owned(),
        },
    )
    .unwrap();
    let joined_later = conn.execute(
        "UPDATE event SET split_group_id = ?2 WHERE id = ?1",
        params![single, split_group_id],
    );

    for (case, err) in [
        ("split group", refusal(in_a_split)),
        ("linked", refusal(linked)),
        ("bucket", refusal(on_a_bucket)),
        ("split group later", refusal(joined_later)),
    ] {
        assert_eq!(
            (err.code.as_str(), err.message.as_str()),
            ("VALIDATION", NOT_SINGLE),
            "{case}"
        );
    }
}

fn causes(
    conn: &Connection,
    account_id: i64,
    seen_balance_minor: i64,
    bank_balance_minor: i64,
) -> Result<Vec<DifferenceCause>, AppError> {
    find_difference_causes_inner(
        conn,
        FindDifferenceCausesInput {
            account_id,
            bank_balance_minor,
            seen_balance_minor,
        },
    )
}

fn bank_line<T>(target: T, amount_minor: i64, event_date: &str) -> BankLine<T> {
    BankLine {
        target,
        event_date: event_date.to_owned(),
        amount_minor,
        note: None,
        counterpart_name: None,
    }
}

fn cashflow_line(event_id: i64, amount_minor: i64, event_date: &str) -> BankLine {
    bank_line(RowTarget::Cashflow { event_id }, amount_minor, event_date)
}

fn equals_gap(event_id: i64, amount_minor: i64, event_date: &str) -> DifferenceCause {
    DifferenceCause::EqualsGap {
        row: cashflow_line(event_id, amount_minor, event_date),
    }
}

fn live_balance(conn: &Connection, account_id: i64) -> i64 {
    row_of(&live(conn), account_id).balance_minor
}

#[test]
fn a_row_before_the_latest_balance_update_is_neither_a_cause_nor_a_twin() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    cashflow(
        &conn,
        account,
        1_250,
        "2026-11-08T12:00:00",
        AFTER_THE_CHECK,
    );
    balance_update(&conn, account, 10_000, "2026-11-10", AFTER_THE_CHECK);

    assert_eq!(causes(&conn, account, 10_000, 8_750).unwrap(), vec![]);

    let counted = cashflow(
        &conn,
        account,
        1_250,
        "2026-11-12T12:00:00",
        AFTER_THE_CHECK,
    );

    assert_eq!(
        causes(&conn, account, 11_250, 10_000).unwrap(),
        vec![equals_gap(counted, 1_250, "2026-11-12T12:00:00")]
    );
}

#[test]
fn a_deleted_row_is_neither_a_cause_nor_a_twin() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(&conn, account, 10_000, "2026-11-01", BEFORE_THE_CHECK);
    let gone = cashflow(
        &conn,
        account,
        -1_250,
        "2026-11-12T12:00:00",
        AFTER_THE_CHECK,
    );
    let kept = cashflow(
        &conn,
        account,
        -1_250,
        "2026-11-12T12:00:00",
        AFTER_THE_CHECK,
    );
    delete_event(&conn, gone).unwrap();

    assert_eq!(
        causes(&conn, account, 8_750, 10_000).unwrap(),
        vec![equals_gap(kept, -1_250, "2026-11-12T12:00:00")]
    );
}

#[test]
fn suspects_are_the_rows_changed_since_the_newest_check() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(&conn, account, 10_000, "2026-11-01", BEFORE_THE_CHECK);
    check_at(
        &conn,
        account,
        10_000,
        "2026-11-13T09:00:00.000",
        "2026-11-13",
    )
    .unwrap();
    cashflow(
        &conn,
        account,
        -1_250,
        "2026-11-12T12:00:00",
        "2026-11-13T12:00:00.000",
    );
    check(&conn, account, 8_750);
    let after_newest = cashflow(
        &conn,
        account,
        -3_000,
        "2026-11-12T12:00:00",
        AFTER_THE_CHECK,
    );

    assert_eq!(causes(&conn, account, 5_750, 7_000).unwrap(), vec![]);
    assert_eq!(
        causes(&conn, account, 5_750, 8_750).unwrap(),
        vec![equals_gap(after_newest, -3_000, "2026-11-12T12:00:00")]
    );
}

#[test]
fn a_row_edited_after_the_newest_check_becomes_a_suspect() {
    let (checked, _) = checked_at_7_500();

    assert_eq!(
        causes(&checked.conn, checked.account, 7_500, 10_000).unwrap(),
        vec![]
    );

    update_event(
        &checked.conn,
        UpdateEventParams {
            event_id: checked.spending,
            amount_minor: -2_500,
            event_date: "2026-11-10T12:00:00".to_owned(),
            note: Some("edited".to_owned()),
        },
    )
    .unwrap();

    assert_eq!(
        causes(&checked.conn, checked.account, 7_500, 10_000).unwrap(),
        vec![DifferenceCause::EqualsGap {
            row: BankLine {
                note: Some("edited".to_owned()),
                ..cashflow_line(checked.spending, -2_500, "2026-11-10T12:00:00")
            }
        }]
    );
}

#[test]
fn causes_refuse_a_balance_that_moved_since_the_user_looked() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);

    assert_eq!(
        causes(&conn, account, 9_000, 8_750).unwrap_err().code,
        "CHECK_CHANGED"
    );
    assert_eq!(
        causes(&conn, account, 10_000, crate::shared::MAX_AMOUNT_MINOR + 1)
            .unwrap_err()
            .code,
        "VALIDATION"
    );
}

fn imported(conn: &Connection, account_id: i64) {
    let profile_id = create_import_profile(
        conn,
        CreateImportProfileParams {
            name: "Bank export".to_owned(),
            column_mapping_json: "{}".to_owned(),
            rules: vec![],
        },
    )
    .unwrap();
    set_preferred_profile(
        conn,
        SetPreferredProfileParams {
            account_id,
            profile_id: Some(profile_id),
        },
    )
    .unwrap();
}

fn days_ago(days: i64) -> String {
    (chrono::Local::now().date_naive() - chrono::Duration::days(days)).to_string()
}

#[test]
fn the_import_hint_shows_on_an_imported_account_whose_newest_row_is_4_days_old() {
    let conn = initialize_in_memory().unwrap();
    let stale = account_at_10_000(&conn);
    let fresh = account_at_10_000(&conn);
    let never_imported = account_at_10_000(&conn);
    imported(&conn, stale);
    imported(&conn, fresh);
    for (account, newest) in [(stale, 4), (fresh, 3), (never_imported, 30)] {
        cashflow(&conn, account, -100, &days_ago(newest), AFTER_THE_CHECK);
        cashflow(&conn, account, 100, &days_ago(-1), AFTER_THE_CHECK);
    }

    assert_eq!(
        causes(&conn, stale, 10_000, 10_000).unwrap(),
        vec![DifferenceCause::StaleImport {
            newest_day: Some(days_ago(4))
        }]
    );
    assert_eq!(causes(&conn, fresh, 10_000, 10_000).unwrap(), vec![]);
    assert_eq!(
        causes(&conn, never_imported, 10_000, 10_000).unwrap(),
        vec![]
    );
}

#[test]
fn the_import_hint_shows_on_an_imported_account_with_no_rows() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);
    imported(&conn, account);

    assert_eq!(
        causes(&conn, account, 10_000, 10_000).unwrap(),
        vec![DifferenceCause::StaleImport { newest_day: None }]
    );
}

fn leg(amount_minor: i64) -> SplitGroupEntry {
    SplitGroupEntry {
        amount_minor,
        event_date: "2026-11-12T12:00:00".to_owned(),
        note: None,
        counterpart_account_id: None,
        bucket_id: None,
        original_currency_id: None,
        original_amount_minor: None,
        fx_rate_mantissa: None,
        fx_rate_exponent: None,
    }
}

#[test]
fn a_split_group_is_one_cause_and_deleting_it_closes_the_gap() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);
    let split_group_id = create_split_group_with_legs(
        &conn,
        CreateSplitGroupWithLegsParams {
            account_id: account,
            group_note: Some("Groceries".to_owned()),
            legs: vec![leg(-1_000), leg(-250)],
        },
    )
    .unwrap();

    assert_eq!(
        causes(&conn, account, 8_750, 10_000).unwrap(),
        vec![DifferenceCause::EqualsGap {
            row: BankLine {
                target: RowTarget::SplitGroup {
                    split_group_id,
                    parts: 2
                },
                event_date: "2026-11-12T12:00:00".to_owned(),
                amount_minor: -1_250,
                note: Some("Groceries".to_owned()),
                counterpart_name: None,
            }
        }]
    );

    delete_split_group_inner(&conn, split_group_id).unwrap();

    assert_eq!(live_balance(&conn, account), 10_000);
}

#[test]
fn deleting_a_near_copy_or_flipping_a_sign_closes_the_gap() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);
    let original = cashflow(
        &conn,
        account,
        -1_250,
        "2026-11-10T12:00:00",
        BEFORE_THE_CHECK,
    );
    check(&conn, account, 8_750);
    let copy = cashflow(
        &conn,
        account,
        -1_250,
        "2026-11-12T12:00:00",
        AFTER_THE_CHECK,
    );

    assert_eq!(
        causes(&conn, account, 7_500, 8_750).unwrap(),
        vec![DifferenceCause::NearCopy {
            row: cashflow_line(copy, -1_250, "2026-11-12T12:00:00"),
            copy_of: cashflow_line(original, -1_250, "2026-11-10T12:00:00"),
        }]
    );
    delete_event_inner(&conn, copy).unwrap();
    assert_eq!(live_balance(&conn, account), 8_750);

    let refund = cashflow(&conn, account, -500, "2026-11-13T12:00:00", AFTER_THE_CHECK);
    let listed = causes(&conn, account, 8_250, 9_250).unwrap();
    let [DifferenceCause::FlippedSign { row }] = listed.as_slice() else {
        panic!("expected one flipped sign, got {listed:?}");
    };
    assert_eq!(
        *row,
        bank_line(
            CashflowTarget::Cashflow { event_id: refund },
            -500,
            "2026-11-13T12:00:00"
        )
    );
    update_event(
        &conn,
        UpdateEventParams {
            event_id: refund,
            amount_minor: -row.amount_minor,
            event_date: row.event_date.clone(),
            note: row.note.clone(),
        },
    )
    .unwrap();
    assert_eq!(live_balance(&conn, account), 9_250);
}

#[test]
fn a_booked_difference_the_real_row_explains_is_listed_first_and_deleting_it_closes_the_gap() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);
    let booked = book(&conn, account, 10_000, 8_750).unwrap();
    written_at(&conn, booked.event_id, BEFORE_THE_CHECK);
    conn.execute(
        "UPDATE balance_check SET checked_at = ?2 WHERE id = ?1",
        params![booked.check_id, CHECKED_AT],
    )
    .unwrap();
    let [booked_on] = unexplained_entry_dates(&conn, account).try_into().unwrap();
    let day_before = format!("{}T12:00:00", days_ago(1));
    let real = cashflow(&conn, account, -1_250, &day_before, AFTER_THE_CHECK);

    assert_eq!(
        causes(&conn, account, 7_500, 8_750).unwrap(),
        vec![
            DifferenceCause::BookedDifferenceExplained {
                row: bank_line(
                    UnexplainedDifferenceTarget::UnexplainedDifference {
                        event_id: booked.event_id
                    },
                    -1_250,
                    &booked_on,
                ),
            },
            equals_gap(real, -1_250, &day_before),
        ]
    );

    delete_event_inner(&conn, booked.event_id).unwrap();

    assert_eq!(live_balance(&conn, account), 8_750);
}

#[test]
fn causes_go_on_the_wire_in_camel_case() {
    let cause = DifferenceCause::NearCopy {
        row: BankLine {
            target: RowTarget::SplitGroup {
                split_group_id: 7,
                parts: 2,
            },
            event_date: "2026-11-12T12:00:00".to_owned(),
            amount_minor: -1_250,
            note: None,
            counterpart_name: Some("Lidl".to_owned()),
        },
        copy_of: BankLine {
            target: RowTarget::UnexplainedDifference { event_id: 3 },
            event_date: "2026-11-10T12:00:00".to_owned(),
            amount_minor: -1_250,
            note: Some("x".to_owned()),
            counterpart_name: None,
        },
    };

    assert_eq!(
        serde_json::to_value(cause).unwrap(),
        json!({
            "kind": "nearCopy",
            "row": {
                "target": { "kind": "splitGroup", "splitGroupId": 7, "parts": 2 },
                "eventDate": "2026-11-12T12:00:00",
                "amountMinor": -1_250,
                "note": null,
                "counterpartName": "Lidl"
            },
            "copyOf": {
                "target": { "kind": "unexplainedDifference", "eventId": 3 },
                "eventDate": "2026-11-10T12:00:00",
                "amountMinor": -1_250,
                "note": "x",
                "counterpartName": null
            }
        })
    );
    assert_eq!(
        serde_json::to_value(DifferenceCause::StaleImport { newest_day: None }).unwrap(),
        json!({ "kind": "staleImport", "newestDay": null })
    );
    assert_eq!(
        serde_json::to_value(DifferenceCause::FlippedSign {
            row: bank_line(
                CashflowTarget::Cashflow { event_id: 4 },
                -500,
                "2026-11-13T12:00:00"
            ),
        })
        .unwrap(),
        json!({
            "kind": "flippedSign",
            "row": {
                "target": { "kind": "cashflow", "eventId": 4 },
                "eventDate": "2026-11-13T12:00:00",
                "amountMinor": -500,
                "note": null,
                "counterpartName": null
            }
        })
    );
    assert_eq!(
        serde_json::to_value(DifferenceCause::BookedDifferenceExplained {
            row: bank_line(
                UnexplainedDifferenceTarget::UnexplainedDifference { event_id: 5 },
                -1_250,
                "2026-11-14T10:00:00"
            ),
        })
        .unwrap(),
        json!({
            "kind": "bookedDifferenceExplained",
            "row": {
                "target": { "kind": "unexplainedDifference", "eventId": 5 },
                "eventDate": "2026-11-14T10:00:00",
                "amountMinor": -1_250,
                "note": null,
                "counterpartName": null
            }
        })
    );
}

fn named_account(conn: &Connection, name: &str) -> i64 {
    create_account(
        conn,
        CreateAccountParams {
            name: name.to_owned(),
            currency_id: 1,
            account_type: "account".to_owned(),
            person_id: Some(DEFAULT_PERSON),
            ..Default::default()
        },
    )
    .unwrap()
}

fn edit(event_id: i64, amount_minor: i64, note: Option<&str>) -> UpdateEventInput {
    UpdateEventInput {
        event_id,
        amount_minor,
        event_date: "2026-11-10T12:00:00".to_owned(),
        note: note.map(str::to_owned),
    }
}

fn new_balance(account_id: i64, amount_minor: i64, event_date: &str) -> CreateBalanceUpdateInput {
    CreateBalanceUpdateInput {
        account_id,
        amount_minor,
        event_date: event_date.to_owned(),
        note: None,
    }
}

fn refused<T: std::fmt::Debug>(result: Result<T, WriteError>) -> Vec<BrokenCheck> {
    match result {
        Err(WriteError::BreaksVerifiedChecks(checks)) => checks,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn broken(account_id: i64, account_name: &str, difference_minor: i64) -> BrokenCheck {
    BrokenCheck {
        account_id,
        account_name: account_name.to_owned(),
        day: CHECK_DAY.to_owned(),
        difference_minor,
        currency_code: "EUR".to_owned(),
        currency_minor_units: 2,
    }
}

fn versions_of(conn: &Connection, event_id: i64) -> Vec<(i64, Option<String>)> {
    let mut stmt = conn
        .prepare("SELECT amount_minor, note FROM event_data WHERE event_id = ?1 ORDER BY id")
        .unwrap();
    let versions = stmt
        .query_map(params![event_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap();
    versions.collect::<rusqlite::Result<_>>().unwrap()
}

#[test]
fn a_change_that_makes_a_verified_check_differ_is_refused_and_leaves_the_row_as_it_was() {
    let (checked, _) = checked_at_7_500();
    let conn = &checked.conn;

    let checks = refused(guard_verified_checks(conn, BreakingChecks::Refuse, || {
        update_event_inner(conn, edit(checked.spending, -3_000, None))
    }));

    assert_eq!(checks, vec![broken(checked.account, "Test account", 500)]);
    assert_eq!(versions_of(conn, checked.spending), vec![(-2_500, None)]);
}

#[test]
fn the_same_change_allowed_is_saved_and_the_check_differs() {
    let (checked, check_id) = checked_at_7_500();
    let conn = &checked.conn;

    guard_verified_checks(conn, BreakingChecks::Allow, || {
        update_event_inner(conn, edit(checked.spending, -3_000, None))
    })
    .unwrap();

    assert_eq!(verification(conn, checked.account), differs(check_id, 500));
}

#[test]
fn a_note_edit_of_a_counted_row_is_saved_without_asking() {
    let (checked, _) = checked_at_7_500();
    let conn = &checked.conn;

    guard_verified_checks(conn, BreakingChecks::Refuse, || {
        update_event_inner(conn, edit(checked.spending, -2_500, Some("Groceries")))
    })
    .unwrap();

    assert_eq!(
        versions_of(conn, checked.spending),
        vec![(-2_500, None), (-2_500, Some("Groceries".to_owned()))]
    );
}

#[test]
fn a_change_on_an_account_whose_check_already_differs_is_saved_without_asking() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(&conn, account, 10_000, "2026-11-01", BEFORE_THE_CHECK);
    let spending = cashflow(
        &conn,
        account,
        -2_500,
        "2026-11-10T12:00:00",
        BEFORE_THE_CHECK,
    );
    let check_id = check(&conn, account, 7_000);

    guard_verified_checks(&conn, BreakingChecks::Refuse, || {
        update_event_inner(&conn, edit(spending, -2_000, None))
    })
    .unwrap();

    assert_eq!(verification(&conn, account), differs(check_id, -1_000));
}

#[test]
fn a_balance_update_dated_before_the_check_day_is_refused_and_one_on_the_day_is_not() {
    let (checked, check_id) = checked_at_7_500();
    let conn = &checked.conn;

    let checks = refused(guard_verified_checks(conn, BreakingChecks::Refuse, || {
        create_balance_update_inner(conn, new_balance(checked.account, 9_000, "2026-11-12"))
    }));
    guard_verified_checks(conn, BreakingChecks::Refuse, || {
        create_balance_update_inner(conn, new_balance(checked.account, 9_000, CHECK_DAY))
    })
    .unwrap();

    assert_eq!(
        checks,
        vec![broken(checked.account, "Test account", -1_500)]
    );
    assert_eq!(
        verification(conn, checked.account),
        verified(check_id),
        "the refused balance update was rolled back and the one on the day is not seen by the check"
    );
}

#[test]
fn deleting_a_transfer_between_two_verified_accounts_names_both() {
    let conn = initialize_in_memory().unwrap();
    let current = named_account(&conn, "Tatra");
    let savings = named_account(&conn, "Savings");
    balance_update(&conn, current, 10_000, "2026-11-01", BEFORE_THE_CHECK);
    balance_update(&conn, savings, 5_000, "2026-11-01", BEFORE_THE_CHECK);
    let (_, received) = create_transfer(
        &conn,
        &CashflowEntry {
            account_id: current,
            amount_minor: -1_000,
            event_date: "2026-11-10T12:00:00".to_owned(),
            note: None,
            counterpart_account_id: Some(savings),
            bucket_id: None,
            original_currency_id: None,
            original_amount_minor: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
        },
    )
    .unwrap();
    check(&conn, current, 9_000);
    check(&conn, savings, 6_000);

    let checks = refused(guard_verified_checks(&conn, BreakingChecks::Refuse, || {
        delete_event_inner(&conn, received)
    }));

    assert_eq!(
        checks,
        vec![
            broken(current, "Tatra", -1_000),
            broken(savings, "Savings", 1_000)
        ]
    );
}

#[test]
fn deleting_a_counted_split_group_is_refused() {
    let conn = initialize_in_memory().unwrap();
    let account = account_at_10_000(&conn);
    let split_group_id = create_split_group_with_legs(
        &conn,
        CreateSplitGroupWithLegsParams {
            account_id: account,
            group_note: None,
            legs: vec![leg(-1_000), leg(-250)],
        },
    )
    .unwrap();
    check(&conn, account, 8_750);

    let checks = refused(guard_verified_checks(&conn, BreakingChecks::Refuse, || {
        delete_split_group_inner(&conn, split_group_id)
    }));

    assert_eq!(checks, vec![broken(account, "Test account", -1_250)]);
}

#[test]
fn a_bulk_balance_update_dated_before_the_check_day_names_each_broken_account() {
    let conn = initialize_in_memory().unwrap();
    let current = named_account(&conn, "Tatra");
    let savings = named_account(&conn, "Savings");
    let cash = named_account(&conn, "Cash");
    for account in [current, savings, cash] {
        balance_update(&conn, account, 10_000, "2026-11-01", BEFORE_THE_CHECK);
        check(&conn, account, 10_000);
    }
    let entry = |account_id, amount_minor| BulkBalanceEntry {
        account_id,
        amount_minor,
    };

    let checks = refused(guard_verified_checks(&conn, BreakingChecks::Refuse, || {
        bulk_create_balance_updates_inner(
            &conn,
            BulkCreateBalanceUpdatesInput {
                entries: vec![
                    entry(current, 9_000),
                    entry(savings, 12_000),
                    entry(cash, 10_000),
                ],
                event_date: "2026-11-12".to_owned(),
                note: None,
            },
        )
    }));

    assert_eq!(
        checks,
        vec![
            broken(current, "Tatra", 1_000),
            broken(savings, "Savings", -2_000)
        ]
    );
}

#[test]
fn a_write_that_fails_on_its_own_returns_its_error_as_code_and_message() {
    let (checked, _) = checked_at_7_500();
    let conn = &checked.conn;

    let error = guard_verified_checks(conn, BreakingChecks::Refuse, || {
        update_event_inner(
            conn,
            UpdateEventInput {
                event_date: "2026-11-1".to_owned(),
                ..edit(checked.spending, -3_000, None)
            },
        )
    })
    .unwrap_err();

    assert!(matches!(&error, WriteError::App(_)), "{error:?}");
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        json!({
            "code": "VALIDATION",
            "message": "event_date must be a valid date in YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS format"
        })
    );
}

#[test]
fn a_refusal_goes_on_the_wire_with_the_broken_checks() {
    let refusal = WriteError::BreaksVerifiedChecks(vec![BrokenCheck {
        account_id: 3,
        account_name: "Tatra".to_owned(),
        day: "2026-11-14".to_owned(),
        difference_minor: -1_250,
        currency_code: "EUR".to_owned(),
        currency_minor_units: 2,
    }]);

    assert_eq!(
        serde_json::to_value(refusal).unwrap(),
        json!({
            "code": "BREAKS_VERIFIED_CHECKS",
            "message": "This change makes a verified bank check differ.",
            "checks": [{
                "accountId": 3,
                "accountName": "Tatra",
                "day": "2026-11-14",
                "differenceMinor": -1_250,
                "currencyCode": "EUR",
                "currencyMinorUnits": 2
            }]
        })
    );
}

const DEBIT_CARD: &str = "423473******0501";
/// Long enough for `created_at` and a check's `checked_at` to order the writes.
const CLOCK_TICK: std::time::Duration = std::time::Duration::from_millis(3);
const BEFORE_ANY_IMPORT_CLOCK: &str = "2000-01-01T08:00:00.000";
const CHECKED_BEFORE_THE_IMPORTS: &str = "2000-01-01T09:00:00.000";
const HOLD_CHECK_DAY: &str = "2026-04-29";

fn verified_on_hold_check_day(check_id: i64) -> Option<Verification> {
    Some(Verification::Verified {
        check_id,
        day: HOLD_CHECK_DAY.to_owned(),
    })
}

fn differs_on_hold_check_day(check_id: i64, difference_minor: i64) -> Option<Verification> {
    Some(Verification::Differs {
        check_id,
        day: HOLD_CHECK_DAY.to_owned(),
        difference_minor,
    })
}

fn live_cashflows(conn: &Connection, account_id: i64) -> Vec<i64> {
    conn.prepare(
        "SELECT id FROM event
         WHERE account_id = ?1 AND event_type = 'cashflow' AND deleted_at IS NULL
         ORDER BY id",
    )
    .unwrap()
    .query_map(params![account_id], |row| row.get(0))
    .unwrap()
    .collect::<rusqlite::Result<_>>()
    .unwrap()
}

fn card_row(index: u32, day: &str, amount_minor: i64, pending: bool) -> IncomingRowInput {
    IncomingRowInput {
        index,
        event_date: format!("{day}T12:00:00"),
        amount_minor,
        note: Some(
            if pending {
                "Platba kartou"
            } else {
                "GITHUB, INC."
            }
            .to_owned(),
        ),
        counterpart_account_id: None,
        names_unknown_account: false,
        original_currency_id: None,
        original_amount_minor: None,
        fx_rate_mantissa: None,
        fx_rate_exponent: None,
        card: Some(DEBIT_CARD.to_owned()),
        pending,
    }
}

fn import_with_default_ticks_removing_cancelled_holds(
    conn: &Connection,
    account_id: i64,
    rows: &[IncomingRowInput],
) {
    let reviewed = preview_csv_import_inner(
        conn,
        PreviewCsvImportInput {
            account_id,
            rows: rows.to_vec(),
        },
    )
    .unwrap();
    let choices = reviewed
        .rows
        .iter()
        .map(|row| RowChoice {
            index: row.index,
            selected: matches!(
                row.verdict,
                ImportVerdict::New
                    | ImportVerdict::NewReplacingSplitHold { .. }
                    | ImportVerdict::HoldPostedAtOtherAmount { .. }
            ),
            bucket_id: None,
            split: None,
        })
        .collect();
    let cancelled_holds = reviewed
        .cancelled_holds
        .iter()
        .map(|hold| CancelledHoldChoice {
            event_id: hold.event_id,
            remove: true,
        })
        .collect();
    apply_csv_import_inner(
        conn,
        ApplyCsvImportInput {
            account_id,
            rows: rows.to_vec(),
            choices,
            cancelled_holds,
            review_token: reviewed.review_token,
        },
    )
    .unwrap();
}

struct HeldAtTheCheck {
    conn: Connection,
    account: i64,
    hold: i64,
    check_id: i64,
}

fn verified_with_a_pending_hold() -> HeldAtTheCheck {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(
        &conn,
        account,
        10_000,
        "2026-04-01",
        BEFORE_ANY_IMPORT_CLOCK,
    );
    import_with_default_ticks_removing_cancelled_holds(
        &conn,
        account,
        &[card_row(0, "2026-04-30", -3_300, true)],
    );
    let [hold] = live_cashflows(&conn, account)[..] else {
        panic!("expected the hold");
    };
    written_at(&conn, hold, BEFORE_ANY_IMPORT_CLOCK);
    let check_id = check_at(
        &conn,
        account,
        6_700,
        CHECKED_BEFORE_THE_IMPORTS,
        HOLD_CHECK_DAY,
    )
    .unwrap();
    assert_eq!(
        verification(&conn, account),
        verified_on_hold_check_day(check_id)
    );
    HeldAtTheCheck {
        conn,
        account,
        hold,
        check_id,
    }
}

fn hold_amount(conn: &Connection, hold: i64) -> i64 {
    conn.query_row(
        "SELECT ed.amount_minor FROM event e JOIN event_data ed ON ed.id = e.latest_data_id
         WHERE e.id = ?1",
        params![hold],
        |row| row.get(0),
    )
    .unwrap()
}

#[test]
fn a_check_stays_verified_when_its_hold_posts_at_another_amount() {
    let held = verified_with_a_pending_hold();

    import_with_default_ticks_removing_cancelled_holds(
        &held.conn,
        held.account,
        &[card_row(0, "2026-04-30", -3_299, false)],
    );

    assert_eq!(hold_amount(&held.conn, held.hold), -3_299);
    assert_eq!(
        verification(&held.conn, held.account),
        verified_on_hold_check_day(held.check_id)
    );
    assert_eq!(
        row_of(&live(&held.conn), held.account).balance_minor,
        6_701,
        "the live balance follows the booked amount"
    );
}

#[test]
fn a_check_stays_verified_when_an_import_removes_its_hold_as_cancelled() {
    let held = verified_with_a_pending_hold();

    import_with_default_ticks_removing_cancelled_holds(
        &held.conn,
        held.account,
        &[
            card_row(0, "2026-04-29", -1_000, false),
            card_row(1, "2026-05-03", -500, false),
        ],
    );

    assert_eq!(live_cashflows(&held.conn, held.account).len(), 2);
    assert!(!live_cashflows(&held.conn, held.account).contains(&held.hold));
    assert_eq!(
        verification(&held.conn, held.account),
        verified_on_hold_check_day(held.check_id)
    );
    assert_eq!(
        row_of(&live(&held.conn), held.account).balance_minor,
        8_500,
        "the live balance drops the hold"
    );
}

#[test]
fn a_check_stays_verified_when_an_import_removes_a_split_hold_as_cancelled() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(
        &conn,
        account,
        10_000,
        "2026-04-01",
        BEFORE_ANY_IMPORT_CLOCK,
    );
    let held = [card_row(0, "2026-04-30", -3_300, true)];
    let reviewed = preview_csv_import_inner(
        &conn,
        PreviewCsvImportInput {
            account_id: account,
            rows: held.to_vec(),
        },
    )
    .unwrap();
    let leg = |amount_minor| SplitLegChoice {
        amount_minor,
        note: None,
        counterpart_account_id: None,
        bucket_id: None,
    };
    apply_csv_import_inner(
        &conn,
        ApplyCsvImportInput {
            account_id: account,
            rows: held.to_vec(),
            choices: vec![RowChoice {
                index: 0,
                selected: true,
                bucket_id: None,
                split: Some(vec![leg(-3_000), leg(-300)]),
            }],
            cancelled_holds: vec![],
            review_token: reviewed.review_token,
        },
    )
    .unwrap();
    let legs = live_cashflows(&conn, account);
    for &leg in &legs {
        written_at(&conn, leg, BEFORE_ANY_IMPORT_CLOCK);
    }
    let check_id = check_at(
        &conn,
        account,
        6_700,
        CHECKED_BEFORE_THE_IMPORTS,
        HOLD_CHECK_DAY,
    )
    .unwrap();

    import_with_default_ticks_removing_cancelled_holds(
        &conn,
        account,
        &[
            card_row(0, "2026-04-29", -1_000, false),
            card_row(1, "2026-05-03", -500, false),
        ],
    );

    assert_eq!(legs.len(), 2);
    assert!(legs
        .iter()
        .all(|leg| !live_cashflows(&conn, account).contains(leg)));
    assert_eq!(
        verification(&conn, account),
        verified_on_hold_check_day(check_id)
    );
}

#[test]
fn a_hold_the_user_deleted_by_hand_stays_gone_after_a_newer_import() {
    let held = verified_with_a_pending_hold();
    delete_event(&held.conn, held.hold).unwrap();
    let differs = verification(&held.conn, held.account);

    import_with_default_ticks_removing_cancelled_holds(
        &held.conn,
        held.account,
        &[card_row(0, "2026-05-03", -500, false)],
    );

    assert_eq!(differs, differs_on_hold_check_day(held.check_id, -3_300));
    assert_eq!(verification(&held.conn, held.account), differs);
}

#[test]
fn a_hold_the_user_deletes_by_hand_still_makes_the_check_differ() {
    let held = verified_with_a_pending_hold();

    delete_event(&held.conn, held.hold).unwrap();

    assert_eq!(
        verification(&held.conn, held.account),
        differs_on_hold_check_day(held.check_id, -3_300)
    );
}

#[test]
fn an_edit_after_the_posting_still_makes_the_check_differ() {
    let held = verified_with_a_pending_hold();
    import_with_default_ticks_removing_cancelled_holds(
        &held.conn,
        held.account,
        &[card_row(0, "2026-04-30", -3_299, false)],
    );

    update_event(
        &held.conn,
        UpdateEventParams {
            event_id: held.hold,
            amount_minor: -3_000,
            event_date: "2026-04-30T12:00:00".to_owned(),
            note: None,
        },
    )
    .unwrap();

    assert_eq!(
        verification(&held.conn, held.account),
        differs_on_hold_check_day(held.check_id, -299),
        "the check saw 67.00; Saldo now counts the hold at 30.00 less the posting's 0.01"
    );
}

#[test]
fn a_check_made_after_the_posting_counts_the_booked_amount() {
    let held = verified_with_a_pending_hold();
    import_with_default_ticks_removing_cancelled_holds(
        &held.conn,
        held.account,
        &[card_row(0, "2026-04-30", -3_299, false)],
    );
    let checked_at = clock_now(&held.conn).unwrap();
    let seen = |checked_at: &str| {
        balance_seen_by_check(
            &held.conn,
            BalanceSeenByCheckParams {
                account_id: held.account,
                check_id: None,
                checked_at: checked_at.to_owned(),
                day: HOLD_CHECK_DAY.to_owned(),
            },
        )
        .unwrap()
    };

    assert_eq!(seen(&checked_at), 6_701);
    assert_eq!(seen(CHECKED_BEFORE_THE_IMPORTS), 6_700);
}

#[test]
fn a_check_stays_verified_when_a_hold_imported_after_it_is_booked_on_the_checks_day() {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(
        &conn,
        account,
        10_000,
        "2026-04-01",
        BEFORE_ANY_IMPORT_CLOCK,
    );
    let check_id = check_at(
        &conn,
        account,
        6_700,
        CHECKED_BEFORE_THE_IMPORTS,
        HOLD_CHECK_DAY,
    )
    .unwrap();
    import_with_default_ticks_removing_cancelled_holds(
        &conn,
        account,
        &[card_row(0, "2026-04-28", -3_300, true)],
    );
    let with_the_hold = verification(&conn, account);

    import_with_default_ticks_removing_cancelled_holds(
        &conn,
        account,
        &[
            card_row(0, "2026-04-29", -3_300, false),
            card_row(1, "2026-04-30", -500, false),
        ],
    );

    assert_eq!(with_the_hold, verified_on_hold_check_day(check_id));
    assert_eq!(
        verification(&conn, account),
        verified_on_hold_check_day(check_id)
    );
}

/// A hold on `held_on`, imported and checked at 67.00 on `HOLD_CHECK_DAY` while pending, split
/// into two parts or not.
fn checked_with_a_hold_on(held_on: &str, split: bool) -> (Connection, i64, i64) {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(
        &conn,
        account,
        10_000,
        "2026-04-01",
        BEFORE_ANY_IMPORT_CLOCK,
    );
    let held = [card_row(0, held_on, -3_300, true)];
    let reviewed = preview_csv_import_inner(
        &conn,
        PreviewCsvImportInput {
            account_id: account,
            rows: held.to_vec(),
        },
    )
    .unwrap();
    let leg = |amount_minor| SplitLegChoice {
        amount_minor,
        note: None,
        counterpart_account_id: None,
        bucket_id: None,
    };
    apply_csv_import_inner(
        &conn,
        ApplyCsvImportInput {
            account_id: account,
            rows: held.to_vec(),
            choices: vec![RowChoice {
                index: 0,
                selected: true,
                bucket_id: None,
                split: split.then(|| vec![leg(-3_000), leg(-300)]),
            }],
            cancelled_holds: vec![],
            review_token: reviewed.review_token,
        },
    )
    .unwrap();
    for event_id in live_cashflows(&conn, account) {
        written_at(&conn, event_id, BEFORE_ANY_IMPORT_CLOCK);
    }
    let check_id = check_at(
        &conn,
        account,
        6_700,
        CHECKED_BEFORE_THE_IMPORTS,
        HOLD_CHECK_DAY,
    )
    .unwrap();
    assert_eq!(
        verification(&conn, account),
        verified_on_hold_check_day(check_id)
    );
    (conn, account, check_id)
}

#[test]
fn a_check_stays_verified_when_a_split_hold_from_before_its_day_is_booked_at_another_amount() {
    for split in [false, true] {
        let (conn, account, check_id) = checked_with_a_hold_on("2026-04-28", split);

        import_with_default_ticks_removing_cancelled_holds(
            &conn,
            account,
            &[
                card_row(0, "2026-04-28", -3_299, false),
                card_row(1, "2026-05-03", -500, false),
            ],
        );

        assert_eq!(
            row_of(&live(&conn), account).balance_minor,
            10_000 - 3_299 - 500,
            "split={split}"
        );
        assert_eq!(
            verification(&conn, account),
            verified_on_hold_check_day(check_id),
            "split={split}"
        );
    }
}

#[test]
fn a_check_stays_verified_when_a_split_hold_from_its_day_or_later_is_booked_at_another_amount() {
    for held_on in [HOLD_CHECK_DAY, "2026-04-30"] {
        for split in [false, true] {
            let (conn, account, check_id) = checked_with_a_hold_on(held_on, split);

            import_with_default_ticks_removing_cancelled_holds(
                &conn,
                account,
                &[
                    card_row(0, held_on, -3_299, false),
                    card_row(1, "2026-05-03", -500, false),
                ],
            );

            assert_eq!(
                verification(&conn, account),
                verified_on_hold_check_day(check_id),
                "held on {held_on}, split={split}"
            );
        }
    }
}

#[test]
fn a_check_stays_verified_whatever_order_the_export_lists_a_hold_booked_a_day_later() {
    for newest_first in [false, true] {
        let conn = initialize_in_memory().unwrap();
        let account = mk(&conn, "account");
        balance_update(
            &conn,
            account,
            10_000,
            "2026-10-01",
            BEFORE_ANY_IMPORT_CLOCK,
        );
        import_with_default_ticks_removing_cancelled_holds(
            &conn,
            account,
            &[card_row(0, "2026-10-03", -50, true)],
        );
        for event_id in live_cashflows(&conn, account) {
            written_at(&conn, event_id, BEFORE_ANY_IMPORT_CLOCK);
        }
        let check_id = check_at(
            &conn,
            account,
            9_950,
            CHECKED_BEFORE_THE_IMPORTS,
            "2026-10-05",
        )
        .unwrap();
        let booked_on_4_oct = card_row(0, "2026-10-04", -50, false);
        let bought_and_booked_on_5_oct = card_row(1, "2026-10-05", -50, false);
        let export = if newest_first {
            vec![
                IncomingRowInput {
                    index: 0,
                    ..bought_and_booked_on_5_oct
                },
                IncomingRowInput {
                    index: 1,
                    ..booked_on_4_oct
                },
            ]
        } else {
            vec![booked_on_4_oct, bought_and_booked_on_5_oct]
        };

        import_with_default_ticks_removing_cancelled_holds(&conn, account, &export);

        assert_eq!(
            verification(&conn, account),
            Some(Verification::Verified {
                check_id,
                day: "2026-10-05".to_owned()
            }),
            "newest_first={newest_first}"
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum CheckedWhile {
    Pending,
    Removed,
}

/// A hold on `held_on` and a Lidl row a day before it, a check on `HOLD_CHECK_DAY` while the
/// hold was pending or after an import removed it, then an export as new as the removing one
/// that lists the hold again. Returns the check's state after the removal and after the return.
fn checked_around_a_hold_listed_again(
    held_on: &str,
    checked_while: CheckedWhile,
) -> (Option<Verification>, Option<Verification>, i64) {
    let conn = initialize_in_memory().unwrap();
    let account = mk(&conn, "account");
    balance_update(
        &conn,
        account,
        10_000,
        "2026-04-01",
        BEFORE_ANY_IMPORT_CLOCK,
    );
    let hold = card_row(0, held_on, -3_300, true);
    let lidl = card_row(1, "2026-04-27", -1_000, false);
    import_with_default_ticks_removing_cancelled_holds(
        &conn,
        account,
        &[hold.clone(), lidl.clone()],
    );
    for event_id in live_cashflows(&conn, account) {
        written_at(&conn, event_id, BEFORE_ANY_IMPORT_CLOCK);
    }
    let check = || {
        std::thread::sleep(CLOCK_TICK);
        let check_id = match checked_while {
            CheckedWhile::Pending => check_at(
                &conn,
                account,
                5_700,
                CHECKED_BEFORE_THE_IMPORTS,
                HOLD_CHECK_DAY,
            ),
            CheckedWhile::Removed => check_at(
                &conn,
                account,
                8_500,
                &clock_now(&conn).unwrap(),
                HOLD_CHECK_DAY,
            ),
        }
        .unwrap();
        std::thread::sleep(CLOCK_TICK);
        check_id
    };
    let mut check_id = None;
    if matches!(checked_while, CheckedWhile::Pending) {
        check_id = Some(check());
    }
    let bolt = card_row(2, "2026-05-03", -500, false);
    import_with_default_ticks_removing_cancelled_holds(
        &conn,
        account,
        &[lidl.clone(), bolt.clone()],
    );
    if matches!(checked_while, CheckedWhile::Removed) {
        check_id = Some(check());
    }
    let after_the_removal = verification(&conn, account);

    import_with_default_ticks_removing_cancelled_holds(&conn, account, &[hold, lidl, bolt]);

    assert_eq!(
        row_of(&live(&conn), account).balance_minor,
        10_000 - 3_300 - 1_000 - 500,
        "the hold is back once"
    );
    (
        after_the_removal,
        verification(&conn, account),
        check_id.unwrap(),
    )
}

#[test]
fn a_check_stays_verified_when_a_removed_hold_is_listed_again() {
    for held_on in ["2026-04-28", HOLD_CHECK_DAY] {
        for checked_while in [CheckedWhile::Pending, CheckedWhile::Removed] {
            let (after_the_removal, after_the_return, check_id) =
                checked_around_a_hold_listed_again(held_on, checked_while);
            let case = format!("held on {held_on}, checked while {checked_while:?}");

            assert_eq!(
                after_the_removal,
                verified_on_hold_check_day(check_id),
                "{case}"
            );
            assert_eq!(
                after_the_return,
                verified_on_hold_check_day(check_id),
                "{case}"
            );
        }
    }
}

fn transfer_row(day: &str, amount_minor: i64, counterpart_account_id: i64) -> IncomingRowInput {
    IncomingRowInput {
        note: Some("Prevod".to_owned()),
        counterpart_account_id: Some(counterpart_account_id),
        card: None,
        ..card_row(0, day, amount_minor, false)
    }
}

fn transfer_legs(conn: &Connection, account_id: i64) -> Vec<i64> {
    conn.prepare(
        "SELECT id FROM event
         WHERE account_id = ?1 AND event_type = 'transfer' AND deleted_at IS NULL
         ORDER BY id",
    )
    .unwrap()
    .query_map(params![account_id], |row| row.get(0))
    .unwrap()
    .collect::<rusqlite::Result<_>>()
    .unwrap()
}

struct OnItsWay {
    conn: Connection,
    sender: i64,
    receiver: i64,
    leg: i64,
}

fn sent_yesterday_and_imported_from_the_sender() -> OnItsWay {
    let conn = initialize_in_memory().unwrap();
    let sender = account_at_10_000(&conn);
    let receiver = account_at_10_000(&conn);
    import_with_default_ticks_removing_cancelled_holds(
        &conn,
        sender,
        &[transfer_row(&days_ago(1), -1_500, receiver)],
    );
    let [leg] = transfer_legs(&conn, receiver)[..] else {
        panic!("the sender's import adds one leg on the receiver");
    };
    OnItsWay {
        conn,
        sender,
        receiver,
        leg,
    }
}

#[test]
fn a_transfer_the_receiving_bank_has_not_listed_is_named_as_not_arrived() {
    let way = sent_yesterday_and_imported_from_the_sender();

    let found = causes(&way.conn, way.receiver, 11_500, 10_000).unwrap();

    assert!(
        matches!(
            found.as_slice(),
            [DifferenceCause::NotArrived { row }]
                if row.target == TransferTarget::Transfer { event_id: way.leg }
                    && row.amount_minor == 1_500
        ),
        "{found:?}"
    );
}

#[test]
fn a_check_leaving_out_a_transfer_not_arrived_stays_verified_after_the_receiving_import() {
    let way = sent_yesterday_and_imported_from_the_sender();

    let check_id = confirm_not_arrived(&way.conn, way.receiver, 11_500, vec![way.leg]).unwrap();
    assert_eq!(
        way.conn
            .query_row(
                "SELECT bank_balance_minor FROM balance_check WHERE id = ?1",
                params![check_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        10_000
    );
    assert_eq!(
        verification(&way.conn, way.receiver),
        verified_today(check_id)
    );

    std::thread::sleep(CLOCK_TICK);
    import_with_default_ticks_removing_cancelled_holds(
        &way.conn,
        way.receiver,
        &[transfer_row(&days_ago(0), 1_500, way.sender)],
    );

    assert_eq!(transfer_legs(&way.conn, way.receiver), vec![way.leg]);
    assert_eq!(live_balance(&way.conn, way.receiver), 11_500);
    assert_eq!(
        verification(&way.conn, way.receiver),
        verified_today(check_id)
    );

    std::thread::sleep(CLOCK_TICK);
    let landed = confirm(&way.conn, way.receiver, 11_500).unwrap();
    assert_eq!(
        verification(&way.conn, way.receiver),
        verified_today(landed)
    );
}

#[test]
fn only_a_transfer_that_may_not_have_arrived_can_be_left_out() {
    let way = sent_yesterday_and_imported_from_the_sender();
    let [sent] = transfer_legs(&way.conn, way.sender)[..] else {
        panic!("the sender has one leg");
    };
    let listed = {
        let way = sent_yesterday_and_imported_from_the_sender();
        import_with_default_ticks_removing_cancelled_holds(
            &way.conn,
            way.receiver,
            &[transfer_row(&days_ago(0), 1_500, way.sender)],
        );
        confirm_not_arrived(&way.conn, way.receiver, 11_500, vec![way.leg])
    };

    for (case, result) in [
        (
            "the sender's own leg",
            confirm_not_arrived(&way.conn, way.receiver, 11_500, vec![sent]),
        ),
        ("a leg the receiving import listed", listed),
    ] {
        let error = result.expect_err(case);
        assert_eq!(error.code, "CHECK_CHANGED", "{case}");
    }
    assert_eq!(checks_of(&way.conn, way.receiver), 0);
}

#[test]
fn the_storage_leaves_out_only_a_transfer_on_the_checked_account() {
    let way = sent_yesterday_and_imported_from_the_sender();
    let [sent] = transfer_legs(&way.conn, way.sender)[..] else {
        panic!("the sender has one leg");
    };
    let cash = cashflow(
        &way.conn,
        way.receiver,
        -100,
        "2026-11-01",
        BEFORE_THE_CHECK,
    );
    let check_id = check_at(&way.conn, way.receiver, 11_400, CHECKED_AT, CHECK_DAY).unwrap();

    for event_id in [sent, cash] {
        let refused = refusal(insert_not_arrived(
            &way.conn,
            InsertNotArrivedParams {
                check_id,
                event_ids: &[event_id],
            },
        ));
        assert_eq!(
            refused.message,
            "Only a transfer on the checked account can be left out of a check."
        );
    }
}
