use serde_json::{json, Value};

use super::*;
use crate::db::initialize_in_memory;
use crate::features::accounts::repository::{create_account, CreateAccountParams};
use crate::features::persons::repository::{create_person, CreatePersonParams};
use crate::features::tax_models::models::ActualMonths;
use crate::features::tax_models::repository::{
    create_tax_model, BracketInput, CreateTaxModelParams,
};
use crate::features::transactions::models::DocumentEntry;
use crate::features::transactions::repository::{
    create_balance_update, create_cashflow, generate_depreciation_events, update_taxable_event,
    update_taxable_split_group, CashflowEntry, CreateBalanceUpdateParams, NewSplitLeg,
    UpdateTaxableEventParams, UpdateTaxableSplitGroupParams,
};

struct Books {
    person_id: i64,
    bank_account_id: i64,
    tax_model_id: i64,
}

fn books(conn: &Connection) -> Books {
    let person_id = create_person(
        conn,
        CreatePersonParams {
            name: "Firma s.r.o.".to_owned(),
            person_type: "legal".to_owned(),
            vat_payer: true,
        },
    )
    .unwrap();
    let bank_account_id = create_account(
        conn,
        CreateAccountParams {
            name: "Tatra business".to_owned(),
            currency_id: 1,
            account_type: "account".to_owned(),
            person_id: Some(person_id),
            ..Default::default()
        },
    )
    .unwrap();
    let tax_model_id = create_tax_model(
        conn,
        CreateTaxModelParams {
            name: "2026".to_owned(),
            calendar_year: 2026,
            person_id,
            vat_status: "all_year".to_owned(),
            vat_from_date: None,
            reserve_fund_current_minor: None,
            reserve_fund_pct_bps: None,
            reserve_fund_max_minor: None,
            dividend_tax_rate_bps: None,
            brackets: vec![BracketInput {
                sort_order: 0,
                lower_bound_minor: 0,
                rate_type: "flat".to_owned(),
                flat_rate_bps: Some(2100),
                tiers_json: None,
            }],
            minimum_tax_bands: vec![],
            actuals_through_month: ActualMonths::new(9).unwrap(),
        },
    )
    .unwrap();
    Books {
        person_id,
        bank_account_id,
        tax_model_id,
    }
}

fn payment(conn: &Connection, books: &Books, amount_minor: i64) -> i64 {
    create_cashflow(
        conn,
        &CashflowEntry {
            account_id: books.bank_account_id,
            amount_minor,
            event_date: "2026-10-14".to_owned(),
            note: Some("BANK TRANSFER".to_owned()),
            counterpart_account_id: None,
            bucket_id: None,
            original_currency_id: None,
            original_amount_minor: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
        },
    )
    .unwrap()
}

fn expense_line(gross_minor: i64, vat_rate_bps: i64) -> Value {
    json!({ "grossMinor": gross_minor, "vatRateBps": vat_rate_bps,
            "deductiblePctBps": 10000, "vatReclaimablePctBps": 10000 })
}

fn item(books: &Books, issuer: &str, number: &str, content: Value) -> Value {
    json!({
        "document": { "issuer": issuer, "number": number, "file": format!("{number}.pdf") },
        "personId": books.person_id,
        "issueDate": "2026-10-12",
        "note": format!("{issuer} {number}"),
        "content": content,
    })
}

fn with(mut item: Value, key: &str, value: Value) -> Value {
    item[key] = value;
    item
}

fn single_expense(books: &Books, issuer: &str, number: &str, gross_minor: i64) -> Value {
    item(
        books,
        issuer,
        number,
        json!({ "kind": "expense", "lines": [expense_line(gross_minor, 2300)] }),
    )
}

fn split_expense(books: &Books) -> Value {
    item(
        books,
        "31347037",
        "2026/10/551",
        json!({ "kind": "expense",
                "lines": [expense_line(12_300, 2300), expense_line(2_140, 700)] }),
    )
}

fn prepaid_expense(books: &Books) -> Value {
    with(
        item(
            books,
            "35763469",
            "PZ-2026-118",
            json!({ "kind": "prepaid_expense", "line": expense_line(120_000, 0),
                    "prepaidUntil": "2027-09-30" }),
        ),
        "issueDate",
        json!("2026-10-01"),
    )
}

fn laptop_and_sleeve(books: &Books) -> Value {
    item(
        books,
        "36562939",
        "FA20261187",
        json!({ "kind": "expense",
                "lines": [expense_line(3_690, 2300)],
                "assets": [{ "name": "MacBook Pro 16", "grossMinor": 359_900, "vatRateBps": 2300,
                             "depreciationMonths": 48 }] }),
    )
}

fn batch(items: Vec<Value>, dry_run: bool) -> RecordBatchInput {
    serde_json::from_value(
        json!({ "label": "Invoices 2026-10", "dryRun": dry_run, "items": items }),
    )
    .unwrap()
}

fn run_batch(conn: &Connection, input: RecordBatchInput) -> Result<BatchReceipt, RecordBatchError> {
    record_batch(WriteTxn::begin(conn).unwrap(), input)
}

fn documents(conn: &Connection, books: &Books) -> Vec<DocumentEntry> {
    find_document_entries(
        conn,
        FindDocumentEntriesParams {
            person_id: Some(books.person_id),
            limit: 100,
            ..Default::default()
        },
    )
    .unwrap()
}

fn document<'a>(found: &'a [DocumentEntry], source_ref: &str) -> &'a DocumentEntry {
    found
        .iter()
        .find(|doc| doc.source_ref.as_deref() == Some(source_ref))
        .unwrap_or_else(|| panic!("{source_ref} is not recorded"))
}

fn count(conn: &Connection, sql: &str, id: i64) -> i64 {
    conn.query_row(sql, [id], |row| row.get(0)).unwrap()
}

fn live_prepaid_shares(conn: &Connection, parent_id: i64) -> i64 {
    count(
        conn,
        "SELECT COUNT(*) FROM event WHERE linked_prepaid_event_id = ?1 AND deleted_at IS NULL",
        parent_id,
    )
}

fn depreciation_rows(conn: &Connection, asset_id: i64) -> i64 {
    count(
        conn,
        "SELECT COUNT(*) FROM event WHERE linked_asset_id = ?1 AND deleted_at IS NULL",
        asset_id,
    )
}

fn unlinked_payments(conn: &Connection, books: &Books) -> Vec<i64> {
    let mut ids: Vec<i64> = find_payments(
        conn,
        FindPaymentsParams {
            person_id: Some(books.person_id),
            unlinked_only: true,
            limit: 100,
            ..Default::default()
        },
    )
    .unwrap()
    .into_iter()
    .map(|p| p.event_id)
    .collect();
    ids.sort_unstable();
    ids
}

fn refused(
    result: Result<BatchReceipt, RecordBatchError>,
) -> (Vec<ItemError>, Vec<RecordedDocument>) {
    match result {
        Err(RecordBatchError::Items { errors, passed, .. }) => (errors, passed),
        other => panic!("expected refused items, got {other:?}"),
    }
}

#[test]
fn records_every_kind_of_document_and_links_its_payments() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let sale_paid = payment(&conn, &books, 120_000);
    let metro_paid = payment(&conn, &books, -14_440);
    let alza_paid = payment(&conn, &books, -363_590);
    let revenue = with(
        with(
            item(
                &books,
                "50123456",
                "2026001",
                json!({ "kind": "revenue",
                        "lines": [{ "grossMinor": 120_000, "vatRateBps": 2300 }] }),
            ),
            "deliveryDate",
            json!("2026-09-30"),
        ),
        "paymentIds",
        json!([sale_paid]),
    );
    let credit_note = single_expense(&books, "35697270", "DB2026000311", -615);

    let receipt = run_batch(
        &conn,
        batch(
            vec![
                revenue,
                with(split_expense(&books), "paymentIds", json!([metro_paid])),
                prepaid_expense(&books),
                with(laptop_and_sleeve(&books), "paymentIds", json!([alza_paid])),
                credit_note,
            ],
            false,
        ),
    )
    .unwrap();

    let batch_id = receipt.batch_id.expect("a real batch has an id");
    assert_eq!(receipt.affected_tax_model_ids, [books.tax_model_id]);
    let [sale, metro, insurance, alza, orange] = &receipt.documents[..] else {
        panic!("five documents")
    };
    assert!(receipt.documents.iter().all(|doc| doc.warnings.is_empty()));

    assert_eq!(sale.entry_date, "2026-09-30");
    assert_eq!(sale.date_source, DateSource::DeliveryDate);
    assert!(matches!(
        sale.shape,
        RecordedShape::Single { event_id: Some(_) }
    ));
    assert_eq!(metro.entry_date, "2026-10-12");
    assert_eq!(metro.date_source, DateSource::IssueDate);
    assert!(matches!(
        metro.shape,
        RecordedShape::SplitGroup {
            split_group_id: Some(_)
        }
    ));
    let RecordedShape::Prepaid {
        event_id: Some(prepaid_id),
        generated_shares,
    } = insurance.shape
    else {
        panic!("a prepaid expense")
    };
    assert_eq!(generated_shares, 2, "one share for 2026 and one for 2027");
    assert_eq!(live_prepaid_shares(&conn, prepaid_id), 2);
    assert_eq!(alza.total_gross_minor, 363_590);
    assert_eq!(alza.assets[0].purchase_price_minor, 292_601);
    assert_eq!(alza.assets[0].price_basis, PriceBasis::NetVatReclaimed);
    assert_eq!(orange.total_gross_minor, -615);

    let found = documents(&conn, &books);
    assert_eq!(
        found.len(),
        5,
        "the laptop is an asset, the sleeve an entry"
    );
    assert!(found.iter().all(|doc| doc.batch_id == Some(batch_id)));
    assert_eq!(
        document(&found, "invoice:50123456/2026001").linked_payment_ids,
        [sale_paid]
    );
    let metro_doc = document(&found, "invoice:31347037/2026/10/551");
    assert_eq!(metro_doc.lines.len(), 2);
    assert_eq!(metro_doc.linked_payment_ids, [metro_paid]);
    assert_eq!(
        document(&found, "invoice:36562939/FA20261187").linked_payment_ids,
        [alza_paid]
    );
    assert_eq!(
        document(&found, "invoice:35697270/DB2026000311").total_gross_minor,
        -615
    );
    assert!(unlinked_payments(&conn, &books).is_empty());

    let asset_id = alza.assets[0].account_id.unwrap();
    let asset_ref = SourceRef::invoice("36562939", "FA20261187").unwrap();
    let recorded = find_recorded_source(&conn, &asset_ref).unwrap().unwrap();
    assert!(
        matches!(&recorded, RecordedSource::Entry { .. })
            || matches!(&recorded, RecordedSource::Asset { asset_account_id, .. } if *asset_account_id == asset_id)
    );
    assert!(depreciation_rows(&conn, asset_id) > 0);
    let initial: (i64, String) = conn
        .query_row(
            "SELECT ed.amount_minor, ed.event_date FROM event e
             JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.account_id = ?1 AND e.event_type = 'balance_update'",
            [asset_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(initial, (292_601, "2026-10-12".to_owned()));
}

#[test]
fn a_dry_run_reports_the_documents_and_keeps_nothing() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let paid = payment(&conn, &books, -14_440);

    let receipt = run_batch(
        &conn,
        batch(
            vec![
                with(split_expense(&books), "paymentIds", json!([paid])),
                laptop_and_sleeve(&books),
            ],
            true,
        ),
    )
    .unwrap();

    assert!(receipt.dry_run);
    assert_eq!(receipt.batch_id, None);
    assert_eq!(
        receipt.documents[0].shape,
        RecordedShape::SplitGroup {
            split_group_id: None
        }
    );
    assert_eq!(receipt.documents[0].lines[0].net_minor, 10_000);
    assert_eq!(receipt.documents[1].assets[0].account_id, None);
    assert_eq!(receipt.documents[1].assets[0].purchase_price_minor, 292_601);
    assert!(documents(&conn, &books).is_empty());
    assert!(list_recent_batches(&conn, 10).unwrap().is_empty());
    let asset_ref = SourceRef::invoice("36562939", "FA20261187").unwrap();
    assert_eq!(find_recorded_source(&conn, &asset_ref).unwrap(), None);
    assert_eq!(unlinked_payments(&conn, &books), [paid]);
}

#[test]
fn a_document_already_in_saldo_is_refused_and_nothing_of_the_batch_is_written() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let first = run_batch(
        &conn,
        batch(
            vec![single_expense(&books, "35697270", "FV1", 2_399)],
            false,
        ),
    )
    .unwrap();

    let (errors, passed) = refused(run_batch(
        &conn,
        batch(
            vec![
                single_expense(&books, "35697270", "FV2", 2_399),
                single_expense(&books, "35697270", " fv 1", 2_399),
            ],
            false,
        ),
    ));

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].index, 1);
    assert_eq!(errors[0].code, "ALREADY_RECORDED");
    let Some(RecordedSource::Entry { batch_id, .. }) = &errors[0].recorded else {
        panic!("the earlier document is an entry");
    };
    assert_eq!(*batch_id, first.batch_id);
    assert_eq!(passed[0].index, 0);
    assert_eq!(documents(&conn, &books).len(), 1);
    assert_eq!(list_recent_batches(&conn, 10).unwrap().len(), 1);
}

#[test]
fn the_same_document_twice_in_one_batch_is_refused() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);

    let (errors, _) = refused(run_batch(
        &conn,
        batch(
            vec![
                single_expense(&books, "35697270", "FV1", 2_399),
                single_expense(&books, "35697270", "FV1", 2_399),
            ],
            true,
        ),
    ));

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].index, 1);
    assert_eq!(errors[0].code, "DUPLICATE_IN_BATCH");
    assert!(documents(&conn, &books).is_empty());
}

#[test]
fn every_failing_document_is_reported_and_nothing_is_written() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let cheap_asset = item(
        &books,
        "36562939",
        "FA2",
        json!({ "kind": "expense",
                "assets": [{ "name": "Monitor", "grossMinor": 30_000, "vatRateBps": 2300,
                             "depreciationMonths": 48 }] }),
    );
    let prepaid_within_the_year = item(
        &books,
        "35763469",
        "PZ1",
        json!({ "kind": "prepaid_expense", "line": expense_line(10_000, 0),
                "prepaidUntil": "2026-12-31" }),
    );

    let (errors, passed) = refused(run_batch(
        &conn,
        batch(
            vec![
                single_expense(&books, "35697270", "FV1", 2_399),
                cheap_asset,
                split_expense(&books),
                prepaid_within_the_year,
            ],
            false,
        ),
    ));

    let failed: Vec<(usize, &str)> = errors.iter().map(|e| (e.index, e.code.as_str())).collect();
    assert_eq!(failed, [(1, "VALIDATION"), (3, "VALIDATION")]);
    assert_eq!(
        errors[1].source_ref.as_ref().unwrap().as_str(),
        "invoice:35763469/PZ1"
    );
    let passed: Vec<usize> = passed.iter().map(|doc| doc.index).collect();
    assert_eq!(passed, [0, 2]);
    assert!(documents(&conn, &books).is_empty());
    assert!(list_recent_batches(&conn, 10).unwrap().is_empty());
}

#[test]
fn payments_that_do_not_add_up_or_are_already_linked_are_warnings() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let short = payment(&conn, &books, -2_000);
    let first = run_batch(
        &conn,
        batch(
            vec![with(
                single_expense(&books, "35697270", "FV1", 2_399),
                "paymentIds",
                json!([short]),
            )],
            false,
        ),
    )
    .unwrap();

    let second = run_batch(
        &conn,
        batch(
            vec![with(
                single_expense(&books, "35697270", "FV2", 2_000),
                "paymentIds",
                json!([short]),
            )],
            true,
        ),
    )
    .unwrap();

    assert_eq!(
        first.documents[0].warnings,
        [Warning::PaymentAmountDiffers {
            payments_total_minor: -2_000,
            document_total_minor: 2_399,
        }]
    );
    let linked_entry = match first.documents[0].shape {
        RecordedShape::Single { event_id } => event_id.unwrap(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        second.documents[0].warnings,
        [Warning::PaymentAlreadyLinked {
            payment_id: short,
            linked_entry_ids: vec![linked_entry],
        }]
    );
}

#[test]
fn a_payment_of_another_person_is_refused() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let other = Books {
        person_id: 1,
        ..self::books(&conn)
    };
    let not_ours = payment(&conn, &other, -2_399);

    let (errors, _) = refused(run_batch(
        &conn,
        batch(
            vec![with(
                single_expense(&books, "35697270", "FV1", 2_399),
                "paymentIds",
                json!([not_ours]),
            )],
            true,
        ),
    ));

    assert_eq!(errors[0].code, "VALIDATION");
}

fn record(conn: &Connection, items: Vec<Value>) -> BatchReceipt {
    run_batch(conn, batch(items, false)).unwrap()
}

fn undo(
    conn: &Connection,
    batch_id: i64,
    include_changed: bool,
) -> Result<UndoReceipt, UndoBatchError> {
    undo_batch(
        WriteTxn::begin(conn).unwrap(),
        UndoBatchInput {
            batch_id,
            include_changed,
        },
    )
}

#[test]
fn undo_removes_everything_the_batch_recorded() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let paid = payment(&conn, &books, -14_440);
    let items = || {
        vec![
            with(split_expense(&books), "paymentIds", json!([paid])),
            prepaid_expense(&books),
            laptop_and_sleeve(&books),
            single_expense(&books, "35697270", "FV1", 2_399),
        ]
    };
    let receipt = record(&conn, items());
    let RecordedShape::Prepaid {
        event_id: Some(prepaid_id),
        ..
    } = receipt.documents[1].shape
    else {
        panic!("a prepaid expense")
    };
    let asset_id = receipt.documents[2].assets[0].account_id.unwrap();

    let undone = undo(&conn, receipt.batch_id.unwrap(), false).unwrap();

    assert!(!undone.already_undone);
    assert_eq!(undone.removed_asset_account_ids, [asset_id]);
    assert_eq!(undone.affected_tax_model_ids, [books.tax_model_id]);
    assert!(documents(&conn, &books).is_empty());
    assert_eq!(live_prepaid_shares(&conn, prepaid_id), 0);
    assert_eq!(depreciation_rows(&conn, asset_id), 0);
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM account WHERE id = ?1",
            asset_id
        ),
        0
    );
    assert_eq!(unlinked_payments(&conn, &books), [paid]);
    assert!(list_recent_batches(&conn, 1).unwrap()[0]
        .undone_at
        .is_some());

    record(&conn, items());
    assert_eq!(
        documents(&conn, &books).len(),
        4,
        "an undone batch can be recorded again"
    );
}

#[test]
fn a_second_undo_changes_nothing() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let batch_id = record(
        &conn,
        vec![single_expense(&books, "35697270", "FV1", 2_399)],
    )
    .batch_id
    .unwrap();
    undo(&conn, batch_id, false).unwrap();

    let again = undo(&conn, batch_id, false).unwrap();

    assert!(again.already_undone);
    assert!(again.removed_event_ids.is_empty());
}

#[test]
fn an_unknown_batch_is_not_found() {
    let conn = initialize_in_memory().unwrap();

    assert!(matches!(
        undo(&conn, 42, false),
        Err(UndoBatchError::NotFound { batch_id: 42 })
    ));
}

#[test]
fn undo_stops_at_rows_changed_in_saldo_until_told_to_remove_them_too() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let receipt = record(
        &conn,
        vec![
            single_expense(&books, "35697270", "FV1", 2_399),
            split_expense(&books),
            laptop_and_sleeve(&books),
        ],
    );
    let batch_id = receipt.batch_id.unwrap();
    let RecordedShape::Single {
        event_id: Some(single_id),
    } = receipt.documents[0].shape
    else {
        panic!("a single entry")
    };
    let RecordedShape::SplitGroup {
        split_group_id: Some(group_id),
    } = receipt.documents[1].shape
    else {
        panic!("a split group")
    };
    let asset_id = receipt.documents[2].assets[0].account_id.unwrap();
    update_taxable_event(
        &conn,
        UpdateTaxableEventParams {
            event_id: single_id,
            event_type: "expense".to_owned(),
            amount_minor: 2_500,
            event_date: "2026-10-12".to_owned(),
            ..Default::default()
        },
    )
    .unwrap();
    update_taxable_split_group(
        &conn,
        UpdateTaxableSplitGroupParams {
            split_group_id: group_id,
            event_type: "expense".to_owned(),
            new_legs: vec![NewSplitLeg {
                amount_minor: 500,
                event_date: "2026-10-12".to_owned(),
                note: None,
                vat_rate_bps: Some(2300),
                vat_reclaimable_pct_bps: None,
                expense_deductible_pct_bps: None,
                prepaid_until: None,
                reclaimed_vat: None,
            }],
            ..Default::default()
        },
    )
    .unwrap();
    create_balance_update(
        &conn,
        CreateBalanceUpdateParams {
            account_id: asset_id,
            amount_minor: 250_000,
            event_date: "2026-12-31".to_owned(),
            note: None,
        },
    )
    .unwrap();

    let Err(UndoBatchError::Changed { rows, .. }) = undo(&conn, batch_id, false) else {
        panic!("the undo must stop")
    };

    let reasons: Vec<ChangeReason> = rows.iter().map(|row| row.reason).collect();
    assert_eq!(rows[0].event_id, single_id);
    assert_eq!(
        reasons,
        [
            ChangeReason::EditedInSaldo,
            ChangeReason::LegAddedInSaldo,
            ChangeReason::EventAddedToAsset
        ]
    );
    assert_eq!(documents(&conn, &books).len(), 3, "nothing was removed");

    undo(&conn, batch_id, true).unwrap();

    assert!(
        documents(&conn, &books).is_empty(),
        "the added leg went too"
    );
}

#[test]
fn depreciation_saldo_writes_later_is_not_a_change() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let earlier = with(laptop_and_sleeve(&books), "issueDate", json!("2025-03-10"));
    let receipt = record(&conn, vec![earlier]);
    let asset_id = receipt.documents[0].assets[0].account_id.unwrap();
    let before = depreciation_rows(&conn, asset_id);
    conn.execute(
        "DELETE FROM event WHERE id = (SELECT MAX(id) FROM event WHERE linked_asset_id = ?1)",
        [asset_id],
    )
    .unwrap();
    generate_depreciation_events(&conn).unwrap();
    assert_eq!(
        depreciation_rows(&conn, asset_id),
        before,
        "the month came back"
    );

    let undone = undo(&conn, receipt.batch_id.unwrap(), false).unwrap();

    assert_eq!(undone.removed_asset_account_ids, [asset_id]);
    assert_eq!(depreciation_rows(&conn, asset_id), 0);
}

#[test]
fn undo_skips_entries_the_user_already_deleted() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let receipt = record(
        &conn,
        vec![
            single_expense(&books, "35697270", "FV1", 2_399),
            single_expense(&books, "35697270", "FV2", 2_399),
        ],
    );
    let ids: Vec<i64> = receipt
        .documents
        .iter()
        .map(|doc| match doc.shape {
            RecordedShape::Single { event_id } => event_id.unwrap(),
            other => panic!("{other:?}"),
        })
        .collect();
    delete_event_inner(&conn, ids[0]).unwrap();

    let undone = undo(&conn, receipt.batch_id.unwrap(), false).unwrap();

    assert_eq!(undone.skipped_already_deleted, [ids[0]]);
    assert_eq!(undone.removed_event_ids, [ids[1]]);
}

fn parse(items: Value) -> Result<RecordBatchInput, String> {
    let input = json!({ "label": "x", "items": [items] });
    serde_path_to_error::deserialize::<_, RecordBatchInput>(input)
        .map_err(|e| format!("{}: {}", e.path(), e.inner()))
}

fn item_with(content: Value) -> Value {
    json!({
        "document": { "issuer": "1", "number": "1", "file": "a.pdf" },
        "personId": 1,
        "issueDate": "2026-10-12",
        "content": content,
    })
}

#[test]
fn a_share_out_of_range_is_refused_where_it_stands() {
    let mut line = expense_line(100, 2300);
    line["vatReclaimablePctBps"] = json!(12_000);

    let err = parse(item_with(json!({ "kind": "expense", "lines": [line] }))).unwrap_err();

    assert!(
        err.starts_with("items[0].content: lines[0].vatReclaimablePctBps: 12000 "),
        "{err}"
    );
}

#[test]
fn an_unknown_kind_is_refused() {
    let err = parse(item_with(json!({ "kind": "invoice", "lines": [] }))).unwrap_err();

    assert!(err.starts_with("items[0].content: "), "{err}");
}

#[test]
fn an_expense_line_needs_both_shares() {
    let line = json!({ "grossMinor": 100, "vatRateBps": 2300, "deductiblePctBps": 10000 });

    assert!(parse(item_with(json!({ "kind": "expense", "lines": [line] }))).is_err());
}

#[test]
fn a_misspelt_field_is_refused_instead_of_ignored() {
    let asset = json!({ "name": "MacBook", "grossMinor": 359_900, "vatRateBps": 2300,
                        "depreciationMonth": 48 });

    assert!(parse(item_with(json!({ "kind": "expense", "assets": [asset] }))).is_err());
    assert!(parse(item_with(
        json!({ "kind": "expense", "lines": [], "reclaimVat": true })
    ))
    .is_err());
}

#[test]
fn a_revenue_line_cannot_carry_expense_shares_and_a_prepaid_expense_cannot_be_split() {
    let revenue = json!({ "kind": "revenue", "lines": [expense_line(100, 2300)] });
    let split_prepaid = json!({ "kind": "prepaid_expense", "lines": [expense_line(100, 0)],
                                "prepaidUntil": "2027-09-30" });

    assert!(parse(item_with(revenue)).is_err());
    assert!(parse(item_with(split_prepaid)).is_err());
}

#[test]
fn a_well_formed_item_parses() {
    let content = json!({ "kind": "prepaid_expense", "line": expense_line(100, 0),
                          "prepaidUntil": "2027-09-30", "reclaimedVat": false });

    assert!(parse(item_with(content)).is_ok());
}
