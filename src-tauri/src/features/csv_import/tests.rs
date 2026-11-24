use std::thread::sleep;
use std::time::Duration;

use chrono::Local;
use rusqlite::{params, Connection};
use serde_json::json;

use super::commands::{apply_csv_import_inner, preview_csv_import_inner};
use super::models::{
    ApplyCsvImportInput, CancelledHold, CancelledHoldChoice, CsvImportPreview, CsvImportResult,
    IncomingRowInput, PreviewCsvImportInput, RowChoice, RowPreview, SplitLegChoice, Verdict,
};
use crate::db::initialize_in_memory;
use crate::error::AppError;
use crate::features::accounts::repository::{create_account, delete_account, CreateAccountParams};
use crate::features::bank::Verification;
use crate::features::buckets::BucketKind;
use crate::features::partner_accounts::repository::{
    create_partner_account, CreatePartnerAccountParams,
};
use crate::features::transactions::commands::get_accounts_snapshot_inner;
use crate::features::transactions::models::SplitGroupEntry;
use crate::features::transactions::repository::{
    create_cashflow, create_split_group_with_legs, create_transfer, delete_event,
    delete_split_group, update_event, CashflowEntry, CreateSplitGroupWithLegsParams,
    GetSnapshotParams, Moment, UpdateEventParams,
};

const DEFAULT_PERSON: i64 = 1;
const UNKNOWN_ACCOUNT_ID: i64 = 9_999;

fn own(conn: &Connection, name: &str) -> i64 {
    let id = account(conn, name, "account", BucketKind::Balance);
    conn.execute(
        "UPDATE account SET iban = ?1 WHERE id = ?2",
        params![format!("SK00TEST{id:016}"), id],
    )
    .unwrap();
    id
}

fn own_without_iban(conn: &Connection, name: &str) -> i64 {
    account(conn, name, "account", BucketKind::Balance)
}

fn account(conn: &Connection, name: &str, account_type: &str, bucket_kind: BucketKind) -> i64 {
    create_account(
        conn,
        CreateAccountParams {
            name: name.to_owned(),
            currency_id: 1,
            account_type: account_type.to_owned(),
            person_id: Some(DEFAULT_PERSON),
            bucket_kind,
            ..Default::default()
        },
    )
    .unwrap()
}

fn partner(conn: &Connection) -> i64 {
    create_partner_account(
        conn,
        CreatePartnerAccountParams {
            name: "Landlord".to_owned(),
            iban: "SK3112000000198742637541".to_owned(),
            currency_id: 1,
        },
    )
    .unwrap()
}

fn row(index: u32, day: &str, amount_minor: i64) -> IncomingRowInput {
    IncomingRowInput {
        index,
        event_date: format!("{day}T12:00:00"),
        amount_minor,
        note: Some(format!("row {index}")),
        counterpart_account_id: None,
        names_unknown_account: false,
        original_currency_id: None,
        original_amount_minor: None,
        fx_rate_mantissa: None,
        fx_rate_exponent: None,
        card: None,
        pending: false,
    }
}

fn to(counterpart_account_id: i64, row: IncomingRowInput) -> IncomingRowInput {
    IncomingRowInput {
        counterpart_account_id: Some(counterpart_account_id),
        ..row
    }
}

fn to_unknown_iban(row: IncomingRowInput) -> IncomingRowInput {
    IncomingRowInput {
        names_unknown_account: true,
        ..row
    }
}

fn cashflow_entry(account_id: i64, date: &str, amount_minor: i64) -> CashflowEntry {
    CashflowEntry {
        account_id,
        amount_minor,
        event_date: date.to_owned(),
        note: Some("typed by hand".to_owned()),
        counterpart_account_id: None,
        bucket_id: None,
        original_currency_id: None,
        original_amount_minor: None,
        fx_rate_mantissa: None,
        fx_rate_exponent: None,
    }
}

fn cashflow(conn: &Connection, account_id: i64, date: &str, amount_minor: i64) -> i64 {
    create_cashflow(conn, &cashflow_entry(account_id, date, amount_minor)).unwrap()
}

fn preview(conn: &Connection, account_id: i64, rows: &[IncomingRowInput]) -> CsvImportPreview {
    preview_csv_import_inner(
        conn,
        PreviewCsvImportInput {
            account_id,
            rows: rows.to_vec(),
        },
    )
    .unwrap()
}

fn verdicts(conn: &Connection, account_id: i64, rows: &[IncomingRowInput]) -> Vec<Verdict> {
    preview(conn, account_id, rows)
        .rows
        .into_iter()
        .map(|row| row.verdict)
        .collect()
}

fn default_choices(preview: &CsvImportPreview) -> Vec<RowChoice> {
    preview
        .rows
        .iter()
        .map(|row| RowChoice {
            index: row.index,
            selected: matches!(
                row.verdict,
                Verdict::New
                    | Verdict::NewReplacingSplitHold { .. }
                    | Verdict::CompletesTransfer { .. }
                    | Verdict::HoldPostedAtOtherAmount { .. }
            ),
            bucket_id: None,
            split: None,
        })
        .collect()
}

fn apply(
    conn: &Connection,
    account_id: i64,
    rows: &[IncomingRowInput],
    choices: Vec<RowChoice>,
    review_token: String,
) -> Result<CsvImportResult, AppError> {
    apply_csv_import_inner(
        conn,
        ApplyCsvImportInput {
            account_id,
            rows: rows.to_vec(),
            choices,
            cancelled_holds: vec![],
            review_token,
        },
    )
}

fn apply_reviewed(
    conn: &Connection,
    account_id: i64,
    rows: &[IncomingRowInput],
    reviewed: &CsvImportPreview,
    choices: Vec<RowChoice>,
    remove_cancelled_holds: bool,
) -> Result<CsvImportResult, AppError> {
    apply_csv_import_inner(
        conn,
        ApplyCsvImportInput {
            account_id,
            rows: rows.to_vec(),
            choices,
            cancelled_holds: reviewed
                .cancelled_holds
                .iter()
                .map(|hold| CancelledHoldChoice {
                    event_id: hold.event_id,
                    remove: remove_cancelled_holds,
                })
                .collect(),
            review_token: reviewed.review_token.clone(),
        },
    )
}

fn import_with_defaults(
    conn: &Connection,
    account_id: i64,
    rows: &[IncomingRowInput],
) -> CsvImportResult {
    let reviewed = preview(conn, account_id, rows);
    let choices = default_choices(&reviewed);
    apply_reviewed(conn, account_id, rows, &reviewed, choices, true).unwrap()
}

fn import(conn: &Connection, account_id: i64, rows: &[IncomingRowInput]) -> u32 {
    import_with_defaults(conn, account_id, rows).imported
}

fn everything(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT 'event ' || id || ' ' || account_id || ' ' || event_type || ' '
                    || IFNULL(deleted_at, '-') || ' ' || IFNULL(latest_data_id, '-') || ' '
                    || IFNULL(linked_event_id, '-') || ' ' || IFNULL(split_group_id, '-')
             FROM event
             UNION ALL
             SELECT 'data ' || id || ' ' || event_id || ' ' || amount_minor || ' ' || event_date
                    || ' ' || IFNULL(note, '-') || ' ' || IFNULL(counterpart_account_id, '-')
                    || ' ' || IFNULL(bucket_id, '-')
             FROM event_data
             UNION ALL
             SELECT 'bank ' || event_id || ' ' || day || ' ' || amount_minor || ' '
                    || IFNULL(text, '-') || ' ' || IFNULL(card, '-') || ' '
                    || IFNULL(hold_seen_to, '-') || ' ' || IFNULL(posted_at, '-') || ' '
                    || IFNULL(posting_change_minor, '-') || ' ' || IFNULL(held_day, '-') || ' '
                    || IFNULL(held_amount_minor, '-') || ' ' || IFNULL(cancelled_at, '-') || ' '
                    || IFNULL(cancelled_by_day, '-')
             FROM bank_entry
             UNION ALL
             SELECT 'split ' || id || ' ' || IFNULL(note, '-')
             FROM split_group
             ORDER BY 1",
        )
        .unwrap();
    stmt.query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[derive(Debug, PartialEq, Eq)]
struct Live {
    account_id: i64,
    event_type: String,
    event_date: String,
    amount_minor: i64,
    counterpart_account_id: Option<i64>,
    linked_event_id: Option<i64>,
}

fn live(conn: &Connection) -> Vec<(i64, Live)> {
    let mut stmt = conn
        .prepare(
            "SELECT e.id, e.account_id, e.event_type, ed.event_date, ed.amount_minor,
                    ed.counterpart_account_id, e.linked_event_id
             FROM event e JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.deleted_at IS NULL
             ORDER BY e.id",
        )
        .unwrap();
    stmt.query_map([], |row| {
        Ok((
            row.get(0)?,
            Live {
                account_id: row.get(1)?,
                event_type: row.get(2)?,
                event_date: row.get(3)?,
                amount_minor: row.get(4)?,
                counterpart_account_id: row.get(5)?,
                linked_event_id: row.get(6)?,
            },
        ))
    })
    .unwrap()
    .collect::<rusqlite::Result<_>>()
    .unwrap()
}

fn bank_entry(conn: &Connection, event_id: i64) -> Option<(String, i64, Option<String>)> {
    conn.query_row(
        "SELECT day, amount_minor, text FROM bank_entry WHERE event_id = ?1",
        params![event_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .ok()
}

fn in_saldo(event_id: i64, event_date: &str, note: &str) -> Verdict {
    Verdict::InSaldo {
        event_id,
        event_date: event_date.to_owned(),
        note: Some(note.to_owned()),
    }
}

fn written_at(conn: &Connection, event_id: i64, created_at: &str) {
    conn.execute(
        "UPDATE event SET created_at = ?2 WHERE id = ?1",
        params![event_id, created_at],
    )
    .unwrap();
}

#[test]
fn a_new_row_is_written_with_the_banks_facts() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let landlord = partner(&conn);

    let imported = import(
        &conn,
        checking,
        &[to(landlord, row(0, "2026-03-01", -50_000))],
    );

    assert_eq!(imported, 1);
    let [(event_id, entry)] = &live(&conn)[..] else {
        panic!("expected one entry");
    };
    assert_eq!(
        entry,
        &Live {
            account_id: checking,
            event_type: "cashflow".to_owned(),
            event_date: "2026-03-01T12:00:00".to_owned(),
            amount_minor: -50_000,
            counterpart_account_id: Some(landlord),
            linked_event_id: None,
        }
    );
    assert_eq!(
        bank_entry(&conn, *event_id),
        Some(("2026-03-01".to_owned(), -50_000, Some("row 0".to_owned())))
    );
}

#[test]
fn a_reimport_adds_nothing_and_changes_no_entry() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let savings = own(&conn, "Savings");
    let landlord = partner(&conn);
    let rows = [
        to(landlord, row(0, "2026-03-01", -50_000)),
        row(1, "2026-03-02", -1_250),
        to(savings, row(2, "2026-03-03", -10_000)),
    ];
    assert_eq!(import(&conn, checking, &rows), 3);
    let before = everything(&conn);

    let again = preview(&conn, checking, &rows);
    let imported = apply(
        &conn,
        checking,
        &rows,
        default_choices(&again),
        again.review_token.clone(),
    )
    .unwrap()
    .imported;

    assert!(again
        .rows
        .iter()
        .all(|row| matches!(row.verdict, Verdict::InSaldo { .. })));
    assert_eq!(imported, 0);
    assert_eq!(everything(&conn), before);
}

#[test]
fn a_ticked_row_already_in_saldo_is_imported_again() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let rows = [row(0, "2026-03-01", -1_250)];
    import(&conn, checking, &rows);

    let again = preview(&conn, checking, &rows);
    let choices = vec![RowChoice {
        index: 0,
        selected: true,
        bucket_id: None,
        split: None,
    }];
    let imported = apply(&conn, checking, &rows, choices, again.review_token).unwrap();

    assert_eq!(imported.imported, 1);
    let entries = live(&conn);
    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .all(|(event_id, _)| bank_entry(&conn, *event_id).is_some()));
}

#[test]
fn two_equal_rows_against_one_entry_in_saldo_match_once() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let typed = cashflow(&conn, checking, "2026-03-01T09:00:00", -500);
    let rows = [row(0, "2026-03-01", -500), row(1, "2026-03-01", -500)];

    let reviewed = preview(&conn, checking, &rows);
    let verdicts: Vec<Verdict> = reviewed.rows.iter().map(|r| r.verdict.clone()).collect();
    let imported = apply(
        &conn,
        checking,
        &rows,
        default_choices(&reviewed),
        reviewed.review_token,
    )
    .unwrap();

    assert_eq!(
        verdicts,
        [
            in_saldo(typed, "2026-03-01T09:00:00", "typed by hand"),
            Verdict::New
        ]
    );
    assert_eq!(imported.imported, 1);
    assert_eq!(live(&conn).len(), 2);
    assert_eq!(
        bank_entry(&conn, typed),
        Some(("2026-03-01".to_owned(), -500, Some("row 0".to_owned())))
    );
}

#[test]
fn an_entry_with_bank_facts_wins_over_an_older_equal_one_without() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let typed = cashflow(&conn, checking, "2026-03-01T09:00:00", -500);
    let rows = [row(0, "2026-03-01", -500)];
    let reviewed = preview(&conn, checking, &rows);
    apply(
        &conn,
        checking,
        &rows,
        vec![choice(0)],
        reviewed.review_token,
    )
    .unwrap();
    let imported = live(&conn)
        .into_iter()
        .map(|(event_id, _)| event_id)
        .find(|&event_id| event_id != typed)
        .unwrap();

    assert!(imported > typed);
    assert_eq!(
        verdicts(&conn, checking, &rows),
        [in_saldo(imported, "2026-03-01T12:00:00", "row 0")]
    );
}

#[test]
fn a_row_keeps_matching_after_the_user_edits_its_amount_or_day() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let rows = [row(0, "2026-03-01", -1_250)];
    import(&conn, checking, &rows);
    let event_id = live(&conn)[0].0;

    update_event(
        &conn,
        UpdateEventParams {
            event_id,
            amount_minor: -1_300,
            event_date: "2026-03-09T12:00:00".to_owned(),
            note: Some("lunch".to_owned()),
        },
    )
    .unwrap();

    assert_eq!(
        verdicts(&conn, checking, &rows),
        [in_saldo(event_id, "2026-03-09T12:00:00", "lunch")]
    );
}

#[test]
fn a_deleted_row_never_matches_and_comes_back_on_reimport() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let rows = [row(0, "2026-03-01", -1_250)];
    import(&conn, checking, &rows);
    delete_event(&conn, live(&conn)[0].0).unwrap();

    assert_eq!(verdicts(&conn, checking, &rows), [Verdict::New]);
    assert_eq!(import(&conn, checking, &rows), 1);
    assert_eq!(live(&conn).len(), 1);
}

fn leg(amount_minor: i64) -> SplitGroupEntry {
    SplitGroupEntry {
        amount_minor,
        event_date: "2026-03-01T09:00:00".to_owned(),
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
fn deleting_the_imported_accounts_deletes_their_bank_entries() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let savings = own(&conn, "Savings");
    import(
        &conn,
        checking,
        &[
            to(savings, row(0, "2026-03-01", -10_000)),
            row(1, "2026-03-02", -500),
        ],
    );

    delete_account(&conn, savings).unwrap();
    delete_account(&conn, checking).unwrap();

    let left: i64 = conn
        .query_row("SELECT COUNT(*) FROM bank_entry", [], |row| row.get(0))
        .unwrap();
    assert_eq!(left, 0);
}

#[test]
fn a_split_group_matches_the_sum_of_its_legs() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let split_group_id = create_split_group_with_legs(
        &conn,
        CreateSplitGroupWithLegsParams {
            account_id: checking,
            group_note: Some("groceries".to_owned()),
            legs: vec![leg(-300), leg(-200)],
        },
    )
    .unwrap();
    let lowest_leg = live(&conn)[0].0;
    let rows = [row(0, "2026-03-01", -500)];

    let reviewed = preview(&conn, checking, &rows);
    let verdict = reviewed.rows[0].verdict.clone();
    apply(
        &conn,
        checking,
        &rows,
        default_choices(&reviewed),
        reviewed.review_token,
    )
    .unwrap();

    assert_eq!(
        verdict,
        in_saldo(lowest_leg, "2026-03-01T09:00:00", "groceries")
    );
    assert_eq!(
        bank_entry(&conn, lowest_leg),
        Some(("2026-03-01".to_owned(), -500, Some("row 0".to_owned())))
    );

    delete_split_group(&conn, split_group_id).unwrap();
    assert_eq!(verdicts(&conn, checking, &rows), [Verdict::New]);
}

#[derive(Debug, PartialEq, Eq)]
struct SplitLegRow {
    amount_minor: i64,
    event_date: String,
    original_amount_minor: Option<i64>,
    fx_rate_mantissa: Option<i64>,
    counterpart_account_id: Option<i64>,
}

fn split_leg_row(conn: &Connection, event_id: i64) -> SplitLegRow {
    conn.query_row(
        "SELECT ed.amount_minor, ed.event_date, ed.original_amount_minor, ed.fx_rate_mantissa,
                ed.counterpart_account_id
         FROM event e JOIN event_data ed ON ed.id = e.latest_data_id
         WHERE e.id = ?1",
        params![event_id],
        |row| {
            Ok(SplitLegRow {
                amount_minor: row.get(0)?,
                event_date: row.get(1)?,
                original_amount_minor: row.get(2)?,
                fx_rate_mantissa: row.get(3)?,
                counterpart_account_id: row.get(4)?,
            })
        },
    )
    .unwrap()
}

#[test]
fn a_split_row_becomes_a_group_whose_legs_share_the_original_amount() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let landlord = partner(&conn);
    let rows = [IncomingRowInput {
        original_currency_id: Some(2),
        original_amount_minor: Some(-3_333),
        fx_rate_mantissa: Some(3_333),
        fx_rate_exponent: Some(-3),
        ..row(0, "2026-03-01", -1_000)
    }];
    let reviewed = preview(&conn, checking, &rows);
    let split = |amount_minor, counterpart_account_id| SplitLegChoice {
        amount_minor,
        note: Some(format!("leg {amount_minor}")),
        counterpart_account_id,
        bucket_id: None,
    };
    let choices = vec![RowChoice {
        index: 0,
        selected: true,
        bucket_id: None,
        split: Some(vec![
            split(-333, Some(landlord)),
            split(-333, None),
            split(-334, None),
        ]),
    }];

    let imported = apply(&conn, checking, &rows, choices, reviewed.review_token).unwrap();

    assert_eq!(imported.imported, 3);
    let leg_ids: Vec<i64> = conn
        .prepare("SELECT id FROM event WHERE split_group_id IS NOT NULL ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let leg = |amount_minor, original_amount_minor, counterpart_account_id| SplitLegRow {
        amount_minor,
        event_date: "2026-03-01T12:00:00".to_owned(),
        original_amount_minor: Some(original_amount_minor),
        fx_rate_mantissa: Some(3_333),
        counterpart_account_id,
    };
    assert_eq!(
        leg_ids
            .iter()
            .map(|&event_id| split_leg_row(&conn, event_id))
            .collect::<Vec<_>>(),
        [
            leg(-333, -1_110, Some(landlord)),
            leg(-333, -1_110, None),
            leg(-334, -1_113, None),
        ]
    );
    assert_eq!(
        bank_entry(&conn, leg_ids[0]),
        Some(("2026-03-01".to_owned(), -1_000, Some("row 0".to_owned())))
    );
    assert!(bank_entry(&conn, leg_ids[1]).is_none());
    assert!(matches!(
        &verdicts(&conn, checking, &rows)[..],
        [Verdict::InSaldo { event_id, .. }] if *event_id == leg_ids[0]
    ));
}

fn the_pair(conn: &Connection, from: i64, to: i64) -> ((i64, Live), (i64, Live)) {
    let entries = live(conn);
    assert_eq!(entries.len(), 2, "expected one transfer pair: {entries:?}");
    let mut entries = entries.into_iter();
    let (first, second) = (entries.next().unwrap(), entries.next().unwrap());
    let (from_leg, to_leg) = if first.1.account_id == from {
        (first, second)
    } else {
        (second, first)
    };
    assert_eq!(
        (from_leg.1.account_id, to_leg.1.account_id),
        (from, to),
        "legs on the wrong accounts"
    );
    assert_eq!(from_leg.1.event_type, "transfer");
    assert_eq!(to_leg.1.event_type, "transfer");
    assert_eq!(from_leg.1.linked_event_id, Some(to_leg.0));
    assert_eq!(to_leg.1.linked_event_id, Some(from_leg.0));
    assert_eq!(from_leg.1.counterpart_account_id, Some(to));
    assert_eq!(to_leg.1.counterpart_account_id, Some(from));
    (from_leg, to_leg)
}

#[test]
fn a_transfer_imported_from_one_account_then_the_other_ends_as_one_pair_on_each_banks_day() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let savings = own(&conn, "Savings");
    let checking_rows = [to(savings, row(0, "2026-03-01", -10_000))];
    let savings_rows = [to(checking, row(0, "2026-03-03", 10_000))];
    import(&conn, checking, &checking_rows);
    let (_, savings_leg) = the_pair(&conn, checking, savings);

    let reviewed = preview(&conn, savings, &savings_rows);
    let imported = apply(
        &conn,
        savings,
        &savings_rows,
        default_choices(&reviewed),
        reviewed.review_token.clone(),
    )
    .unwrap();

    assert_eq!(
        reviewed.rows[0].verdict,
        Verdict::TransferLeg {
            event_id: savings_leg.0,
            other_account_name: "Checking".to_owned(),
            event_date: "2026-03-01T12:00:00".to_owned(),
        }
    );
    assert_eq!(imported.imported, 0);
    let (checking_leg, savings_leg) = the_pair(&conn, checking, savings);
    assert_eq!(checking_leg.1.event_date, "2026-03-01T12:00:00");
    assert_eq!(savings_leg.1.event_date, "2026-03-03T12:00:00");
    assert_eq!(
        bank_entry(&conn, savings_leg.0),
        Some(("2026-03-03".to_owned(), 10_000, Some("row 0".to_owned())))
    );
    assert!(matches!(
        &verdicts(&conn, savings, &savings_rows)[..],
        [Verdict::InSaldo { .. }]
    ));
    assert!(matches!(
        &verdicts(&conn, checking, &checking_rows)[..],
        [Verdict::InSaldo { .. }]
    ));
}

#[test]
fn a_transfer_imported_in_the_other_order_ends_as_the_same_pair() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let savings = own(&conn, "Savings");
    import(
        &conn,
        savings,
        &[to(checking, row(0, "2026-03-03", 10_000))],
    );

    let checking_rows = [to(savings, row(0, "2026-03-01", -10_000))];
    let verdict = verdicts(&conn, checking, &checking_rows);
    import(&conn, checking, &checking_rows);

    assert!(matches!(
        &verdict[..],
        [Verdict::TransferLeg { other_account_name, event_date, .. }]
            if other_account_name == "Savings" && event_date == "2026-03-03T12:00:00"
    ));
    let (checking_leg, savings_leg) = the_pair(&conn, checking, savings);
    assert_eq!(checking_leg.1.event_date, "2026-03-01T12:00:00");
    assert_eq!(savings_leg.1.event_date, "2026-03-03T12:00:00");
}

#[test]
fn a_row_completes_the_cash_flow_the_other_accounts_import_made() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let savings = own(&conn, "Savings");
    let reserve = account(&conn, "Reserve", "bucket", BucketKind::Balance);
    let savings_rows = [row(0, "2026-03-03", 10_000)];
    let reviewed = preview(&conn, savings, &savings_rows);
    let tagged = RowChoice {
        bucket_id: Some(reserve),
        ..choice(0)
    };
    apply(
        &conn,
        savings,
        &savings_rows,
        vec![tagged],
        reviewed.review_token,
    )
    .unwrap();
    let cash_flow = live(&conn)[0].0;

    let checking_rows = [to(savings, row(0, "2026-03-01", -10_000))];
    let reviewed = preview(&conn, checking, &checking_rows);
    let imported = apply(
        &conn,
        checking,
        &checking_rows,
        default_choices(&reviewed),
        reviewed.review_token.clone(),
    )
    .unwrap();

    assert_eq!(
        reviewed.rows[0].verdict,
        Verdict::CompletesTransfer {
            event_id: cash_flow,
            account_id: savings,
            account_name: "Savings".to_owned(),
            event_date: "2026-03-03T12:00:00".to_owned(),
            note: Some("row 0".to_owned()),
        }
    );
    assert_eq!(imported.imported, 1);
    let (checking_leg, savings_leg) = the_pair(&conn, checking, savings);
    assert_eq!(savings_leg.0, cash_flow);
    assert_eq!(checking_leg.1.event_date, "2026-03-01T12:00:00");
    assert_eq!(savings_leg.1.event_date, "2026-03-03T12:00:00");
    assert_eq!(savings_leg.1.amount_minor, 10_000);
    let (note, bucket_id, versions): (Option<String>, Option<i64>, i64) = conn
        .query_row(
            "SELECT ed.note, ed.bucket_id,
                    (SELECT COUNT(*) FROM event_data WHERE event_id = e.id)
             FROM event e JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.id = ?1",
            params![cash_flow],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (note.as_deref(), bucket_id, versions),
        (Some("row 0"), Some(reserve), 2)
    );
    assert_eq!(
        bank_entry(&conn, cash_flow),
        Some(("2026-03-03".to_owned(), 10_000, Some("row 0".to_owned())))
    );
    assert_eq!(
        bank_entry(&conn, checking_leg.0),
        Some(("2026-03-01".to_owned(), -10_000, Some("row 0".to_owned())))
    );
    assert!(matches!(
        &verdicts(&conn, savings, &savings_rows)[..],
        [Verdict::InSaldo { .. }]
    ));
}

#[test]
fn a_transfer_completes_only_when_the_row_and_the_cash_flow_have_no_rival() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let savings = own(&conn, "Savings");
    let brokerage = own(&conn, "Brokerage");
    cashflow(&conn, savings, "2026-03-02T09:00:00", 10_000);
    cashflow(&conn, savings, "2026-03-03T09:00:00", 10_000);
    cashflow(&conn, brokerage, "2026-03-02T09:00:00", 7_000);

    let two_cash_flows = verdicts(
        &conn,
        checking,
        &[to(savings, row(0, "2026-03-01", -10_000))],
    );
    let two_rows = verdicts(
        &conn,
        checking,
        &[
            to(brokerage, row(0, "2026-03-01", -7_000)),
            to(brokerage, row(1, "2026-03-04", -7_000)),
        ],
    );
    let one_each = verdicts(
        &conn,
        checking,
        &[to(brokerage, row(0, "2026-03-01", -7_000))],
    );

    assert_eq!(two_cash_flows, [Verdict::New]);
    assert_eq!(two_rows, [Verdict::New, Verdict::New]);
    assert!(matches!(
        &one_each[..],
        [Verdict::CompletesTransfer { account_id, .. }] if *account_id == brokerage
    ));
}

fn leg_on_checking(conn: &Connection, checking: i64, savings: i64, date: &str) -> i64 {
    let (_, checking_leg) = create_transfer(
        conn,
        &CashflowEntry {
            counterpart_account_id: Some(checking),
            ..cashflow_entry(savings, date, -10_000)
        },
    )
    .unwrap();
    checking_leg
}

#[test]
fn only_a_row_without_a_counterpart_or_naming_the_legs_other_account_claims_a_leg() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let savings = own(&conn, "Savings");
    let brokerage = own(&conn, "Brokerage");
    let landlord = partner(&conn);
    let checking_leg = leg_on_checking(&conn, checking, savings, "2026-03-01T09:00:00");
    let near = || row(0, "2026-03-03", 10_000);

    assert_eq!(
        verdicts(&conn, checking, &[to(landlord, near())]),
        [Verdict::New]
    );
    assert_eq!(
        verdicts(&conn, checking, &[to(brokerage, near())]),
        [Verdict::New]
    );
    for claimant in [near(), to(savings, near())] {
        assert!(matches!(
            &verdicts(&conn, checking, &[claimant])[..],
            [Verdict::TransferLeg { event_id, .. }] if *event_id == checking_leg
        ));
    }
    assert_eq!(
        verdicts(&conn, checking, &[row(0, "2026-03-07", 10_000)]),
        [Verdict::New],
        "a leg 6 days away is not this transfer"
    );
}

#[test]
fn a_row_claims_the_closest_leg_then_the_newest() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let savings = own(&conn, "Savings");
    let far = leg_on_checking(&conn, checking, savings, "2026-03-01T09:00:00");
    let older = leg_on_checking(&conn, checking, savings, "2026-03-07T09:00:00");
    let newer = leg_on_checking(&conn, checking, savings, "2026-03-07T09:00:00");
    written_at(&conn, older, "2026-03-07T10:00:00.000");
    written_at(&conn, newer, "2026-03-08T10:00:00.000");
    written_at(&conn, far, "2026-03-09T10:00:00.000");

    let claimed = verdicts(
        &conn,
        checking,
        &[row(0, "2026-03-05", 10_000), row(1, "2026-03-05", 10_000)],
    );

    let event_ids: Vec<i64> = claimed
        .iter()
        .map(|verdict| match verdict {
            Verdict::TransferLeg { event_id, .. } => *event_id,
            other => panic!("expected a transfer leg, got {other:?}"),
        })
        .collect();
    assert_eq!(event_ids, [newer, older]);
}

struct Transfer {
    checking: i64,
    savings: i64,
    checking_leg: i64,
    savings_leg: i64,
}

/// Savings' export names Checking, so Savings' import made the pair for 50.00 from Checking on
/// 1 March, and Checking's leg has no bank entry yet.
fn savings_imported_a_transfer_from_checking(conn: &Connection) -> Transfer {
    let checking = own(conn, "Checking");
    let savings = own(conn, "Savings");
    import(conn, savings, &[to(checking, row(0, "2026-03-01", 5_000))]);
    let ((savings_leg, _), (checking_leg, _)) = the_pair(conn, savings, checking);
    Transfer {
        checking,
        savings,
        checking_leg,
        savings_leg,
    }
}

impl Transfer {
    /// Checking's export lists `payment` first and the transfer row, on `transfer_day`, second.
    fn import_checking(
        &self,
        conn: &Connection,
        payment: IncomingRowInput,
        transfer_day: &str,
    ) -> Vec<Verdict> {
        let rows = [payment, to(self.savings, row(1, transfer_day, -5_000))];
        let reviewed = preview(conn, self.checking, &rows);
        apply(
            conn,
            self.checking,
            &rows,
            default_choices(&reviewed),
            reviewed.review_token.clone(),
        )
        .unwrap();
        reviewed.rows.into_iter().map(|row| row.verdict).collect()
    }

    fn claimed_leg(&self) -> Verdict {
        Verdict::TransferLeg {
            event_id: self.checking_leg,
            other_account_name: "Savings".to_owned(),
            event_date: "2026-03-01T12:00:00".to_owned(),
        }
    }

    fn payment(&self, day: &str, counterpart_account_id: Option<i64>) -> Live {
        Live {
            account_id: self.checking,
            event_type: "cashflow".to_owned(),
            event_date: format!("{day}T12:00:00"),
            amount_minor: -5_000,
            counterpart_account_id,
            linked_event_id: None,
        }
    }

    /// The pair, with Checking's leg on `transfer_day`, and `payment` as the only other entry.
    fn assert_one_pair_beside(&self, conn: &Connection, transfer_day: &str, payment: Live) {
        let entries = live(conn);
        assert_eq!(
            entries.len(),
            3,
            "expected the pair and the payment: {entries:?}"
        );
        let entry = |event_id: i64| {
            &entries
                .iter()
                .find(|(id, _)| *id == event_id)
                .expect("a leg of the pair is gone")
                .1
        };
        assert_eq!(
            entry(self.savings_leg),
            &Live {
                account_id: self.savings,
                event_type: "transfer".to_owned(),
                event_date: "2026-03-01T12:00:00".to_owned(),
                amount_minor: 5_000,
                counterpart_account_id: Some(self.checking),
                linked_event_id: Some(self.checking_leg),
            }
        );
        assert_eq!(
            entry(self.checking_leg),
            &Live {
                account_id: self.checking,
                event_type: "transfer".to_owned(),
                event_date: format!("{transfer_day}T12:00:00"),
                amount_minor: -5_000,
                counterpart_account_id: Some(self.savings),
                linked_event_id: Some(self.savings_leg),
            }
        );
        let (_, written) = entries
            .iter()
            .find(|(id, _)| ![self.savings_leg, self.checking_leg].contains(id))
            .unwrap();
        assert_eq!(written, &payment);
    }
}

#[test]
fn a_payment_naming_no_account_on_the_legs_day_leaves_the_leg_to_the_transfer_row() {
    let conn = initialize_in_memory().unwrap();
    let transfer = savings_imported_a_transfer_from_checking(&conn);

    let verdicts = transfer.import_checking(&conn, row(0, "2026-03-01", -5_000), "2026-03-03");

    assert_eq!(verdicts, [Verdict::New, transfer.claimed_leg()]);
    transfer.assert_one_pair_beside(&conn, "2026-03-03", transfer.payment("2026-03-01", None));
}

#[test]
fn a_partner_payment_on_the_legs_day_leaves_the_leg_to_the_transfer_row() {
    let conn = initialize_in_memory().unwrap();
    let transfer = savings_imported_a_transfer_from_checking(&conn);
    let landlord = partner(&conn);

    let verdicts = transfer.import_checking(
        &conn,
        to(landlord, row(0, "2026-03-01", -5_000)),
        "2026-03-03",
    );

    assert_eq!(verdicts, [Verdict::New, transfer.claimed_leg()]);
    transfer.assert_one_pair_beside(
        &conn,
        "2026-03-03",
        transfer.payment("2026-03-01", Some(landlord)),
    );
}

#[test]
fn a_partner_payment_on_the_transfers_own_day_leaves_the_leg_to_the_transfer_row() {
    let conn = initialize_in_memory().unwrap();
    let transfer = savings_imported_a_transfer_from_checking(&conn);
    let landlord = partner(&conn);

    let verdicts = transfer.import_checking(
        &conn,
        to(landlord, row(0, "2026-03-01", -5_000)),
        "2026-03-01",
    );

    assert!(
        matches!(
            &verdicts[..],
            [Verdict::New, Verdict::InSaldo { event_id, .. }] if *event_id == transfer.checking_leg
        ),
        "{verdicts:?}"
    );
    transfer.assert_one_pair_beside(
        &conn,
        "2026-03-01",
        transfer.payment("2026-03-01", Some(landlord)),
    );
}

#[test]
fn a_newest_first_export_leaves_the_leg_to_the_transfer_row_listed_after_a_payment() {
    let conn = initialize_in_memory().unwrap();
    let transfer = savings_imported_a_transfer_from_checking(&conn);

    let verdicts = transfer.import_checking(&conn, row(0, "2026-03-03", -5_000), "2026-03-02");

    assert_eq!(verdicts, [Verdict::New, transfer.claimed_leg()]);
    transfer.assert_one_pair_beside(&conn, "2026-03-02", transfer.payment("2026-03-03", None));
}

#[test]
fn an_oldest_first_export_leaves_the_leg_to_the_transfer_row_listed_after_a_closer_payment() {
    let conn = initialize_in_memory().unwrap();
    let transfer = savings_imported_a_transfer_from_checking(&conn);

    let verdicts = transfer.import_checking(&conn, row(0, "2026-03-02", -5_000), "2026-03-04");

    assert_eq!(verdicts, [Verdict::New, transfer.claimed_leg()]);
    transfer.assert_one_pair_beside(&conn, "2026-03-04", transfer.payment("2026-03-02", None));
}

#[test]
fn a_payment_to_an_unknown_iban_leaves_the_leg_to_the_transfer_row() {
    let conn = initialize_in_memory().unwrap();
    let transfer = savings_imported_a_transfer_from_checking(&conn);

    let verdicts = transfer.import_checking(
        &conn,
        to_unknown_iban(row(0, "2026-03-03", -5_000)),
        "2026-03-02",
    );

    assert_eq!(verdicts, [Verdict::New, transfer.claimed_leg()]);
    transfer.assert_one_pair_beside(&conn, "2026-03-02", transfer.payment("2026-03-03", None));
}

#[test]
fn a_row_naming_an_unknown_iban_a_partner_or_another_account_never_claims_a_leg() {
    let conn = initialize_in_memory().unwrap();
    let transfer = savings_imported_a_transfer_from_checking(&conn);
    let brokerage = own(&conn, "Brokerage");
    let landlord = partner(&conn);

    for day in ["2026-03-01", "2026-03-03"] {
        let payment = || row(0, day, -5_000);
        for claimant in [
            to_unknown_iban(payment()),
            to(landlord, payment()),
            to(brokerage, payment()),
        ] {
            assert_eq!(
                verdicts(&conn, transfer.checking, std::slice::from_ref(&claimant)),
                [Verdict::New],
                "{claimant:?}"
            );
        }
        assert!(
            matches!(
                &verdicts(&conn, transfer.checking, &[payment()])[..],
                [Verdict::InSaldo { event_id, .. } | Verdict::TransferLeg { event_id, .. }]
                    if *event_id == transfer.checking_leg
            ),
            "a row naming no account claims the leg on {day}"
        );
    }
}

#[test]
fn a_row_naming_an_unknown_iban_claims_a_leg_to_an_own_account_saldo_has_no_iban_for() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let cash_box = own_without_iban(&conn, "Cash box");
    import(
        &conn,
        cash_box,
        &[to(checking, row(0, "2026-03-01", 5_000))],
    );
    let ((_, _), (checking_leg, _)) = the_pair(&conn, cash_box, checking);

    let claimed = verdicts(
        &conn,
        checking,
        &[to_unknown_iban(row(0, "2026-03-02", -5_000))],
    );

    assert_eq!(
        claimed,
        [Verdict::TransferLeg {
            event_id: checking_leg,
            other_account_name: "Cash box".to_owned(),
            event_date: "2026-03-01T12:00:00".to_owned(),
        }]
    );
}

#[test]
fn an_unknown_iban_payment_has_no_counterpart_and_matches_once_the_iban_is_a_partner() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let payment = || to_unknown_iban(row(0, "2026-03-01", -700));
    import(&conn, checking, &[payment()]);
    let [(event_id, entry)] = &live(&conn)[..] else {
        panic!("expected one entry");
    };
    assert_eq!(entry.counterpart_account_id, None);
    let landlord = partner(&conn);

    let matched = || in_saldo(*event_id, "2026-03-01T12:00:00", "row 0");
    assert_eq!(verdicts(&conn, checking, &[payment()]), [matched()]);
    assert_eq!(
        verdicts(&conn, checking, &[to(landlord, row(0, "2026-03-01", -700))]),
        [matched()]
    );
}

#[test]
fn a_stale_review_refuses_and_writes_nothing() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let typed = cashflow(&conn, checking, "2026-03-02T09:00:00", -800);
    let rows = [row(0, "2026-03-01", -1_250), row(1, "2026-03-02", -800)];
    let reviewed = preview(&conn, checking, &rows);

    update_event(
        &conn,
        UpdateEventParams {
            event_id: typed,
            amount_minor: -800,
            event_date: "2026-03-02T09:00:00".to_owned(),
            note: Some("typed by hand".to_owned()),
        },
    )
    .unwrap();
    let before = everything(&conn);
    let refused = apply(
        &conn,
        checking,
        &rows,
        default_choices(&reviewed),
        reviewed.review_token,
    )
    .unwrap_err();

    assert_eq!(refused.code, "IMPORT_CHANGED");
    assert_eq!(everything(&conn), before);
}

#[test]
fn rows_that_differ_from_the_review_refuse() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let reviewed = preview(&conn, checking, &[row(0, "2026-03-01", -1_250)]);

    let refused = apply(
        &conn,
        checking,
        &[row(0, "2026-03-01", -1_300)],
        default_choices(&reviewed),
        reviewed.review_token,
    )
    .unwrap_err();

    assert_eq!(refused.code, "IMPORT_CHANGED");
    assert!(live(&conn).is_empty());
}

#[test]
fn an_entry_added_to_the_account_during_the_review_refuses_the_import() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let rows = [row(0, "2026-03-01", -1_250)];
    let reviewed = preview(&conn, checking, &rows);

    cashflow(&conn, checking, "2026-02-20T09:00:00", -300);
    let before = everything(&conn);
    let refused = apply(
        &conn,
        checking,
        &rows,
        default_choices(&reviewed),
        reviewed.review_token,
    )
    .unwrap_err();

    assert_eq!(refused.code, "IMPORT_CHANGED");
    assert_eq!(everything(&conn), before);
}

#[test]
fn applying_a_review_a_second_time_refuses_and_writes_nothing() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let rows = [row(0, "2026-03-01", -1_250)];
    import(&conn, checking, &rows);
    let again = preview(&conn, checking, &rows);
    let first = apply(
        &conn,
        checking,
        &rows,
        vec![choice(0)],
        again.review_token.clone(),
    )
    .unwrap();
    let before = everything(&conn);

    let replayed = apply(&conn, checking, &rows, vec![choice(0)], again.review_token).unwrap_err();

    assert_eq!(first.imported, 1);
    assert_eq!(replayed.code, "IMPORT_CHANGED");
    assert_eq!(everything(&conn), before);
}

fn choice(index: u32) -> RowChoice {
    RowChoice {
        index,
        selected: true,
        bucket_id: None,
        split: None,
    }
}

fn split_choice(index: u32, legs: &[(i64, Option<i64>)]) -> RowChoice {
    RowChoice {
        split: Some(
            legs.iter()
                .map(|&(amount_minor, counterpart_account_id)| SplitLegChoice {
                    amount_minor,
                    note: None,
                    counterpart_account_id,
                    bucket_id: None,
                })
                .collect(),
        ),
        ..choice(index)
    }
}

#[test]
fn an_invalid_import_is_refused_and_writes_nothing() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let landlord = partner(&conn);
    let good = row(0, "2026-03-01", -1_000);
    let cases: Vec<(&str, i64, Vec<IncomingRowInput>, Vec<RowChoice>)> = vec![
        (
            "a partner account",
            landlord,
            vec![good.clone()],
            vec![choice(0)],
        ),
        (
            "an unknown account",
            UNKNOWN_ACCOUNT_ID,
            vec![good.clone()],
            vec![choice(0)],
        ),
        (
            "a counterpart equal to the account",
            checking,
            vec![good.clone(), to(checking, row(1, "2026-03-01", -500))],
            vec![choice(0), choice(1)],
        ),
        (
            "an unknown counterpart",
            checking,
            vec![
                good.clone(),
                to(UNKNOWN_ACCOUNT_ID, row(1, "2026-03-01", -500)),
            ],
            vec![choice(0), choice(1)],
        ),
        (
            "a row naming both an unknown IBAN and an account",
            checking,
            vec![
                good.clone(),
                to_unknown_iban(to(landlord, row(1, "2026-03-01", -500))),
            ],
            vec![choice(0), choice(1)],
        ),
        (
            "a bad date",
            checking,
            vec![
                good.clone(),
                IncomingRowInput {
                    event_date: "2026-3-01T12:00:00".to_owned(),
                    ..row(1, "", -500)
                },
            ],
            vec![choice(0), choice(1)],
        ),
        (
            "a duplicate index",
            checking,
            vec![good.clone(), row(0, "2026-03-02", -500)],
            vec![choice(0)],
        ),
        ("a missing choice", checking, vec![good.clone()], vec![]),
        (
            "a choice for no row",
            checking,
            vec![good.clone()],
            vec![choice(0), choice(1)],
        ),
        (
            "two choices for a row",
            checking,
            vec![good.clone(), row(1, "2026-03-02", -500)],
            vec![choice(0), choice(0)],
        ),
        (
            "a split of one leg",
            checking,
            vec![good.clone()],
            vec![split_choice(0, &[(-1_000, None)])],
        ),
        (
            "a split that does not add up",
            checking,
            vec![good.clone()],
            vec![split_choice(0, &[(-600, None), (-300, None)])],
        ),
        (
            "a split leg with an unknown counterpart",
            checking,
            vec![good.clone()],
            vec![split_choice(
                0,
                &[(-600, None), (-400, Some(UNKNOWN_ACCOUNT_ID))],
            )],
        ),
    ];
    let before = everything(&conn);

    for (case, account_id, rows, choices) in cases {
        let refused = apply(&conn, account_id, &rows, choices, String::new()).unwrap_err();

        assert_eq!(refused.code, "VALIDATION", "{case}: {}", refused.message);
        assert_eq!(everything(&conn), before, "{case} wrote something");
    }
}

#[test]
fn a_write_refused_by_storage_undoes_the_whole_import() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let budget = account(&conn, "Income budget", "bucket", BucketKind::IncomeBudget);
    let rows = [row(0, "2026-03-01", -1_000), row(1, "2026-03-02", 2_000)];
    let reviewed = preview(&conn, checking, &rows);
    let choices = vec![
        choice(0),
        RowChoice {
            bucket_id: Some(budget),
            ..choice(1)
        },
    ];

    let refused = apply(&conn, checking, &rows, choices, reviewed.review_token).unwrap_err();

    assert_eq!(refused.code, "VALIDATION");
    assert!(live(&conn).is_empty());
}

#[test]
fn the_preview_names_the_rows_days_with_a_balance_update() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    for date in ["2026-03-02T23:59:00", "2026-03-04T23:59:00"] {
        crate::features::transactions::repository::create_balance_update(
            &conn,
            crate::features::transactions::repository::CreateBalanceUpdateParams {
                account_id: checking,
                amount_minor: 100_000,
                event_date: date.to_owned(),
                note: None,
            },
        )
        .unwrap();
    }

    let reviewed = preview(
        &conn,
        checking,
        &[
            row(0, "2026-03-04", -1),
            row(1, "2026-03-01", -1),
            row(2, "2026-03-02", -1),
        ],
    );

    assert_eq!(reviewed.balance_update_days, ["2026-03-02", "2026-03-04"]);
    assert!(reviewed.rows.iter().all(|row| row.verdict == Verdict::New));
}

#[test]
fn an_import_runs_on_the_demo_database() {
    let conn = initialize_in_memory().unwrap();
    crate::demo::seed_demo_data(&conn, None, None).unwrap();
    let checking: i64 = conn
        .query_row(
            "SELECT id FROM account WHERE account_type = 'account' ORDER BY id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let rows = [row(0, "2026-03-01", -1_250)];

    assert_eq!(import(&conn, checking, &rows), 1);
    assert!(matches!(
        &verdicts(&conn, checking, &rows)[..],
        [Verdict::InSaldo { .. }]
    ));
}

const CARD: &str = "433703******2103";
const SECOND_CARD: &str = "433703******7708";
const DEBIT_CARD: &str = "423473******0501";
const HOLD_TEXT: &str = "BUDE DOPLNENÝ PO ZÚČT.";

fn card_row(
    index: u32,
    day: &str,
    amount_minor: i64,
    card: &str,
    state: CardRow,
) -> IncomingRowInput {
    IncomingRowInput {
        note: Some(match state {
            CardRow::Pending => HOLD_TEXT.to_owned(),
            CardRow::Booked(text) => text.to_owned(),
        }),
        card: Some(card.to_owned()),
        pending: matches!(state, CardRow::Pending),
        ..row(index, day, amount_minor)
    }
}

#[derive(Clone, Copy)]
enum CardRow {
    Pending,
    Booked(&'static str),
}

#[derive(Debug, PartialEq, Eq)]
struct HoldState {
    card: Option<String>,
    hold_seen_to: Option<String>,
    posted: bool,
    posting_change_minor: Option<i64>,
    cancelled: bool,
}

fn hold_state(conn: &Connection, event_id: i64) -> HoldState {
    conn.query_row(
        "SELECT card, hold_seen_to, posted_at IS NOT NULL, posting_change_minor,
                cancelled_at IS NOT NULL
         FROM bank_entry WHERE event_id = ?1",
        params![event_id],
        |row| {
            Ok(HoldState {
                card: row.get(0)?,
                hold_seen_to: row.get(1)?,
                posted: row.get(2)?,
                posting_change_minor: row.get(3)?,
                cancelled: row.get(4)?,
            })
        },
    )
    .unwrap()
}

fn pending_until(card: &str, seen_to: &str) -> HoldState {
    HoldState {
        card: Some(card.to_owned()),
        hold_seen_to: Some(seen_to.to_owned()),
        posted: false,
        posting_change_minor: None,
        cancelled: false,
    }
}

fn posted_with(card: &str, posting_change_minor: i64) -> HoldState {
    HoldState {
        card: Some(card.to_owned()),
        hold_seen_to: None,
        posted: true,
        posting_change_minor: Some(posting_change_minor),
        cancelled: false,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Entry {
    event_date: String,
    amount_minor: i64,
    note: Option<String>,
    counterpart_account_id: Option<i64>,
    bucket_id: Option<i64>,
    deleted: bool,
}

fn entry(conn: &Connection, event_id: i64) -> Entry {
    conn.query_row(
        "SELECT ed.event_date, ed.amount_minor, ed.note, ed.counterpart_account_id, ed.bucket_id,
                e.deleted_at IS NOT NULL
         FROM event e JOIN event_data ed ON ed.id = e.latest_data_id
         WHERE e.id = ?1",
        params![event_id],
        |row| {
            Ok(Entry {
                event_date: row.get(0)?,
                amount_minor: row.get(1)?,
                note: row.get(2)?,
                counterpart_account_id: row.get(3)?,
                bucket_id: row.get(4)?,
                deleted: row.get(5)?,
            })
        },
    )
    .unwrap()
}

fn event_ids(conn: &Connection) -> Vec<i64> {
    live(conn)
        .into_iter()
        .map(|(event_id, _)| event_id)
        .collect()
}

fn ticked(index: u32, selected: bool) -> RowChoice {
    RowChoice {
        selected,
        ..choice(index)
    }
}

fn tchibo_held(conn: &Connection) -> (i64, i64) {
    let card_account = own_without_iban(conn, "Card");
    import(
        conn,
        card_account,
        &[
            card_row(0, "2026-09-14", -7_896, CARD, CardRow::Pending),
            card_row(1, "2026-09-16", -500, CARD, CardRow::Booked("BILLA")),
        ],
    );
    let hold = live(conn)[0].0;
    (card_account, hold)
}

#[test]
fn a_pending_row_is_recorded_as_a_hold_seen_to_the_files_newest_day() {
    let conn = initialize_in_memory().unwrap();
    let (_, hold) = tchibo_held(&conn);
    let billa = live(&conn)[1].0;

    assert_eq!(hold_state(&conn, hold), pending_until(CARD, "2026-09-16"));
    assert_eq!(
        hold_state(&conn, billa),
        HoldState {
            card: Some(CARD.to_owned()),
            hold_seen_to: None,
            posted: false,
            posting_change_minor: None,
            cancelled: false,
        }
    );
}

#[test]
fn a_pending_export_imported_again_changes_nothing_and_a_newer_one_sees_the_hold_longer() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, hold) = tchibo_held(&conn);
    let first = [
        card_row(0, "2026-09-14", -7_896, CARD, CardRow::Pending),
        card_row(1, "2026-09-16", -500, CARD, CardRow::Booked("BILLA")),
    ];
    let before = everything(&conn);

    assert_eq!(import(&conn, card_account, &first), 0);
    assert_eq!(everything(&conn), before);

    let newer = [
        card_row(0, "2026-09-14", -7_896, CARD, CardRow::Pending),
        card_row(1, "2026-09-18", -900, CARD, CardRow::Booked("LIDL")),
    ];
    assert_eq!(import(&conn, card_account, &newer), 1);
    assert_eq!(hold_state(&conn, hold), pending_until(CARD, "2026-09-18"));
    assert_eq!(event_ids(&conn).len(), 3);
}

#[test]
fn a_hold_booked_a_day_later_is_posted_in_place_and_keeps_its_day_bucket_and_partner() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let landlord = partner(&conn);
    let groceries = account(&conn, "Groceries", "bucket", BucketKind::Balance);
    let held = [to(
        landlord,
        card_row(0, "2026-09-14", -7_896, CARD, CardRow::Pending),
    )];
    let reviewed = preview(&conn, card_account, &held);
    apply(
        &conn,
        card_account,
        &held,
        vec![RowChoice {
            bucket_id: Some(groceries),
            ..choice(0)
        }],
        reviewed.review_token,
    )
    .unwrap();
    let hold = live(&conn)[0].0;
    let booked = [card_row(
        0,
        "2026-09-15",
        -7_896,
        CARD,
        CardRow::Booked("TCHIBO PRAHA"),
    )];

    let reviewed = preview(&conn, card_account, &booked);
    let result = apply_reviewed(
        &conn,
        card_account,
        &booked,
        &reviewed,
        default_choices(&reviewed),
        true,
    )
    .unwrap();

    assert_eq!(
        reviewed.rows[0].verdict,
        Verdict::HoldPosted {
            event_id: hold,
            event_date: "2026-09-14T12:00:00".to_owned(),
            note: Some(HOLD_TEXT.to_owned()),
        }
    );
    assert_eq!(
        result,
        CsvImportResult {
            imported: 0,
            removed_holds: 0,
            repriced_holds: 0
        }
    );
    assert_eq!(event_ids(&conn), [hold]);
    assert_eq!(
        entry(&conn, hold),
        Entry {
            event_date: "2026-09-14T12:00:00".to_owned(),
            amount_minor: -7_896,
            note: Some("TCHIBO PRAHA".to_owned()),
            counterpart_account_id: Some(landlord),
            bucket_id: Some(groceries),
            deleted: false,
        }
    );
    assert_eq!(
        bank_entry(&conn, hold),
        Some((
            "2026-09-15".to_owned(),
            -7_896,
            Some("TCHIBO PRAHA".to_owned())
        ))
    );
    assert_eq!(hold_state(&conn, hold), posted_with(CARD, 0));

    let before = everything(&conn);
    assert_eq!(import(&conn, card_account, &booked), 0);
    assert_eq!(everything(&conn), before, "the same export again");
}

#[test]
fn a_note_the_user_wrote_stays_when_the_hold_posts() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, hold) = tchibo_held(&conn);
    update_event(
        &conn,
        UpdateEventParams {
            event_id: hold,
            amount_minor: -7_896,
            event_date: "2026-09-14T12:00:00".to_owned(),
            note: Some("coffee for the office".to_owned()),
        },
    )
    .unwrap();

    import(
        &conn,
        card_account,
        &[card_row(
            0,
            "2026-09-15",
            -7_896,
            CARD,
            CardRow::Booked("TCHIBO PRAHA"),
        )],
    );

    assert_eq!(
        entry(&conn, hold).note.as_deref(),
        Some("coffee for the office")
    );
    assert_eq!(hold_state(&conn, hold), posted_with(CARD, 0));
}

const SLOVNAFT_HOLD_TEXT: &str = "SLOVNAFT-SK40106         Kosice       SK";

#[test]
fn a_card_hold_text_imported_before_holds_were_known_takes_the_booked_text() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let legacy = |note: &str, amount_minor| {
        create_cashflow(
            &conn,
            &CashflowEntry {
                note: Some(note.to_owned()),
                ..cashflow_entry(card_account, "2026-10-31T12:00:00", amount_minor)
            },
        )
        .unwrap()
    };
    let slovnaft = legacy(SLOVNAFT_HOLD_TEXT, -1_999);
    let ekolky = legacy("HTTP   M EKOLKY GOV SK passport stamp", -5_000);
    let typed = cashflow(&conn, card_account, "2026-10-31T12:00:00", -500);
    let booked = [
        card_row(
            0,
            "2026-10-31",
            -1_999,
            CARD,
            CardRow::Booked("SLOVNAFT KOSICE"),
        ),
        card_row(
            1,
            "2026-10-31",
            -5_000,
            CARD,
            CardRow::Booked("HTTP   M EKOLKY GOV SK"),
        ),
        card_row(2, "2026-10-31", -500, CARD, CardRow::Booked("BILLA")),
    ];

    assert_eq!(import(&conn, card_account, &booked), 0);

    assert_eq!(
        entry(&conn, slovnaft).note.as_deref(),
        Some("SLOVNAFT KOSICE")
    );
    assert_eq!(
        entry(&conn, ekolky).note.as_deref(),
        Some("HTTP   M EKOLKY GOV SK passport stamp"),
        "a booked text with 3 spaces is no card hold's"
    );
    assert_eq!(entry(&conn, typed).note.as_deref(), Some("typed by hand"));
    let before = everything(&conn);
    import(&conn, card_account, &booked);
    assert_eq!(everything(&conn), before);
}

#[test]
fn the_same_export_again_keeps_the_note_the_user_wrote_on_a_booked_entry() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let export = [
        card_row(
            0,
            "2026-08-03",
            -5_000,
            "433704******5206",
            CardRow::Booked("HTTP   M EKOLKY GOV SK"),
        ),
        card_row(
            1,
            "2026-08-03",
            -1_999,
            "433704******5206",
            CardRow::Booked(SLOVNAFT_HOLD_TEXT),
        ),
    ];
    import(&conn, card_account, &export);
    let [ekolky, slovnaft] = event_ids(&conn)[..] else {
        panic!("expected two entries");
    };
    let annotate = |event_id, amount_minor, note: &str| {
        update_event(
            &conn,
            UpdateEventParams {
                event_id,
                amount_minor,
                event_date: "2026-08-03T12:00:00".to_owned(),
                note: Some(note.to_owned()),
            },
        )
        .unwrap();
    };
    annotate(ekolky, -5_000, "HTTP   M EKOLKY GOV SK passport stamp");
    annotate(slovnaft, -1_999, SLOVNAFT_HOLD_TEXT);
    let before = everything(&conn);

    assert_eq!(import(&conn, card_account, &export), 0);

    assert_eq!(
        everything(&conn),
        before,
        "an entry with bank facts keeps its note, even one in a card hold's layout"
    );
}

#[test]
fn an_older_export_in_another_encoding_never_rewrites_a_booked_entrys_note() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let fee_refund = |text| card_row(0, "2026-11-01", 500, CARD, CardRow::Booked(text));
    let good = "ZĽAVA Z POPLATKU ZA KARTU";
    let newer = [
        fee_refund(good),
        card_row(1, "2026-11-25", -900, CARD, CardRow::Booked("LIDL")),
    ];
    let older = [
        fee_refund("Z?AVA Z POPLATKU ZA KARTU"),
        card_row(1, "2026-11-20", -385, CARD, CardRow::Booked("TERNO")),
    ];
    import(&conn, card_account, &newer);
    let refund = live(&conn)[0].0;

    assert_eq!(import(&conn, card_account, &older), 1, "only Terno is new");

    assert_eq!(entry(&conn, refund).note.as_deref(), Some(good));
    assert_eq!(
        bank_entry(&conn, refund),
        Some(("2026-11-01".to_owned(), 500, Some(good.to_owned())))
    );
    let before = everything(&conn);
    import(&conn, card_account, &newer);
    assert_eq!(everything(&conn), before, "the newer export again");
}

fn github_held(conn: &Connection) -> (i64, i64) {
    let checking = own(conn, "Checking");
    import(
        conn,
        checking,
        &[card_row(
            0,
            "2026-04-30",
            -3_300,
            DEBIT_CARD,
            CardRow::Pending,
        )],
    );
    (checking, live(conn)[0].0)
}

fn github_booked() -> [IncomingRowInput; 2] {
    [
        card_row(
            0,
            "2026-04-30",
            -3_299,
            DEBIT_CARD,
            CardRow::Booked("GITHUB, INC."),
        ),
        card_row(1, "2026-05-02", -1_000, DEBIT_CARD, CardRow::Booked("BOLT")),
    ]
}

#[test]
fn a_hold_booked_at_another_amount_is_repriced_in_place_when_ticked() {
    let conn = initialize_in_memory().unwrap();
    let (checking, hold) = github_held(&conn);
    let groceries = account(&conn, "Groceries", "bucket", BucketKind::Balance);
    let booked = github_booked();
    let reviewed = preview(&conn, checking, &booked);

    let choices = vec![
        RowChoice {
            bucket_id: Some(groceries),
            ..choice(0)
        },
        choice(1),
    ];
    let result = apply_reviewed(&conn, checking, &booked, &reviewed, choices, true).unwrap();

    assert_eq!(
        reviewed.rows[0].verdict,
        Verdict::HoldPostedAtOtherAmount {
            event_id: hold,
            event_date: "2026-04-30T12:00:00".to_owned(),
            note: Some(HOLD_TEXT.to_owned()),
            hold_amount_minor: -3_300,
        }
    );
    assert!(reviewed.cancelled_holds.is_empty());
    assert_eq!(
        result,
        CsvImportResult {
            imported: 1,
            removed_holds: 0,
            repriced_holds: 1
        },
        "the Bolt row is new and GitHub's hold takes its booked amount"
    );
    assert_eq!(
        entry(&conn, hold),
        Entry {
            event_date: "2026-04-30T12:00:00".to_owned(),
            amount_minor: -3_299,
            note: Some("GITHUB, INC.".to_owned()),
            counterpart_account_id: None,
            bucket_id: None,
            deleted: false,
        },
        "the hold keeps its own bucket"
    );
    assert_eq!(hold_state(&conn, hold), posted_with(DEBIT_CARD, 1));

    let before = everything(&conn);
    assert_eq!(import(&conn, checking, &booked), 0);
    assert_eq!(everything(&conn), before, "the same export again");
}

#[test]
fn an_unticked_hold_at_another_amount_stays_pending_and_the_row_is_skipped() {
    let conn = initialize_in_memory().unwrap();
    let (checking, hold) = github_held(&conn);
    let booked = github_booked();
    let before = entry(&conn, hold);
    let reviewed = preview(&conn, checking, &booked);

    let result = apply_reviewed(
        &conn,
        checking,
        &booked,
        &reviewed,
        vec![ticked(0, false), choice(1)],
        true,
    )
    .unwrap();

    assert_eq!(result.repriced_holds, 0);
    assert_eq!(entry(&conn, hold), before);
    assert_eq!(
        hold_state(&conn, hold),
        pending_until(DEBIT_CARD, "2026-04-30")
    );
    assert_eq!(event_ids(&conn).len(), 2, "the hold and the Bolt row");
}

#[test]
fn a_hold_posting_at_another_amount_cannot_be_split() {
    let conn = initialize_in_memory().unwrap();
    let (checking, _) = github_held(&conn);
    let booked = github_booked();
    let reviewed = preview(&conn, checking, &booked);
    let before = everything(&conn);

    let refused = apply_reviewed(
        &conn,
        checking,
        &booked,
        &reviewed,
        vec![choice(1), split_choice(0, &[(-3_000, None), (-299, None)])],
        true,
    )
    .unwrap_err();

    assert_eq!(refused.code, "VALIDATION");
    assert_eq!(everything(&conn), before);
}

#[test]
fn ticking_a_posted_hold_adds_a_row_and_leaves_the_hold_pending() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, hold) = tchibo_held(&conn);
    let booked = [card_row(
        0,
        "2026-09-15",
        -7_896,
        CARD,
        CardRow::Booked("TCHIBO PRAHA"),
    )];
    let reviewed = preview(&conn, card_account, &booked);

    let result = apply_reviewed(
        &conn,
        card_account,
        &booked,
        &reviewed,
        vec![choice(0)],
        true,
    )
    .unwrap();

    assert_eq!(result.imported, 1);
    assert_eq!(event_ids(&conn).len(), 3);
    assert_eq!(hold_state(&conn, hold), pending_until(CARD, "2026-09-16"));
    assert_eq!(entry(&conn, hold).note.as_deref(), Some(HOLD_TEXT));
}

#[test]
fn the_same_export_again_after_ticking_a_posted_hold_changes_nothing() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, hold) = tchibo_held(&conn);
    let booked = [
        card_row(
            0,
            "2026-09-15",
            -7_896,
            CARD,
            CardRow::Booked("TCHIBO PRAHA"),
        ),
        card_row(1, "2026-09-18", -900, CARD, CardRow::Booked("LIDL")),
        card_row(2, "2026-09-13", -100, CARD, CardRow::Booked("KIOSK")),
    ];
    let reviewed = preview(&conn, card_account, &booked);
    apply_reviewed(
        &conn,
        card_account,
        &booked,
        &reviewed,
        vec![choice(0), choice(1), choice(2)],
        true,
    )
    .unwrap();
    let before = everything(&conn);

    let again = preview(&conn, card_account, &booked);

    assert!(again.cancelled_holds.is_empty());
    assert_eq!(import(&conn, card_account, &booked), 0);
    assert_eq!(everything(&conn), before);
    let newer = [
        card_row(0, "2026-09-13", -250, CARD, CardRow::Booked("BOLT")),
        card_row(1, "2026-09-20", -300, CARD, CardRow::Booked("BOLT")),
    ];
    assert_eq!(
        preview(&conn, card_account, &newer)
            .cancelled_holds
            .iter()
            .map(|cancelled| cancelled.event_id)
            .collect::<Vec<_>>(),
        [hold],
        "a newer export that no longer lists the hold cancels it"
    );
}

fn held_on_25_oct_export() -> [IncomingRowInput; 2] {
    [
        card_row(0, "2026-10-25", -1_999, CARD, CardRow::Pending),
        card_row(1, "2026-10-25", -500, CARD, CardRow::Booked("BILLA")),
    ]
}

fn held_on_25_oct(conn: &Connection) -> (i64, i64) {
    let card_account = own_without_iban(conn, "Card");
    import(conn, card_account, &held_on_25_oct_export());
    (card_account, live(conn)[0].0)
}

fn export_of_31_oct() -> [IncomingRowInput; 2] {
    [
        card_row(0, "2026-10-25", -500, CARD, CardRow::Booked("BILLA")),
        card_row(1, "2026-10-31", -1_200, CARD, CardRow::Booked("LIDL")),
    ]
}

#[test]
fn a_hold_a_newer_covering_export_no_longer_lists_is_removed_when_ticked() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, hold) = held_on_25_oct(&conn);
    let newer = export_of_31_oct();

    let reviewed = preview(&conn, card_account, &newer);
    let result = apply_reviewed(
        &conn,
        card_account,
        &newer,
        &reviewed,
        default_choices(&reviewed),
        true,
    )
    .unwrap();

    assert_eq!(
        reviewed.cancelled_holds,
        [CancelledHold {
            event_id: hold,
            event_date: "2026-10-25T12:00:00".to_owned(),
            amount_minor: -1_999,
            note: Some(HOLD_TEXT.to_owned()),
        }]
    );
    assert_eq!(
        result,
        CsvImportResult {
            imported: 1,
            removed_holds: 1,
            repriced_holds: 0
        }
    );
    assert!(entry(&conn, hold).deleted);
    assert_eq!(
        hold_state(&conn, hold),
        HoldState {
            card: Some(CARD.to_owned()),
            hold_seen_to: None,
            posted: false,
            posting_change_minor: None,
            cancelled: true,
        }
    );

    let before = everything(&conn);
    let again = preview(&conn, card_account, &newer);
    assert!(again.cancelled_holds.is_empty());
    assert_eq!(import(&conn, card_account, &newer), 0);
    assert_eq!(everything(&conn), before, "the same export again");
}

#[test]
fn an_unticked_cancelled_hold_is_kept_pending() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, hold) = held_on_25_oct(&conn);
    let newer = export_of_31_oct();
    let reviewed = preview(&conn, card_account, &newer);

    let result = apply_reviewed(
        &conn,
        card_account,
        &newer,
        &reviewed,
        default_choices(&reviewed),
        false,
    )
    .unwrap();

    assert_eq!(result.removed_holds, 0);
    assert!(!entry(&conn, hold).deleted);
    assert_eq!(hold_state(&conn, hold), pending_until(CARD, "2026-10-25"));
}

fn split_github_hold(conn: &Connection) -> (i64, Vec<i64>) {
    let checking = own(conn, "Checking");
    let held = [card_row(
        0,
        "2026-04-30",
        -3_300,
        DEBIT_CARD,
        CardRow::Pending,
    )];
    let reviewed = preview(conn, checking, &held);
    apply(
        conn,
        checking,
        &held,
        vec![split_choice(0, &[(-3_000, None), (-300, None)])],
        reviewed.review_token,
    )
    .unwrap();
    (checking, event_ids(conn))
}

#[derive(Debug, PartialEq, Eq)]
struct Posting {
    held_day: String,
    held_amount_minor: i64,
    posting_change_minor: i64,
}

fn posting(conn: &Connection, event_id: i64) -> Option<Posting> {
    conn.query_row(
        "SELECT held_day, held_amount_minor, posting_change_minor
         FROM bank_entry WHERE event_id = ?1 AND posted_at IS NOT NULL",
        params![event_id],
        |row| {
            Ok(Posting {
                held_day: row.get(0)?,
                held_amount_minor: row.get(1)?,
                posting_change_minor: row.get(2)?,
            })
        },
    )
    .ok()
}

fn replaced_hold(conn: &Connection, event_id: i64) -> Option<i64> {
    conn.query_row(
        "SELECT replaces_event_id FROM bank_entry WHERE event_id = ?1",
        params![event_id],
        |row| row.get(0),
    )
    .unwrap()
}

fn github_posting() -> Option<Posting> {
    Some(Posting {
        held_day: "2026-04-30".to_owned(),
        held_amount_minor: -3_300,
        posting_change_minor: 1,
    })
}

#[test]
fn a_split_hold_booked_at_another_amount_is_posted_onto_the_rows_new_entry() {
    let conn = initialize_in_memory().unwrap();
    let (checking, legs) = split_github_hold(&conn);
    let booked = github_booked();

    let reviewed = preview(&conn, checking, &booked);
    let result = apply_reviewed(
        &conn,
        checking,
        &booked,
        &reviewed,
        default_choices(&reviewed),
        true,
    )
    .unwrap();

    assert_eq!(
        reviewed.rows[0].verdict,
        Verdict::NewReplacingSplitHold {
            event_id: legs[0],
            event_date: "2026-04-30T12:00:00".to_owned(),
            hold_amount_minor: -3_300,
        }
    );
    assert!(
        reviewed.cancelled_holds.is_empty(),
        "the replaced hold belongs to its row"
    );
    assert_eq!(
        result,
        CsvImportResult {
            imported: 1,
            removed_holds: 0,
            repriced_holds: 1
        },
        "Bolt is new and GitHub's hold took its booked amount"
    );
    assert!(legs.iter().all(|&leg| entry(&conn, leg).deleted));
    assert!(
        !hold_state(&conn, legs[0]).cancelled,
        "a check counts the replaced hold as gone"
    );
    let github = event_ids(&conn)[0];
    assert_eq!(entry(&conn, github).amount_minor, -3_299);
    assert_eq!(posting(&conn, github), github_posting());
    assert_eq!(replaced_hold(&conn, github), Some(legs[0]));
    assert_eq!(account_total(&conn, checking), -3_299 - 1_000);
}

#[test]
fn a_split_hold_posted_onto_new_entries_takes_the_split_chosen_in_the_review() {
    let conn = initialize_in_memory().unwrap();
    let (checking, _) = split_github_hold(&conn);
    let booked = github_booked();
    let reviewed = preview(&conn, checking, &booked);

    apply_reviewed(
        &conn,
        checking,
        &booked,
        &reviewed,
        vec![split_choice(0, &[(-3_000, None), (-299, None)]), choice(1)],
        true,
    )
    .unwrap();

    let new_legs: Vec<i64> = event_ids(&conn)[..2].to_vec();
    assert_eq!(
        new_legs
            .iter()
            .map(|&leg| entry(&conn, leg).amount_minor)
            .collect::<Vec<_>>(),
        [-3_000, -299]
    );
    assert_eq!(posting(&conn, new_legs[0]), github_posting());
    assert_eq!(account_total(&conn, checking), -3_299 - 1_000);
    let before = everything(&conn);
    assert_eq!(import(&conn, checking, &booked), 0);
    assert_eq!(everything(&conn), before, "the same export again");
}

#[test]
fn an_unticked_row_replacing_a_split_hold_leaves_the_hold_pending_and_imports_nothing() {
    let conn = initialize_in_memory().unwrap();
    let (checking, legs) = split_github_hold(&conn);
    let [github, _] = github_booked();
    let booked = [github];
    let before = everything(&conn);

    let reviewed = preview(&conn, checking, &booked);
    let result = apply_reviewed(
        &conn,
        checking,
        &booked,
        &reviewed,
        vec![ticked(0, false)],
        true,
    )
    .unwrap();

    assert!(reviewed.cancelled_holds.is_empty());
    assert_eq!(
        result,
        CsvImportResult {
            imported: 0,
            removed_holds: 0,
            repriced_holds: 0
        }
    );
    assert_eq!(everything(&conn), before);
    assert_eq!(
        hold_state(&conn, legs[0]),
        pending_until(DEBIT_CARD, "2026-04-30")
    );
}

#[test]
fn the_export_that_listed_a_split_hold_adds_nothing_after_a_newer_one_replaced_it() {
    let with_the_same_newest_day = [
        card_row(0, "2026-04-30", -3_300, DEBIT_CARD, CardRow::Pending),
        card_row(1, "2026-04-29", -1_000, DEBIT_CARD, CardRow::Booked("BOLT")),
    ];
    let pending_only = [card_row(
        0,
        "2026-04-30",
        -3_300,
        DEBIT_CARD,
        CardRow::Pending,
    )];
    for (case, held) in [
        (
            "the replacing export ends on the hold's day",
            &with_the_same_newest_day[..],
        ),
        ("the export lists only the hold", &pending_only[..]),
    ] {
        let conn = initialize_in_memory().unwrap();
        let checking = own(&conn, "Checking");
        let reviewed = preview(&conn, checking, held);
        let mut choices = default_choices(&reviewed);
        choices[0] = split_choice(held[0].index, &[(-3_000, None), (-300, None)]);
        apply(&conn, checking, held, choices, reviewed.review_token).unwrap();
        let replacing = [
            card_row(
                0,
                "2026-04-30",
                -3_299,
                DEBIT_CARD,
                CardRow::Booked("GITHUB, INC."),
            ),
            card_row(1, "2026-04-29", -1_000, DEBIT_CARD, CardRow::Booked("BOLT")),
        ];
        import_with_defaults(&conn, checking, &replacing[..held.len()]);
        let github = *event_ids(&conn).last().unwrap();
        let before = everything(&conn);

        assert_eq!(
            verdicts(&conn, checking, &held[..1]),
            [in_saldo(github, "2026-04-30T12:00:00", "GITHUB, INC.")],
            "{case}"
        );
        assert_eq!(import(&conn, checking, held), 0, "{case}");
        assert_eq!(everything(&conn), before, "{case}");
    }
}

#[test]
fn a_week_export_again_after_the_month_statement_reprices_its_hold_adds_nothing() {
    for split in [false, true] {
        let conn = initialize_in_memory().unwrap();
        let checking = own(&conn, "Checking");
        let week = [
            card_row(0, "2026-04-30", -3_300, DEBIT_CARD, CardRow::Pending),
            card_row(1, "2026-05-05", -700, DEBIT_CARD, CardRow::Booked("BOLT")),
        ];
        let reviewed = preview(&conn, checking, &week);
        let github_choice = if split {
            split_choice(0, &[(-3_000, None), (-300, None)])
        } else {
            choice(0)
        };
        apply(
            &conn,
            checking,
            &week,
            vec![github_choice, choice(1)],
            reviewed.review_token,
        )
        .unwrap();
        let april_statement = [
            card_row(0, "2026-04-20", -1_000, DEBIT_CARD, CardRow::Booked("LIDL")),
            card_row(
                1,
                "2026-04-30",
                -3_299,
                DEBIT_CARD,
                CardRow::Booked("GITHUB, INC."),
            ),
        ];
        import(&conn, checking, &april_statement);
        assert_eq!(
            account_total(&conn, checking),
            -1_000 - 3_299 - 700,
            "split={split}"
        );
        let before = everything(&conn);

        assert_eq!(import(&conn, checking, &week), 0, "split={split}");
        assert_eq!(everything(&conn), before, "split={split}");
    }
}

#[test]
fn a_cancelled_split_hold_is_removed_as_a_whole() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let held = [
        card_row(0, "2026-10-25", -1_999, CARD, CardRow::Pending),
        card_row(1, "2026-10-25", -500, CARD, CardRow::Booked("BILLA")),
    ];
    let reviewed = preview(&conn, card_account, &held);
    apply(
        &conn,
        card_account,
        &held,
        vec![split_choice(0, &[(-1_500, None), (-499, None)]), choice(1)],
        reviewed.review_token,
    )
    .unwrap();
    let legs = event_ids(&conn)[..2].to_vec();
    let newer = export_of_31_oct();

    let reviewed = preview(&conn, card_account, &newer);
    let result = apply_reviewed(
        &conn,
        card_account,
        &newer,
        &reviewed,
        default_choices(&reviewed),
        true,
    )
    .unwrap();

    assert_eq!(
        reviewed.cancelled_holds,
        [CancelledHold {
            event_id: legs[0],
            event_date: "2026-10-25T12:00:00".to_owned(),
            amount_minor: -1_999,
            note: Some(HOLD_TEXT.to_owned()),
        }]
    );
    assert_eq!(result.removed_holds, 1);
    assert!(legs.iter().all(|&leg| entry(&conn, leg).deleted));
    assert!(hold_state(&conn, legs[0]).cancelled);

    let before = everything(&conn);
    assert_eq!(
        verdicts(&conn, card_account, &held)[0],
        in_saldo(legs[0], "2026-10-25T12:00:00", HOLD_TEXT),
        "the export that listed it, again"
    );
    assert_eq!(import(&conn, card_account, &held), 0);
    assert_eq!(everything(&conn), before);
}

#[test]
fn a_split_hold_booked_a_day_later_keeps_every_legs_day_and_takes_the_booked_text() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let held = [card_row(0, "2026-10-25", -1_999, CARD, CardRow::Pending)];
    let reviewed = preview(&conn, card_account, &held);
    apply(
        &conn,
        card_account,
        &held,
        vec![split_choice(0, &[(-1_500, None), (-499, None)])],
        reviewed.review_token,
    )
    .unwrap();
    let legs = event_ids(&conn);

    import(
        &conn,
        card_account,
        &[card_row(
            0,
            "2026-10-26",
            -1_999,
            CARD,
            CardRow::Booked("DM DROGERIE"),
        )],
    );

    assert_eq!(
        legs.iter()
            .map(|&leg| (entry(&conn, leg).event_date, entry(&conn, leg).amount_minor))
            .collect::<Vec<_>>(),
        [
            ("2026-10-25T12:00:00".to_owned(), -1_500),
            ("2026-10-25T12:00:00".to_owned(), -499)
        ]
    );
    let group_note: Option<String> = conn
        .query_row("SELECT note FROM split_group", [], |row| row.get(0))
        .unwrap();
    assert_eq!(group_note.as_deref(), Some("DM DROGERIE"));
    assert_eq!(hold_state(&conn, legs[0]), posted_with(CARD, 0));
}

#[test]
fn the_cancelled_hold_choices_must_be_the_reviews() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, hold) = held_on_25_oct(&conn);
    let newer = export_of_31_oct();
    let reviewed = preview(&conn, card_account, &newer);
    let remove = |event_id| CancelledHoldChoice {
        event_id,
        remove: true,
    };
    let cases = [
        ("no choice", vec![]),
        ("another entry", vec![remove(hold + 100)]),
        ("two choices", vec![remove(hold), remove(hold)]),
        ("an extra choice", vec![remove(hold), remove(hold + 100)]),
    ];
    let before = everything(&conn);

    for (case, cancelled_holds) in cases {
        let refused = apply_csv_import_inner(
            &conn,
            ApplyCsvImportInput {
                account_id: card_account,
                rows: newer.to_vec(),
                choices: default_choices(&reviewed),
                cancelled_holds,
                review_token: reviewed.review_token.clone(),
            },
        )
        .unwrap_err();

        assert_eq!(refused.code, "VALIDATION", "{case}: {}", refused.message);
        assert_eq!(everything(&conn), before, "{case} wrote something");
    }
}

#[test]
fn a_hold_seen_again_since_the_review_refuses_the_import() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, _) = held_on_25_oct(&conn);
    let newer = export_of_31_oct();
    let reviewed = preview(&conn, card_account, &newer);
    import(
        &conn,
        card_account,
        &[
            card_row(0, "2026-10-25", -1_999, CARD, CardRow::Pending),
            card_row(1, "2026-10-26", -700, CARD, CardRow::Booked("BOLT")),
        ],
    );
    let before = everything(&conn);

    let refused = apply_reviewed(
        &conn,
        card_account,
        &newer,
        &reviewed,
        default_choices(&reviewed),
        true,
    )
    .unwrap_err();

    assert_eq!(refused.code, "IMPORT_CHANGED");
    assert_eq!(everything(&conn), before);
}

#[test]
fn the_25_oct_then_31_oct_card_exports_post_every_hold_in_place() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let pending =
        |index, day, amount_minor, card| card_row(index, day, amount_minor, card, CardRow::Pending);
    let booked = |index, day, amount_minor, card, text| {
        card_row(index, day, amount_minor, card, CardRow::Booked(text))
    };
    let of_25_oct = [
        booked(0, "2026-10-22", -2_450, CARD, "SHELL"),
        pending(1, "2026-10-23", -50, CARD),
        pending(2, "2026-10-23", -50, SECOND_CARD),
        pending(3, "2026-10-24", -7_896, CARD),
        pending(4, "2026-10-25", -1_999, SECOND_CARD),
        pending(5, "2026-10-25", -315, CARD),
    ];
    let of_31_oct = [
        booked(0, "2026-10-22", -2_450, CARD, "SHELL"),
        booked(1, "2026-10-23", -50, SECOND_CARD, "ZSSK"),
        booked(2, "2026-10-23", -50, CARD, "ZSSK"),
        booked(3, "2026-10-25", -7_896, CARD, "TCHIBO"),
        booked(4, "2026-10-25", -1_999, SECOND_CARD, "DM DROGERIE"),
        booked(5, "2026-10-25", -315, CARD, "KAUFLAND"),
        booked(6, "2026-10-30", -1_200, CARD, "LIDL"),
        pending(7, "2026-10-31", -4_000, CARD),
    ];
    assert_eq!(import(&conn, card_account, &of_25_oct), 6);
    let holds: Vec<i64> = event_ids(&conn)[1..].to_vec();

    let reviewed = preview(&conn, card_account, &of_31_oct);
    let posted: Vec<i64> = reviewed
        .rows
        .iter()
        .filter_map(|row| match row.verdict {
            Verdict::HoldPosted { event_id, .. } => Some(event_id),
            _ => None,
        })
        .collect();
    let result = apply_reviewed(
        &conn,
        card_account,
        &of_31_oct,
        &reviewed,
        default_choices(&reviewed),
        true,
    )
    .unwrap();

    assert_eq!(posted, [holds[1], holds[0], holds[2], holds[3], holds[4]]);
    assert!(reviewed.cancelled_holds.is_empty());
    assert_eq!(result.imported, 2, "Lidl and the new hold");
    assert_eq!(event_ids(&conn).len(), 8);
    assert_eq!(
        entry(&conn, holds[2]).event_date,
        "2026-10-24T12:00:00",
        "Tchibo keeps its hold's day"
    );
    assert!(holds.iter().all(|&hold| hold_state(&conn, hold).posted));

    let before = everything(&conn);
    assert_eq!(import(&conn, card_account, &of_31_oct), 0);
    assert_eq!(everything(&conn), before, "the 31 Oct export again");
    assert_eq!(import(&conn, card_account, &of_25_oct), 0);
    assert_eq!(everything(&conn), before, "the older 25 Oct export again");
}

fn account_total(conn: &Connection, account_id: i64) -> i64 {
    live(conn)
        .into_iter()
        .filter(|(_, live)| live.account_id == account_id)
        .map(|(_, live)| live.amount_minor)
        .sum()
}

/// Long enough for `created_at` and a check's `checked_at`, both in milliseconds, to order the
/// writes.
const CLOCK_TICK: Duration = Duration::from_millis(1);

/// Records what the bank app showed, strictly after the writes before it and before the ones
/// after it.
fn check_now(conn: &Connection, account_id: i64, day: &str, bank_balance_minor: i64) {
    sleep(CLOCK_TICK);
    conn.execute(
        "INSERT INTO balance_check (account_id, day, bank_balance_minor) VALUES (?1, ?2, ?3)",
        params![account_id, day, bank_balance_minor],
    )
    .unwrap();
    sleep(CLOCK_TICK);
}

fn verification_now(conn: &Connection, account_id: i64) -> Option<Verification> {
    get_accounts_snapshot_inner(
        conn,
        GetSnapshotParams {
            moment: Moment::live(Local::now().date_naive()),
            person_id: None,
        },
    )
    .unwrap()
    .rows
    .into_iter()
    .find(|row| row.account_id == account_id)
    .and_then(|row| row.verification)
}

fn is_verified(conn: &Connection, account_id: i64) -> bool {
    matches!(
        verification_now(conn, account_id),
        Some(Verification::Verified { .. })
    )
}

fn tickets_held_on_23_and_24_oct(conn: &Connection) -> (i64, i64, i64) {
    let card_account = own_without_iban(conn, "Card");
    import(
        conn,
        card_account,
        &[
            card_row(0, "2026-10-24", -50, CARD, CardRow::Pending),
            card_row(1, "2026-10-23", -50, CARD, CardRow::Pending),
        ],
    );
    let [of_24_oct, of_23_oct] = event_ids(conn)[..] else {
        panic!("expected two holds");
    };
    (card_account, of_23_oct, of_24_oct)
}

#[test]
fn a_ticket_booked_a_day_later_posts_its_own_hold_while_the_next_days_ticket_is_pending() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, of_23_oct, of_24_oct) = tickets_held_on_23_and_24_oct(&conn);
    let booked_listed_first = [
        card_row(0, "2026-10-26", -900, CARD, CardRow::Booked("LIDL")),
        card_row(1, "2026-10-24", -50, CARD, CardRow::Booked("ZSSK")),
        card_row(2, "2026-10-24", -50, CARD, CardRow::Pending),
        card_row(3, "2026-10-23", -300, SECOND_CARD, CardRow::Booked("KIOSK")),
    ];

    let reviewed = preview(&conn, card_account, &booked_listed_first);
    let result = apply_reviewed(
        &conn,
        card_account,
        &booked_listed_first,
        &reviewed,
        default_choices(&reviewed),
        true,
    )
    .unwrap();

    assert_eq!(
        reviewed
            .rows
            .iter()
            .map(|row| row.verdict.clone())
            .collect::<Vec<_>>(),
        [
            Verdict::New,
            Verdict::HoldPosted {
                event_id: of_23_oct,
                event_date: "2026-10-23T12:00:00".to_owned(),
                note: Some(HOLD_TEXT.to_owned()),
            },
            in_saldo(of_24_oct, "2026-10-24T12:00:00", HOLD_TEXT),
            Verdict::New,
        ]
    );
    assert!(reviewed.cancelled_holds.is_empty());
    assert_eq!(
        result,
        CsvImportResult {
            imported: 2,
            removed_holds: 0,
            repriced_holds: 0
        }
    );
    assert_eq!(account_total(&conn, card_account), -50 - 50 - 900 - 300);
}

#[test]
fn a_ticket_booked_a_day_later_is_counted_once_whatever_the_export_lists_first() {
    let booked = card_row(1, "2026-10-24", -50, CARD, CardRow::Booked("ZSSK"));
    let pending = card_row(2, "2026-10-24", -50, CARD, CardRow::Pending);
    let orders = [
        ("booked row first", [booked.clone(), pending.clone()]),
        (
            "pending row first",
            [
                IncomingRowInput { index: 2, ..booked },
                IncomingRowInput {
                    index: 1,
                    ..pending
                },
            ],
        ),
    ];

    for (order, tickets) in orders {
        let conn = initialize_in_memory().unwrap();
        let (card_account, _, _) = tickets_held_on_23_and_24_oct(&conn);
        let mut export = vec![card_row(
            0,
            "2026-10-26",
            -900,
            CARD,
            CardRow::Booked("LIDL"),
        )];
        export.extend(tickets);

        import(&conn, card_account, &export);

        assert_eq!(event_ids(&conn).len(), 3, "{order}");
        assert_eq!(
            account_total(&conn, card_account),
            -50 - 50 - 900,
            "{order}"
        );
    }
}

fn daily_tickets_booked_a_day_later(pending_listed_first: bool) -> (usize, i64) {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let zssk = CardRow::Booked("ZELEZNICE SR");
    import(
        &conn,
        card_account,
        &[card_row(0, "2026-11-18", -50, CARD, CardRow::Pending)],
    );
    import(
        &conn,
        card_account,
        &[card_row(0, "2026-11-19", -50, CARD, zssk)],
    );
    let (second_held, first_booked) = if pending_listed_first { (0, 1) } else { (1, 0) };
    let lidl = card_row(2, "2026-11-20", -900, CARD, CardRow::Booked("LIDL"));
    import(
        &conn,
        card_account,
        &[
            card_row(second_held, "2026-11-19", -50, CARD, CardRow::Pending),
            card_row(first_booked, "2026-11-19", -50, CARD, zssk),
            lidl.clone(),
        ],
    );
    import(
        &conn,
        card_account,
        &[
            card_row(0, "2026-11-19", -50, CARD, zssk),
            card_row(1, "2026-11-20", -50, CARD, zssk),
            lidl,
        ],
    );
    (event_ids(&conn).len(), account_total(&conn, card_account))
}

#[test]
fn daily_tickets_booked_a_day_later_are_counted_once_whatever_the_export_lists_first() {
    assert_eq!(
        daily_tickets_booked_a_day_later(true),
        (3, -1_000),
        "pending rows listed first"
    );
    assert_eq!(
        daily_tickets_booked_a_day_later(false),
        (3, -1_000),
        "booked rows listed first"
    );
}

#[test]
fn an_older_export_imported_again_after_an_edited_hold_posted_at_another_amount_adds_nothing() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let older = [
        card_row(0, "2026-11-18", -3_300, CARD, CardRow::Pending),
        card_row(1, "2026-11-18", -500, CARD, CardRow::Booked("BILLA")),
    ];
    import(&conn, card_account, &older);
    let hold = event_ids(&conn)[0];
    update_event(
        &conn,
        UpdateEventParams {
            event_id: hold,
            amount_minor: -3_299,
            event_date: "2026-11-18T12:00:00".to_owned(),
            note: Some("GitHub".to_owned()),
        },
    )
    .unwrap();
    import(
        &conn,
        card_account,
        &[
            card_row(
                0,
                "2026-11-18",
                -3_299,
                CARD,
                CardRow::Booked("GITHUB, INC."),
            ),
            card_row(1, "2026-11-18", -500, CARD, CardRow::Booked("BILLA")),
            card_row(2, "2026-11-20", -900, CARD, CardRow::Booked("LIDL")),
        ],
    );
    assert!(hold_state(&conn, hold).posted);
    let before = everything(&conn);

    assert_eq!(import(&conn, card_account, &older), 0);
    assert_eq!(everything(&conn), before);
}

#[test]
fn an_older_export_imported_after_the_newer_finds_the_hold_the_newer_booked_a_day_later() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    import(
        &conn,
        card_account,
        &[
            card_row(0, "2026-10-26", -900, CARD, CardRow::Booked("LIDL")),
            card_row(1, "2026-10-25", -7_896, CARD, CardRow::Booked("TCHIBO")),
        ],
    );
    let before = everything(&conn);

    assert_eq!(
        import(
            &conn,
            card_account,
            &[card_row(0, "2026-10-24", -7_896, CARD, CardRow::Pending)]
        ),
        0
    );
    assert_eq!(everything(&conn), before);
}

#[test]
fn an_older_export_imported_again_after_a_cancel_keeps_the_hold_removed() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, hold) = held_on_25_oct(&conn);
    import(&conn, card_account, &export_of_31_oct());
    assert!(entry(&conn, hold).deleted);
    let before = everything(&conn);

    let older = [
        card_row(0, "2026-10-25", -1_999, CARD, CardRow::Pending),
        card_row(1, "2026-10-25", -500, CARD, CardRow::Booked("BILLA")),
    ];

    assert_eq!(import(&conn, card_account, &older), 0);
    assert_eq!(everything(&conn), before);
}

#[test]
fn a_hold_an_export_as_new_as_the_cancelling_one_lists_again_comes_back() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, hold) = held_on_25_oct(&conn);
    import(&conn, card_account, &export_of_31_oct());
    let listed_again = [
        card_row(0, "2026-10-25", -1_999, CARD, CardRow::Pending),
        card_row(1, "2026-10-31", -1_200, CARD, CardRow::Booked("LIDL")),
    ];

    assert_eq!(import(&conn, card_account, &listed_again), 1);
    let [.., (back, _)] = &live(&conn)[..] else {
        panic!("expected live entries");
    };
    assert_ne!(*back, hold);
    assert_eq!(
        (entry(&conn, *back).amount_minor, hold_state(&conn, *back)),
        (-1_999, pending_until(CARD, "2026-10-31"))
    );
    assert!(hold_state(&conn, hold).cancelled);
    assert_eq!(
        replaced_hold(&conn, *back),
        Some(hold),
        "the returning hold takes the removed one's place"
    );
    let before = everything(&conn);
    assert_eq!(import(&conn, card_account, &held_on_25_oct_export()), 0);
    assert_eq!(everything(&conn), before, "the oldest export again");
}

#[test]
fn an_unticked_row_listing_a_removed_hold_again_keeps_the_hold_removed() {
    let conn = initialize_in_memory().unwrap();
    let (card_account, hold) = held_on_25_oct(&conn);
    import(&conn, card_account, &export_of_31_oct());
    let listed_again = [
        card_row(0, "2026-10-25", -1_999, CARD, CardRow::Pending),
        card_row(1, "2026-10-31", -1_200, CARD, CardRow::Booked("LIDL")),
    ];
    let removed = everything(&conn);

    let reviewed = preview(&conn, card_account, &listed_again);
    apply_reviewed(
        &conn,
        card_account,
        &listed_again,
        &reviewed,
        vec![ticked(0, false), ticked(1, false)],
        true,
    )
    .unwrap();

    assert_eq!(reviewed.rows[0].verdict, Verdict::New);
    assert_eq!(everything(&conn), removed);
    assert!(hold_state(&conn, hold).cancelled);
}

#[test]
fn an_older_export_finds_the_hold_a_newer_one_booked_at_another_amount() {
    for booked_amount_minor in [-3_300, -3_299] {
        let conn = initialize_in_memory().unwrap();
        let checking = own(&conn, "Checking");
        import(
            &conn,
            checking,
            &[
                card_row(
                    0,
                    "2026-04-30",
                    booked_amount_minor,
                    DEBIT_CARD,
                    CardRow::Booked("GITHUB, INC."),
                ),
                card_row(
                    1,
                    "2026-05-04",
                    -5_000,
                    DEBIT_CARD,
                    CardRow::Booked("BANSKA"),
                ),
            ],
        );
        let before = everything(&conn);
        let older = [
            card_row(0, "2026-04-30", -3_300, DEBIT_CARD, CardRow::Pending),
            card_row(1, "2026-04-29", -500, DEBIT_CARD, CardRow::Booked("LIDL")),
        ];

        assert_eq!(
            import(&conn, checking, &older),
            1,
            "booked at {booked_amount_minor}: only Lidl is new"
        );
        assert_eq!(
            account_total(&conn, checking),
            booked_amount_minor - 5_000 - 500,
            "booked at {booked_amount_minor}: GitHub once, at its booked amount"
        );
        let github = event_ids(&conn)[0];
        assert_eq!(
            everything(&conn)
                .into_iter()
                .filter(|line| before.contains(line))
                .collect::<Vec<_>>(),
            before,
            "booked at {booked_amount_minor}: GitHub's entry and bank entry stay"
        );
        assert_eq!(entry(&conn, github).amount_minor, booked_amount_minor);
    }
}

#[test]
fn an_older_export_of_two_daily_tickets_adds_nothing_after_both_posted() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let zssk = CardRow::Booked("ZSSK");
    let both_held = [
        card_row(0, "2026-10-24", -50, CARD, CardRow::Pending),
        card_row(1, "2026-10-25", -50, CARD, CardRow::Pending),
    ];
    import(&conn, card_account, &both_held);
    import(
        &conn,
        card_account,
        &[
            card_row(0, "2026-10-25", -50, CARD, zssk),
            card_row(1, "2026-10-25", -50, CARD, CardRow::Pending),
        ],
    );
    import(
        &conn,
        card_account,
        &[
            card_row(0, "2026-10-25", -50, CARD, zssk),
            card_row(1, "2026-10-26", -50, CARD, zssk),
            card_row(2, "2026-10-27", -900, CARD, CardRow::Booked("LIDL")),
        ],
    );
    assert_eq!(account_total(&conn, card_account), -50 - 50 - 900);
    let before = everything(&conn);

    assert_eq!(import(&conn, card_account, &both_held), 0);
    assert_eq!(everything(&conn), before);
}

/// Each row's verdict, naming the entry it found.
fn verdict_phrases(reviewed: &CsvImportPreview) -> Vec<String> {
    reviewed
        .rows
        .iter()
        .map(|row| match &row.verdict {
            Verdict::New => "new".to_owned(),
            Verdict::InSaldo { event_id, .. } => format!("in Saldo {event_id}"),
            Verdict::HoldPosted { event_id, .. } => format!("posts {event_id}"),
            Verdict::HoldPostedAtOtherAmount { event_id, .. } => {
                format!("posts {event_id} at another amount")
            }
            Verdict::NewReplacingSplitHold { event_id, .. } => format!("replaces split {event_id}"),
            Verdict::TransferLeg { event_id, .. } => format!("leg {event_id}"),
            Verdict::CompletesTransfer { event_id, .. } => format!("completes {event_id}"),
        })
        .collect()
}

fn cancelled_hold_ids(reviewed: &CsvImportPreview) -> Vec<i64> {
    reviewed
        .cancelled_holds
        .iter()
        .map(|hold| hold.event_id)
        .collect()
}

const FIXTURE_SECOND_CARD: &str = "433704******5206";

/// The verify-saldo fixtures `card-holds-1.csv` and `card-holds-2.csv`. GitHub's hold is booked on
/// its day at another amount; Kaufland's, of the same card and day, is booked a day later at its
/// own amount.
#[test]
fn a_hold_repriced_on_its_day_posts_next_to_a_hold_of_its_card_and_day_booked_later() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let kaviaren = || card_row(0, "2026-11-15", -1_000, CARD, CardRow::Booked("Kaviaren"));
    let second = FIXTURE_SECOND_CARD;
    let held = CardRow::Pending;
    let of_18_nov = [
        kaviaren(),
        card_row(1, "2026-11-18", -2_904, CARD, held),
        card_row(2, "2026-11-18", -3_536, second, held),
        card_row(3, "2026-11-17", -7_896, CARD, held),
        card_row(4, "2026-11-18", -3_300, second, held),
        card_row(5, "2026-11-17", -1_200, second, held),
        card_row(6, "2026-11-18", -50, CARD, held),
        card_row(7, "2026-11-18", -50, second, held),
    ];
    import(&conn, card_account, &of_18_nov);
    let [kaviaren_id, kraj, kaufland, tchibo, github, zssk, ticket, second_ticket] =
        event_ids(&conn)[..]
    else {
        panic!("expected 8 entries");
    };
    check_now(&conn, card_account, "2026-11-19", -19_936);
    let booked = |index, day, amount_minor, card, text| {
        card_row(index, day, amount_minor, card, CardRow::Booked(text))
    };
    let of_20_nov = [
        kaviaren(),
        booked(1, "2026-11-18", -2_904, CARD, "2128 KRAJ"),
        booked(2, "2026-11-19", -3_536, second, "Kaufland SK 2020"),
        booked(3, "2026-11-18", -7_896, CARD, "WWW.TCHIBO.SK"),
        booked(4, "2026-11-18", -3_299, second, "GITHUB, INC."),
        booked(5, "2026-11-18", -50, CARD, "Zeleznice SR, Poprad"),
        booked(6, "2026-11-18", -50, second, "Zeleznice SR, Poprad"),
    ];

    let reviewed = preview(&conn, card_account, &of_20_nov);
    assert_eq!(
        verdict_phrases(&reviewed),
        [
            format!("in Saldo {kaviaren_id}"),
            format!("posts {kraj}"),
            format!("posts {kaufland}"),
            format!("posts {tchibo}"),
            format!("posts {github} at another amount"),
            format!("posts {ticket}"),
            format!("posts {second_ticket}"),
        ]
    );
    assert_eq!(cancelled_hold_ids(&reviewed), [zssk]);
    let result = apply_reviewed(
        &conn,
        card_account,
        &of_20_nov,
        &reviewed,
        default_choices(&reviewed),
        true,
    )
    .unwrap();
    assert_eq!(
        (result.repriced_holds, result.removed_holds),
        (1, 1),
        "{result:?}"
    );
    assert_eq!(account_total(&conn, card_account), -18_735);
    assert!(is_verified(&conn, card_account), "the check of 19 Nov");
    let before = everything(&conn);
    for (case, export) in [("of 20 Nov", &of_20_nov[..]), ("of 18 Nov", &of_18_nov[..])] {
        assert_eq!(
            import(&conn, card_account, export),
            0,
            "the export {case} again"
        );
        assert_eq!(everything(&conn), before, "the export {case} again");
    }
}

/// A ticket held on 4 Oct and booked on 5 Oct, and Tchibo held on 5 Oct and booked on 6 Oct, both
/// on one card and at their own amounts.
#[test]
fn two_holds_booked_a_day_later_at_their_own_amounts_post_their_own_holds() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let coffee = account(&conn, "Coffee", "bucket", BucketKind::Balance);
    let of_5_oct = [
        card_row(0, "2026-10-04", -50, CARD, CardRow::Pending),
        card_row(1, "2026-10-05", -1_999, CARD, CardRow::Pending),
    ];
    let reviewed = preview(&conn, card_account, &of_5_oct);
    apply(
        &conn,
        card_account,
        &of_5_oct,
        vec![
            choice(0),
            RowChoice {
                bucket_id: Some(coffee),
                ..choice(1)
            },
        ],
        reviewed.review_token,
    )
    .unwrap();
    let [ticket, tchibo] = event_ids(&conn)[..] else {
        panic!("expected two holds");
    };
    check_now(&conn, card_account, "2026-10-06", -2_049);
    let of_7_oct = [
        card_row(0, "2026-10-05", -50, CARD, CardRow::Booked("ZSSK")),
        card_row(1, "2026-10-06", -1_999, CARD, CardRow::Booked("TCHIBO")),
    ];

    let reviewed = preview(&conn, card_account, &of_7_oct);
    assert_eq!(
        verdict_phrases(&reviewed),
        [format!("posts {ticket}"), format!("posts {tchibo}")]
    );
    assert_eq!(import(&conn, card_account, &of_7_oct), 0);
    assert_eq!(entry(&conn, tchibo).bucket_id, Some(coffee));
    assert_eq!(account_total(&conn, card_account), -2_049);
    assert!(is_verified(&conn, card_account), "the check of 6 Oct");
}

#[test]
fn two_holds_booked_a_day_later_keep_a_check_made_while_they_were_pending_verified() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let lidl = || card_row(0, "2026-10-03", -900, CARD, CardRow::Booked("LIDL"));
    import(
        &conn,
        card_account,
        &[
            lidl(),
            card_row(1, "2026-10-04", -50, CARD, CardRow::Pending),
            card_row(2, "2026-10-05", -1_999, CARD, CardRow::Pending),
        ],
    );
    check_now(&conn, card_account, "2026-10-07", -2_949);

    import(
        &conn,
        card_account,
        &[
            lidl(),
            card_row(1, "2026-10-05", -50, CARD, CardRow::Booked("ZSSK")),
            card_row(2, "2026-10-06", -1_999, CARD, CardRow::Booked("TCHIBO")),
        ],
    );

    assert_eq!(account_total(&conn, card_account), -2_949);
    assert!(is_verified(&conn, card_account), "the check of 7 Oct");
}

/// The ticket of 3 Oct is the only hold of its card and day and is booked on its day at 0.49. The
/// shop's hold of 1 Oct, on the same card, is booked on 3 Oct at its own amount.
#[test]
fn a_lone_hold_booked_at_another_amount_posts_on_a_day_another_hold_of_its_card_is_booked() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    import(
        &conn,
        card_account,
        &[
            card_row(0, "2026-10-01", -500, CARD, CardRow::Pending),
            card_row(1, "2026-10-03", -50, CARD, CardRow::Pending),
        ],
    );
    let [shop, ticket] = event_ids(&conn)[..] else {
        panic!("expected two holds");
    };
    check_now(&conn, card_account, "2026-10-04", -550);
    let of_5_oct = [
        card_row(0, "2026-10-03", -500, CARD, CardRow::Booked("SHOP")),
        card_row(1, "2026-10-03", -49, CARD, CardRow::Booked("ZSSK")),
        card_row(2, "2026-10-05", -900, CARD, CardRow::Booked("LIDL")),
    ];

    let reviewed = preview(&conn, card_account, &of_5_oct);
    assert_eq!(
        verdict_phrases(&reviewed),
        [
            format!("posts {shop}"),
            format!("posts {ticket} at another amount"),
            "new".to_owned(),
        ]
    );
    assert_eq!(cancelled_hold_ids(&reviewed), Vec::<i64>::new());
    import(&conn, card_account, &of_5_oct);
    assert_eq!(account_total(&conn, card_account), -1_449);
    assert!(is_verified(&conn, card_account), "the check of 4 Oct");
}

/// Two 33.00 holds on one card: one held on 3 Oct and booked on 5 Oct at its amount, one held on
/// 4 Oct and booked on its day at 32.99. The 6 Oct export comes first, then the 4 Oct one that
/// still lists both holds.
#[test]
fn an_older_export_adds_nothing_after_a_newer_one_booked_one_hold_later_and_repriced_another() {
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    assert_eq!(
        import(
            &conn,
            card_account,
            &[
                card_row(0, "2026-10-06", -900, CARD, CardRow::Booked("LIDL")),
                card_row(1, "2026-10-05", -3_300, CARD, CardRow::Booked("SHOP A")),
                card_row(2, "2026-10-04", -3_299, CARD, CardRow::Booked("SHOP B")),
            ],
        ),
        3
    );
    let before = everything(&conn);
    let of_4_oct = [
        card_row(0, "2026-10-04", -3_300, CARD, CardRow::Pending),
        card_row(1, "2026-10-03", -3_300, CARD, CardRow::Pending),
    ];

    assert_eq!(import(&conn, card_account, &of_4_oct), 0);
    assert_eq!(everything(&conn), before);
}

/// Tchibo's hold of 3 Oct is gone from the 5 Oct export, so the import removes it, and a check on
/// 6 Oct matches the bank without it. The 8 Oct export lists the hold again, pending or booked on
/// 4 Oct.
#[test]
fn a_removed_hold_the_bank_lists_again_keeps_a_check_made_in_between_verified() {
    let posted_from_the_hold = Some(Posting {
        held_day: "2026-10-03".to_owned(),
        held_amount_minor: -1_999,
        posting_change_minor: 0,
    });
    for (case, listed_again, posting_of_the_row) in [
        (
            "pending",
            card_row(2, "2026-10-03", -1_999, CARD, CardRow::Pending),
            None,
        ),
        (
            "booked",
            card_row(2, "2026-10-04", -1_999, CARD, CardRow::Booked("TCHIBO")),
            posted_from_the_hold,
        ),
    ] {
        let conn = initialize_in_memory().unwrap();
        let card_account = own_without_iban(&conn, "Card");
        let lidl = |index, day, amount_minor| {
            card_row(index, day, amount_minor, CARD, CardRow::Booked("LIDL"))
        };
        import(
            &conn,
            card_account,
            &[
                lidl(0, "2026-10-02", -900),
                card_row(1, "2026-10-03", -1_999, CARD, CardRow::Pending),
            ],
        );
        let tchibo = event_ids(&conn)[1];
        let removed = import_with_defaults(
            &conn,
            card_account,
            &[lidl(0, "2026-10-02", -900), lidl(1, "2026-10-05", -901)],
        );
        assert_eq!(removed.removed_holds, 1, "{case}");
        check_now(&conn, card_account, "2026-10-06", -1_801);
        let of_8_oct = [
            lidl(0, "2026-10-02", -900),
            lidl(1, "2026-10-05", -901),
            listed_again,
            lidl(3, "2026-10-08", -902),
        ];

        assert_eq!(import(&conn, card_account, &of_8_oct), 2, "{case}");
        assert_eq!(account_total(&conn, card_account), -4_702, "{case}");
        assert!(
            is_verified(&conn, card_account),
            "{case}: the check of 6 Oct"
        );
        let (back, _) = live(&conn)
            .into_iter()
            .find(|(_, live)| live.amount_minor == -1_999)
            .unwrap();
        assert_eq!(replaced_hold(&conn, back), Some(tchibo), "{case}");
        assert_eq!(posting(&conn, back), posting_of_the_row, "{case}");
        let before = everything(&conn);
        assert_eq!(import(&conn, card_account, &of_8_oct), 0, "{case}: again");
        assert_eq!(everything(&conn), before, "{case}: again");
        let of_3_oct = [
            lidl(0, "2026-10-02", -900),
            card_row(1, "2026-10-03", -1_999, CARD, CardRow::Pending),
        ];
        assert_eq!(
            import(&conn, card_account, &of_3_oct),
            0,
            "{case}: the export that listed the hold first"
        );
        assert_eq!(
            everything(&conn),
            before,
            "{case}: the export that listed the hold first"
        );
    }
}

/// A hold of 20 Oct counted by a check on 22 Oct, removed by the export of 23 Oct, which no longer
/// lists it, and listed booked by a later export of the same day.
#[test]
fn a_removed_hold_the_bank_books_later_keeps_a_check_made_while_it_was_pending_verified() {
    for booked_on in ["2026-10-20", "2026-10-21"] {
        let conn = initialize_in_memory().unwrap();
        let card_account = own_without_iban(&conn, "Card");
        let billa = || card_row(1, "2026-10-19", -500, CARD, CardRow::Booked("BILLA"));
        let lidl = || card_row(2, "2026-10-23", -700, CARD, CardRow::Booked("LIDL"));
        import(
            &conn,
            card_account,
            &[
                card_row(0, "2026-10-20", -1_999, CARD, CardRow::Pending),
                billa(),
            ],
        );
        check_now(&conn, card_account, "2026-10-22", -2_499);
        let removed = import_with_defaults(&conn, card_account, &[billa(), lidl()]);
        assert_eq!(removed.removed_holds, 1, "booked on {booked_on}");

        let booked = card_row(3, booked_on, -1_999, CARD, CardRow::Booked("SHOP"));
        assert_eq!(
            import(&conn, card_account, &[billa(), lidl(), booked]),
            1,
            "booked on {booked_on}"
        );
        assert_eq!(
            account_total(&conn, card_account),
            -3_199,
            "booked on {booked_on}"
        );
        assert!(
            is_verified(&conn, card_account),
            "booked on {booked_on}: the check of 22 Oct"
        );
    }
}

#[test]
fn the_storage_keeps_a_bank_entry_in_one_hold_state() {
    let conn = initialize_in_memory().unwrap();
    let checking = own(&conn, "Checking");
    let event_id = cashflow(&conn, checking, "2026-09-14T12:00:00", -7_896);
    let write = |columns: &str, values: &str| {
        conn.execute("DELETE FROM bank_entry", []).unwrap();
        conn.execute(
            &format!(
                "INSERT INTO bank_entry (event_id, day, amount_minor, {columns})
                 VALUES ({event_id}, '2026-09-14', -7896, {values})"
            ),
            [],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    };
    let stamp = "'2026-11-22T10:00:00.000'";
    let posted = "posted_at, posting_change_minor, held_day, held_amount_minor";
    let posting = format!("{stamp}, 0, '2026-09-13', -7896");
    let cancelled = "cancelled_at, cancelled_by_day";
    let cancelling = format!("{stamp}, '2026-09-20'");

    let replacing = format!("{posted}, replaces_event_id");
    let allowed = [
        ("text", "'TCHIBO'"),
        ("card, hold_seen_to", "'433703******2103', '2026-09-16'"),
        (posted, &posting as &str),
        (cancelled, &cancelling as &str),
        (
            &replacing as &str,
            &format!("{posting}, {event_id}") as &str,
        ),
        (
            "hold_seen_to, replaces_event_id",
            &format!("'2026-09-16', {event_id}") as &str,
        ),
    ];
    for (columns, values) in allowed {
        assert_eq!(write(columns, values), Ok(()), "{columns}");
    }

    let refused = [
        ("hold_seen_to", "'2026-9-16'"),
        (posted, "'2026-11-22 10:00:00', 0, '2026-09-13', -7896"),
        (posted, &format!("{stamp}, 0, '2026-9-13', -7896") as &str),
        ("posted_at", stamp),
        ("posting_change_minor", "-1"),
        ("held_day, held_amount_minor", "'2026-09-13', -7896"),
        (
            "posted_at, posting_change_minor",
            &format!("{stamp}, 0") as &str,
        ),
        (
            &format!("hold_seen_to, {posted}") as &str,
            &format!("'2026-09-16', {posting}") as &str,
        ),
        ("cancelled_at", stamp),
        ("cancelled_by_day", "'2026-09-20'"),
        (cancelled, &format!("{stamp}, '2026-9-20'") as &str),
        (
            &format!("hold_seen_to, {cancelled}") as &str,
            &format!("'2026-09-16', {cancelling}") as &str,
        ),
        (
            &format!("{posted}, {cancelled}") as &str,
            &format!("{posting}, {cancelling}") as &str,
        ),
        ("replaces_event_id", &event_id.to_string() as &str),
    ];
    for (columns, values) in refused {
        assert!(
            write(columns, values).is_err_and(|error| error.contains("CHECK")),
            "{columns} = {values} was stored"
        );
    }
}

#[test]
fn the_wire_shapes_are_the_ones_the_wizard_sends_and_reads() {
    let input: ApplyCsvImportInput = serde_json::from_value(json!({
        "accountId": 4,
        "rows": [{
            "index": 7, "eventDate": "2026-03-01T12:00:00", "amountMinor": -1000, "note": null,
            "counterpartAccountId": 5, "namesUnknownAccount": false, "originalCurrencyId": 2,
            "originalAmountMinor": -3333, "fxRateMantissa": 3333, "fxRateExponent": -3,
            "card": "433703******2103", "pending": true
        }],
        "choices": [{
            "index": 7, "selected": true, "bucketId": null,
            "split": [{ "amountMinor": -600, "note": "a", "counterpartAccountId": null, "bucketId": 9 }]
        }],
        "cancelledHolds": [{ "eventId": 12, "remove": false }],
        "reviewToken": "token"
    }))
    .unwrap();
    assert_eq!(
        (
            input.account_id,
            input.rows[0].index,
            input.rows[0].counterpart_account_id,
            input.rows[0].names_unknown_account,
            input.rows[0].fx_rate_exponent,
            input.rows[0].card.as_deref(),
            input.rows[0].pending,
            input.choices[0].split.as_ref().unwrap()[0].bucket_id,
            (
                input.cancelled_holds[0].event_id,
                input.cancelled_holds[0].remove
            ),
            input.review_token.as_str(),
        ),
        (
            4,
            7,
            Some(5),
            false,
            Some(-3),
            Some("433703******2103"),
            true,
            Some(9),
            (12, false),
            "token"
        )
    );

    let preview = CsvImportPreview {
        rows: vec![RowPreview {
            index: 7,
            verdict: Verdict::New,
        }],
        balance_update_days: vec!["2026-03-01".to_owned()],
        cancelled_holds: vec![CancelledHold {
            event_id: 12,
            event_date: "2026-03-01T12:33:53".to_owned(),
            amount_minor: -3_300,
            note: Some("GITHUB".to_owned()),
        }],
        review_token: "token".to_owned(),
    };
    let verdicts = [
        Verdict::CompletesTransfer {
            event_id: 1,
            account_id: 2,
            account_name: "Savings".to_owned(),
            event_date: "2026-03-03T12:00:00".to_owned(),
            note: None,
        },
        Verdict::InSaldo {
            event_id: 3,
            event_date: "2026-03-01T12:00:00".to_owned(),
            note: Some("rent".to_owned()),
        },
        Verdict::TransferLeg {
            event_id: 4,
            other_account_name: "Checking".to_owned(),
            event_date: "2026-03-01T12:00:00".to_owned(),
        },
        Verdict::HoldPosted {
            event_id: 5,
            event_date: "2026-09-14T12:00:00".to_owned(),
            note: Some("TCHIBO".to_owned()),
        },
        Verdict::HoldPostedAtOtherAmount {
            event_id: 6,
            event_date: "2026-04-30T12:33:53".to_owned(),
            note: None,
            hold_amount_minor: -3_300,
        },
        Verdict::NewReplacingSplitHold {
            event_id: 8,
            event_date: "2026-04-30T12:33:53".to_owned(),
            hold_amount_minor: -3_300,
        },
    ];

    assert_eq!(
        serde_json::to_value(&preview).unwrap(),
        json!({
            "rows": [{ "index": 7, "verdict": { "kind": "new" } }],
            "balanceUpdateDays": ["2026-03-01"],
            "cancelledHolds": [{ "eventId": 12, "eventDate": "2026-03-01T12:33:53",
                                 "amountMinor": -3300, "note": "GITHUB" }],
            "reviewToken": "token"
        })
    );
    assert_eq!(
        serde_json::to_value(&verdicts).unwrap(),
        json!([
            { "kind": "completesTransfer", "eventId": 1, "accountId": 2, "accountName": "Savings",
              "eventDate": "2026-03-03T12:00:00", "note": null },
            { "kind": "inSaldo", "eventId": 3, "eventDate": "2026-03-01T12:00:00", "note": "rent" },
            { "kind": "transferLeg", "eventId": 4, "otherAccountName": "Checking",
              "eventDate": "2026-03-01T12:00:00" },
            { "kind": "holdPosted", "eventId": 5, "eventDate": "2026-09-14T12:00:00",
              "note": "TCHIBO" },
            { "kind": "holdPostedAtOtherAmount", "eventId": 6, "eventDate": "2026-04-30T12:33:53",
              "note": null, "holdAmountMinor": -3300 },
            { "kind": "newReplacingSplitHold", "eventId": 8, "eventDate": "2026-04-30T12:33:53",
              "holdAmountMinor": -3300 }
        ])
    );
    assert_eq!(
        serde_json::to_value(CsvImportResult {
            imported: 3,
            removed_holds: 1,
            repriced_holds: 2
        })
        .unwrap(),
        json!({ "imported": 3, "removedHolds": 1, "repricedHolds": 2 })
    );
}

mod export_sequences;
mod harness;
