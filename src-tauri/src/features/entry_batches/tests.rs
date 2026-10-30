use serde_json::{json, Value};

use super::*;
use crate::db::initialize_in_memory;
use crate::features::accounts::repository::{
    create_account, update_account, CreateAccountParams, UpdateAccountParams,
};
use crate::features::ekasa::models::EkasaRuleInput;
use crate::features::ekasa::reading::fake::{read_offline, FakeEkasaApi};
use crate::features::ekasa::reading::ReceiptReading;
use crate::features::ekasa::repository::{upsert_ekasa_profile, UpsertEkasaProfileParams};
use crate::features::persons::repository::{
    create_person, update_person, CreatePersonParams, UpdatePersonParams,
};
use crate::features::tax_models::models::ActualMonths;
use crate::features::tax_models::repository::{
    create_tax_model, BracketInput, CreateTaxModelParams,
};
use crate::features::transactions::models::DocumentEntry;
use crate::features::transactions::repository::{
    create_balance_update, create_cashflow, generate_depreciation_events,
    recalculate_depreciation_for_asset, unlink_cashflow_from_taxable, update_taxable_event,
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
    Books {
        person_id,
        bank_account_id,
        tax_model_id: tax_model(conn, person_id, 2026),
    }
}

fn tax_model(conn: &Connection, person_id: i64, calendar_year: i64) -> i64 {
    create_tax_model(
        conn,
        CreateTaxModelParams {
            name: calendar_year.to_string(),
            calendar_year,
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
    .unwrap()
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

struct Batch {
    items: Vec<Value>,
    dry_run: bool,
}

fn batch(items: Vec<Value>, dry_run: bool) -> Batch {
    Batch { items, dry_run }
}

fn input(items: &[Value], dry_run: bool, review_token: Option<String>) -> RecordBatchInput {
    serde_json::from_value(json!({ "label": "Invoices 2026-10", "dryRun": dry_run,
                                   "reviewToken": review_token, "items": items }))
    .unwrap()
}

fn submit(conn: &Connection, input: RecordBatchInput) -> Result<BatchReceipt, RecordBatchError> {
    record_batch(WriteTxn::begin(conn).unwrap(), input)
}

/// A real write carries the reviewToken of its own dry run, as Claude sends it after the
/// user's yes.
fn run_batch(conn: &Connection, batch: Batch) -> Result<BatchReceipt, RecordBatchError> {
    let dry_run = submit(conn, input(&batch.items, true, None));
    if batch.dry_run {
        return dry_run;
    }
    let reviewed = dry_run.expect("the dry run before a write passes");
    submit(conn, input(&batch.items, false, reviewed.review_token))
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
    assert_eq!(
        serde_json::to_value(&receipt.documents[0].lines[0]).unwrap(),
        json!({ "grossMinor": 12_300, "netMinor": 10_000, "vatRateBps": 2300 }),
        "an invoice line reports neither the document's note nor its shares"
    );
    assert_eq!(receipt.documents[1].assets[0].account_id, None);
    assert_eq!(receipt.documents[1].assets[0].purchase_price_minor, 292_601);
    assert!(documents(&conn, &books).is_empty());
    assert!(list_recent_batches(&conn, 10).unwrap().is_empty());
    let asset_ref = SourceRef::invoice("36562939", "FA20261187").unwrap();
    assert_eq!(find_recorded_source(&conn, &asset_ref).unwrap(), None);
    assert_eq!(unlinked_payments(&conn, &books), [paid]);
}

fn reviewed_write(items: Vec<Value>, review_token: Option<String>) -> RecordBatchInput {
    input(&items, false, review_token)
}

#[test]
fn a_write_needs_the_token_its_dry_run_returned() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let items = || vec![split_expense(&books), laptop_and_sleeve(&books)];
    let reviewed = run_batch(&conn, batch(items(), true)).unwrap();

    let written = submit(&conn, reviewed_write(items(), reviewed.review_token)).unwrap();

    assert!(written.batch_id.is_some());
    assert_eq!(written.review_token, None);
    assert_eq!(documents(&conn, &books).len(), 2);
}

#[test]
fn a_write_without_the_reviewed_token_is_refused_and_writes_nothing() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let reviewed = run_batch(
        &conn,
        batch(vec![single_expense(&books, "35697270", "FV1", 2_399)], true),
    )
    .unwrap();

    let missing = submit(
        &conn,
        reviewed_write(vec![single_expense(&books, "35697270", "FV1", 2_399)], None),
    );
    let other_amount = submit(
        &conn,
        reviewed_write(
            vec![single_expense(&books, "35697270", "FV1", 2_400)],
            reviewed.review_token,
        ),
    );

    assert!(matches!(
        missing,
        Err(RecordBatchError::ReviewTokenMismatch)
    ));
    assert!(matches!(
        other_amount,
        Err(RecordBatchError::ReviewTokenMismatch)
    ));
    assert!(documents(&conn, &books).is_empty());
    assert!(list_recent_batches(&conn, 10).unwrap().is_empty());
}

#[test]
fn a_document_recorded_after_its_dry_run_is_refused_at_the_write_and_nothing_is_written() {
    for receipt in [false, true] {
        let conn = initialize_in_memory().unwrap();
        let books = books(&conn);
        let late = if receipt {
            receipt_item(
                &books,
                &billa(),
                &[by("claude", 0, 0), by("claude", 0, 0), by("claude", 0, 0)],
            )
        } else {
            single_expense(&books, "35697270", " fv 1", 2_399)
        };
        let items = || {
            vec![
                single_expense(&books, "35697270", "FV2", 2_399),
                late.clone(),
            ]
        };
        let reviewed = run_batch(&conn, batch(items(), true)).unwrap();
        let first = record(&conn, vec![late.clone()]);

        let refusal = submit(&conn, reviewed_write(items(), reviewed.review_token));

        let Err(RecordBatchError::Items {
            dry_run: false,
            errors,
            passed,
        }) = refusal
        else {
            panic!("expected the write's item errors, got {refusal:?}");
        };
        let failed: Vec<(usize, &str)> =
            errors.iter().map(|e| (e.index, e.code.as_str())).collect();
        assert_eq!(failed, [(1, "ALREADY_RECORDED")], "receipt: {receipt}");
        assert!(matches!(
            &errors[0].recorded,
            Some(RecordedSource::Entry { batch_id, .. }) if *batch_id == first.batch_id
        ));
        let passed: Vec<usize> = passed.iter().map(|doc| doc.index).collect();
        assert_eq!(passed, [0]);
        let fv2 = SourceRef::invoice("35697270", "FV2").unwrap();
        assert_eq!(find_recorded_source(&conn, &fv2).unwrap(), None);
        assert_eq!(list_recent_batches(&conn, 10).unwrap().len(), 1);
    }
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
            true,
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

fn revenue(books: &Books, number: &str, gross_minor: i64) -> Value {
    item(
        books,
        "50123456",
        number,
        json!({ "kind": "revenue", "lines": [{ "grossMinor": gross_minor, "vatRateBps": 2300 }] }),
    )
}

fn single_event_id(document: &RecordedDocument) -> i64 {
    match document.shape {
        RecordedShape::Single { event_id } => event_id.unwrap(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_payment_the_wrong_way_round_is_a_warning() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let received = payment(&conn, &books, 120_000);
    let paid_out = payment(&conn, &books, -2_399);
    let refunded = payment(&conn, &books, 615);
    let sent_back = payment(&conn, &books, -615);
    let linked =
        |document: Value, payment_id: i64| with(document, "paymentIds", json!([payment_id]));

    let receipt = run_batch(
        &conn,
        batch(
            vec![
                linked(revenue(&books, "2026001", 120_000), received),
                linked(revenue(&books, "2026002", 2_399), paid_out),
                linked(single_expense(&books, "35697270", "FV1", 2_399), paid_out),
                linked(single_expense(&books, "35697270", "DB1", -615), refunded),
                linked(single_expense(&books, "35697270", "DB2", -615), sent_back),
            ],
            true,
        ),
    )
    .unwrap();

    let direction: Vec<Vec<&Warning>> = receipt
        .documents
        .iter()
        .map(|doc| {
            doc.warnings
                .iter()
                .filter(|w| matches!(w, Warning::PaymentDirectionDiffers { .. }))
                .collect()
        })
        .collect();
    assert!(direction[0].is_empty(), "revenue comes in");
    assert_eq!(
        direction[1],
        [&Warning::PaymentDirectionDiffers {
            payment_id: paid_out,
            amount_minor: -2_399,
        }]
    );
    assert!(direction[2].is_empty(), "an expense is paid out");
    assert!(
        direction[3].is_empty(),
        "an expense credit note is refunded"
    );
    assert_eq!(
        direction[4],
        [&Warning::PaymentDirectionDiffers {
            payment_id: sent_back,
            amount_minor: -615,
        }]
    );
}

#[test]
fn an_entry_of_the_same_kind_and_total_nearby_is_similar_whatever_its_reference() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let keyed_by_name = record(
        &conn,
        vec![single_expense(
            &books,
            "Orange Slovensko a.s.",
            "FV1",
            2_399,
        )],
    );
    let typed_by_hand = create_taxable_event_inner(
        &conn,
        CreateTaxableEventInput {
            person_id: books.person_id,
            event_type: "expense".to_owned(),
            amount_minor: 2_399,
            event_date: "2026-10-20".to_owned(),
            note: Some("Orange".to_owned()),
            vat_rate_bps: Some(2300),
            vat_reclaimable_pct_bps: Some(10000),
            expense_deductible_pct_bps: Some(10000),
            prepaid_until: None,
            reclaimed_vat: Some(true),
            source_ref: None,
        },
    )
    .unwrap();
    record(
        &conn,
        vec![
            revenue(&books, "2026001", 2_399),
            with(
                single_expense(&books, "35697270", "FV0", 2_399),
                "issueDate",
                json!("2026-09-20"),
            ),
            single_expense(&books, "35697270", "FV2", 2_400),
        ],
    );

    let receipt = run_batch(
        &conn,
        batch(vec![single_expense(&books, "35697270", "FV1", 2_399)], true),
    )
    .unwrap();

    assert_eq!(
        receipt.documents[0].warnings,
        [
            Warning::SimilarDocument {
                event_id: typed_by_hand,
                source_ref: None,
                date: "2026-10-20".to_owned(),
            },
            Warning::SimilarDocument {
                event_id: single_event_id(&keyed_by_name.documents[0]),
                source_ref: Some("invoice:ORANGESLOVENSKOA.S./FV1".to_owned()),
                date: "2026-10-12".to_owned(),
            },
        ]
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
    let revalued = create_balance_update(
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

    let [edited, leg_added, event_added] = &rows[..] else {
        panic!("three changes: {rows:?}")
    };
    assert_eq!(
        *edited,
        ChangedRow::EditedInSaldo {
            event_id: single_id
        }
    );
    assert!(matches!(leg_added, ChangedRow::LegAddedInSaldo { .. }));
    assert_eq!(
        *event_added,
        ChangedRow::EventAddedToAsset {
            event_id: revalued,
            account_id: asset_id
        }
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

/// Saves the laptop the way the asset form does, after `edit`, and lets Saldo regenerate its
/// depreciation.
fn save_laptop(
    conn: &Connection,
    books: &Books,
    asset_id: i64,
    edit: fn(&mut UpdateAccountParams),
) {
    let mut params = UpdateAccountParams {
        account_id: asset_id,
        name: "MacBook Pro 16".to_owned(),
        iban: None,
        person_id: Some(books.person_id),
        purchase_price_minor: Some(292_601),
        purchase_date: Some("2026-10-12".to_owned()),
        depreciation_period_months: Some(48),
    };
    edit(&mut params);
    update_account(conn, params).unwrap();
    recalculate_depreciation_for_asset(conn, asset_id).unwrap();
}

#[test]
fn undo_stops_at_an_asset_edited_in_saldo() {
    let edits: [(&str, fn(&mut UpdateAccountParams)); 4] = [
        ("name", |p| p.name = "MacBook Pro 14".to_owned()),
        ("purchase price", |p| p.purchase_price_minor = Some(300_000)),
        ("purchase date", |p| {
            p.purchase_date = Some("2026-11-02".to_owned())
        }),
        ("depreciation months", |p| {
            p.depreciation_period_months = Some(36)
        }),
    ];
    for (field, edit) in edits {
        let conn = initialize_in_memory().unwrap();
        let books = books(&conn);
        let receipt = record(&conn, vec![laptop_and_sleeve(&books)]);
        let asset_id = receipt.documents[0].assets[0].account_id.unwrap();
        save_laptop(&conn, &books, asset_id, edit);

        let Err(UndoBatchError::Changed { rows, .. }) =
            undo(&conn, receipt.batch_id.unwrap(), false)
        else {
            panic!("an edited {field} must stop the undo")
        };

        assert_eq!(
            rows,
            [ChangedRow::AssetEditedInSaldo {
                account_id: asset_id
            }],
            "{field}"
        );
    }
}

#[test]
fn saving_an_asset_unchanged_is_not_a_change() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let receipt = record(&conn, vec![laptop_and_sleeve(&books)]);
    let asset_id = receipt.documents[0].assets[0].account_id.unwrap();
    save_laptop(&conn, &books, asset_id, |_| {});

    let undone = undo(&conn, receipt.batch_id.unwrap(), false).unwrap();

    assert_eq!(undone.removed_asset_account_ids, [asset_id]);
}

#[test]
fn undo_stops_at_payment_links_added_or_removed_in_saldo() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let paid = payment(&conn, &books, -2_399);
    let fee = payment(&conn, &books, -100);
    let receipt = record(
        &conn,
        vec![
            with(
                single_expense(&books, "35697270", "FV1", 2_399),
                "paymentIds",
                json!([paid]),
            ),
            split_expense(&books),
        ],
    );
    let batch_id = receipt.batch_id.unwrap();
    let entry = single_event_id(&receipt.documents[0]);
    link_cashflows_to_taxable(
        &conn,
        LinkCashflowsParams {
            taxable_event_id: entry,
            cashflow_event_ids: vec![fee],
        },
    )
    .unwrap();
    unlink_cashflow_from_taxable(&conn, entry, paid).unwrap();

    let Err(UndoBatchError::Changed { rows, .. }) = undo(&conn, batch_id, false) else {
        panic!("the undo must stop")
    };

    assert_eq!(
        rows,
        [
            ChangedRow::PaymentLinkAddedInSaldo {
                event_id: entry,
                payment_id: fee
            },
            ChangedRow::PaymentLinkRemovedInSaldo {
                event_id: entry,
                payment_id: paid
            },
        ]
    );
    undo(&conn, batch_id, true).unwrap();
    assert!(documents(&conn, &books).is_empty());
}

#[test]
fn affected_tax_models_follow_prepaid_shares_and_depreciation_into_later_years() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let next_year = tax_model(&conn, books.person_id, 2027);
    tax_model(&conn, books.person_id, 2031);
    let documents: [fn(&Books) -> Value; 2] = [prepaid_expense, laptop_and_sleeve];

    for document in documents {
        let reviewed = run_batch(&conn, batch(vec![document(&books)], true)).unwrap();
        let receipt = record(&conn, vec![document(&books)]);
        let undone = undo(&conn, receipt.batch_id.unwrap(), false).unwrap();

        let shares_into_2027 = [books.tax_model_id, next_year];
        assert_eq!(reviewed.affected_tax_model_ids, shares_into_2027);
        assert_eq!(receipt.affected_tax_model_ids, shares_into_2027);
        assert_eq!(undone.affected_tax_model_ids, shares_into_2027);
    }
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

fn parse_batch(input: Value) -> Result<RecordBatchInput, String> {
    serde_path_to_error::deserialize::<_, RecordBatchInput>(input)
        .map_err(|e| format!("{}: {}", e.path(), e.inner()))
}

fn parse(items: Value) -> Result<RecordBatchInput, String> {
    parse_batch(json!({ "label": "x", "dryRun": true, "items": [items] }))
}

#[test]
fn a_batch_must_say_whether_it_is_a_dry_run() {
    let err = parse_batch(json!({ "label": "x", "items": [] })).unwrap_err();

    assert!(err.contains("dryRun"), "{err}");
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
        err.starts_with("items[0]: content: lines[0].vatReclaimablePctBps: 12000 "),
        "{err}"
    );
}

#[test]
fn an_unknown_kind_is_refused() {
    let err = parse(item_with(json!({ "kind": "invoice", "lines": [] }))).unwrap_err();

    assert!(err.starts_with("items[0]: content: "), "{err}");
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

const BILLA: &str = "O-7A3F0C1E9B2D4C58A6E1F0D3B5C7A9E2";
const DATART_OFFLINE: &str =
    "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:261020154211:118:2290.00";

fn billa() -> ReceiptReading {
    FakeEkasaApi {
        receipt_id: BILLA,
        lines: vec![
            ("KÁVA LAVAZZA 1KG", 1599, 2300),
            ("MLIEKO 1L", 129, 500),
            ("PAPIER A4", 662, 2300),
        ],
    }
    .read()
}

fn by(who: &str, deductible: i64, vat_reclaimable: i64) -> Value {
    json!({ "deductiblePctBps": deductible, "vatReclaimablePctBps": vat_reclaimable, "by": who })
}

fn receipt_item(books: &Books, reading: &ReceiptReading, shares: &[Value]) -> Value {
    let lines: Vec<Value> = reading
        .lines
        .as_ref()
        .expect("an online reading has lines")
        .iter()
        .zip(shares)
        .map(|(line, shares)| {
            let mut sent = json!({ "name": line.name, "grossMinor": line.gross_minor,
                                   "vatRateBps": line.vat_rate_bps });
            if !shares.is_null() {
                sent["shares"] = shares.clone();
            }
            sent
        })
        .collect();
    json!({
        "receipt": reading.receipt,
        "personId": books.person_id,
        "date": reading.date,
        "vendor": reading.vendor,
        "lines": lines,
    })
}

fn ekasa_profile(conn: &Connection, books: &Books, rules: &[(&str, i64, i64)]) {
    upsert_ekasa_profile(
        conn,
        UpsertEkasaProfileParams {
            person_id: books.person_id,
            default_deductible_pct_bps: 10000,
            default_vat_reclaimable_pct_bps: 10000,
            rules: rules
                .iter()
                .enumerate()
                .map(
                    |(index, &(pattern, deductible, vat_reclaimable))| EkasaRuleInput {
                        sort_order: index as i64,
                        name_pattern: pattern.to_owned(),
                        deductible_pct_bps: deductible,
                        vat_reclaimable_pct_bps: vat_reclaimable,
                    },
                )
                .collect(),
        },
    )
    .unwrap();
}

fn legs_with_ref(conn: &Connection, source_ref: &str) -> Vec<(Option<String>, i64, i64)> {
    let mut statement = conn
        .prepare(
            "SELECT ed.note, ed.expense_deductible_pct_bps, ed.vat_reclaimable_pct_bps
             FROM event e JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.source_ref = ?1 AND e.deleted_at IS NULL ORDER BY e.id",
        )
        .unwrap();
    statement
        .query_map([source_ref], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn group_note_of(conn: &Connection, source_ref: &str) -> Option<String> {
    conn.query_row(
        "SELECT sg.note FROM event e JOIN split_group sg ON sg.id = e.split_group_id
         WHERE e.source_ref = ?1 LIMIT 1",
        [source_ref],
        |row| row.get(0),
    )
    .unwrap()
}

#[test]
fn a_receipt_records_a_leg_per_line_with_the_shares_that_win() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    ekasa_profile(
        &conn,
        &books,
        &[("kava", 10000, 10000), ("mlieko|rozok", 0, 0)],
    );
    let reading = billa();
    let item = || {
        receipt_item(
            &books,
            &reading,
            &[
                by("claude", 0, 0),
                by("user", 10000, 5000),
                by("claude", 5000, 0),
            ],
        )
    };

    let reviewed = run_batch(&conn, batch(vec![item()], true)).unwrap();
    let written = record(&conn, vec![item()]);

    let document = serde_json::to_value(&reviewed.documents[0]).unwrap();
    assert_eq!(document["sourceRef"], format!("online:{BILLA}"));
    assert_eq!(document["dateSource"], "receipt_date");
    assert_eq!(document["entryDate"], "2026-10-12");
    assert_eq!(document["groupNote"], "BILLA s.r.o.");
    assert_eq!(document["shape"]["kind"], "split_group");
    assert_eq!(document["totalGrossMinor"], 2390);
    assert_eq!(
        document["lines"][0],
        json!({ "grossMinor": 1599, "netMinor": 1300, "vatRateBps": 2300,
                "note": "KÁVA LAVAZZA 1KG",
                "shares": { "deductiblePctBps": 10000, "vatReclaimablePctBps": 10000,
                            "from": "rule",
                            "rule": { "position": 1, "pattern": "kava",
                                      "deductiblePctBps": 10000, "vatReclaimablePctBps": 10000 },
                            "claude": { "deductiblePctBps": 0, "vatReclaimablePctBps": 0 } } })
    );
    assert_eq!(
        document["lines"][1]["shares"],
        json!({ "deductiblePctBps": 10000, "vatReclaimablePctBps": 5000, "from": "user",
                "rule": { "position": 2, "pattern": "mlieko|rozok",
                          "deductiblePctBps": 0, "vatReclaimablePctBps": 0 } })
    );
    assert_eq!(
        document["lines"][2]["shares"],
        json!({ "deductiblePctBps": 5000, "vatReclaimablePctBps": 0, "from": "claude" })
    );

    let source_ref = format!("online:{BILLA}");
    assert_eq!(
        legs_with_ref(&conn, &source_ref),
        [
            (Some("KÁVA LAVAZZA 1KG".to_owned()), 10000, 10000),
            (Some("MLIEKO 1L".to_owned()), 10000, 5000),
            (Some("PAPIER A4".to_owned()), 5000, 0),
        ]
    );
    assert_eq!(
        group_note_of(&conn, &source_ref).as_deref(),
        Some("BILLA s.r.o.")
    );
    assert!(written.documents[0].warnings.is_empty());
}

#[test]
fn a_one_line_receipt_is_a_single_entry_named_after_its_line() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let reading = FakeEkasaApi {
        receipt_id: BILLA,
        lines: vec![("PAPIER A4", 662, 2300)],
    }
    .read();

    let receipt = record(
        &conn,
        vec![receipt_item(
            &books,
            &reading,
            &[by("claude", 10000, 10000)],
        )],
    );

    assert!(matches!(
        receipt.documents[0].shape,
        RecordedShape::Single { event_id: Some(_) }
    ));
    assert_eq!(receipt.documents[0].group_note, None);
    assert_eq!(
        legs_with_ref(&conn, &format!("online:{BILLA}")),
        [(Some("PAPIER A4".to_owned()), 10000, 10000)]
    );
}

#[test]
fn a_line_without_shares_needs_a_rule_and_the_profile_default_does_not_count() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    ekasa_profile(&conn, &books, &[("kava", 10000, 10000)]);
    let reading = billa();

    let (errors, _) = refused(run_batch(
        &conn,
        batch(
            vec![receipt_item(
                &books,
                &reading,
                &[Value::Null, Value::Null, Value::Null],
            )],
            true,
        ),
    ));

    assert_eq!(errors[0].code, "SHARES_NEEDED");
    let message = &errors[0].message;
    assert!(
        message.contains("lines[1] 'MLIEKO 1L'") && message.contains("lines[2] 'PAPIER A4'"),
        "{message}"
    );
    assert!(!message.contains("lines[0]"), "{message}");
}

#[test]
fn a_receipt_that_differs_from_its_reading_is_refused() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let mut item = receipt_item(
        &books,
        &billa(),
        &[by("claude", 0, 0), by("claude", 0, 0), by("claude", 0, 0)],
    );
    item["lines"][2]["grossMinor"] = json!(626);

    let (errors, _) = refused(run_batch(&conn, batch(vec![item], true)));

    assert_eq!(errors[0].code, "RECEIPT_CHANGED");
    assert_eq!(errors[0].file, "/r/billa.jpg");
}

#[test]
fn a_receipt_the_app_imported_is_already_recorded_and_twice_in_a_batch_is_a_duplicate() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let item = || {
        receipt_item(
            &books,
            &billa(),
            &[by("claude", 0, 0), by("claude", 0, 0), by("claude", 0, 0)],
        )
    };
    let (twice, _) = refused(run_batch(&conn, batch(vec![item(), item()], true)));
    let imported = create_taxable_event_inner(
        &conn,
        CreateTaxableEventInput {
            person_id: books.person_id,
            event_type: "expense".to_owned(),
            amount_minor: 2390,
            event_date: "2026-10-12".to_owned(),
            note: Some("BILLA".to_owned()),
            vat_rate_bps: Some(2300),
            vat_reclaimable_pct_bps: Some(10000),
            expense_deductible_pct_bps: Some(10000),
            prepaid_until: None,
            reclaimed_vat: Some(true),
            source_ref: Some(SourceRef::ekasa_online(BILLA)),
        },
    )
    .unwrap();

    let (recorded, _) = refused(run_batch(&conn, batch(vec![item()], true)));

    assert_eq!(twice[0].index, 1);
    assert_eq!(twice[0].code, "DUPLICATE_IN_BATCH");
    assert_eq!(recorded[0].code, "ALREADY_RECORDED");
    assert!(matches!(
        recorded[0].recorded,
        Some(RecordedSource::Entry { event_id, .. }) if event_id == imported
    ));
}

#[test]
fn an_invoice_and_a_receipt_record_together_and_undo_together() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let paid = payment(&conn, &books, -2_390);
    let receipt = with(
        receipt_item(
            &books,
            &billa(),
            &[
                by("claude", 10000, 10000),
                by("claude", 0, 0),
                by("claude", 10000, 10000),
            ],
        ),
        "paymentIds",
        json!([paid]),
    );

    let recorded = record(
        &conn,
        vec![single_expense(&books, "35697270", "FV1", 2_399), receipt],
    );
    let found = documents(&conn, &books);
    let undone = undo(&conn, recorded.batch_id.unwrap(), false).unwrap();

    assert_eq!(found.len(), 2);
    let receipt = document(&found, &format!("online:{BILLA}"));
    assert_eq!(receipt.linked_payment_ids, [paid]);
    assert!(receipt.split_group_id.is_some());
    assert!(found.iter().all(|doc| doc.batch_id == recorded.batch_id));
    assert_eq!(
        undone.removed_event_ids.len(),
        4,
        "the invoice and the receipt's three legs"
    );
    assert!(documents(&conn, &books).is_empty());
    assert_eq!(unlinked_payments(&conn, &books), [paid]);
}

#[test]
fn an_offline_receipt_with_equipment_records_an_asset_without_vat() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let reading = read_offline(DATART_OFFLINE);
    let item = json!({
        "receipt": reading.receipt,
        "personId": books.person_id,
        "date": reading.date,
        "vendor": "DATART",
        "lines": [
            { "name": "Notebook Lenovo ThinkPad T14", "grossMinor": 219_900, "vatRateBps": 2300,
              "asset": { "depreciationMonths": 48 } },
            { "name": "Mys Logitech M185", "grossMinor": 2_900, "vatRateBps": 2300,
              "shares": by("claude", 10000, 10000) },
            { "name": "Taska na notebook", "grossMinor": 6_200, "vatRateBps": 2300,
              "shares": by("claude", 10000, 10000) },
        ],
    });
    let mut short = item.clone();
    short["lines"][2]["grossMinor"] = json!(6_000);

    let (errors, _) = refused(run_batch(&conn, batch(vec![short], true)));
    let receipt = record(&conn, vec![item]);

    assert_eq!(errors[0].code, "RECEIPT_TOTAL_DIFFERS");
    let document = &receipt.documents[0];
    assert_eq!(document.entry_date, "2026-10-20");
    assert_eq!(document.total_gross_minor, 229_000);
    assert_eq!(document.lines.len(), 2);
    assert_eq!(document.assets[0].name, "Notebook Lenovo ThinkPad T14");
    assert_eq!(document.assets[0].purchase_price_minor, 178_780);
    assert_eq!(document.assets[0].price_basis, PriceBasis::NetVatReclaimed);
    assert_eq!(document.group_note.as_deref(), Some("DATART"));
}

#[test]
fn an_asset_line_takes_no_shares() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    let reading = read_offline(DATART_OFFLINE);
    let item = json!({
        "receipt": reading.receipt, "personId": books.person_id, "date": reading.date,
        "vendor": "DATART",
        "lines": [{ "name": "Notebook", "grossMinor": 229_000, "vatRateBps": 2300,
                    "asset": { "depreciationMonths": 48 }, "shares": by("user", 10000, 10000) }],
    });

    let (errors, _) = refused(run_batch(&conn, batch(vec![item], true)));

    assert_eq!(errors[0].code, "VALIDATION");
}

#[test]
fn a_write_after_the_rules_or_the_vat_payer_flag_changed_is_refused() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    ekasa_profile(&conn, &books, &[("kava", 10000, 10000)]);
    let items = || {
        vec![receipt_item(
            &books,
            &billa(),
            &[by("claude", 0, 0), by("claude", 0, 0), by("claude", 0, 0)],
        )]
    };
    let reviewed = run_batch(&conn, batch(items(), true)).unwrap();
    ekasa_profile(&conn, &books, &[("kava", 5000, 10000)]);
    let after_rule_edit = submit(&conn, reviewed_write(items(), reviewed.review_token));
    let reviewed = run_batch(&conn, batch(items(), true)).unwrap();
    update_person(
        &conn,
        UpdatePersonParams {
            person_id: books.person_id,
            name: "Firma s.r.o.".to_owned(),
            person_type: "legal".to_owned(),
            vat_payer: false,
        },
    )
    .unwrap();
    let after_flag_edit = submit(&conn, reviewed_write(items(), reviewed.review_token));

    assert!(matches!(
        after_rule_edit,
        Err(RecordBatchError::ReviewTokenMismatch)
    ));
    assert!(matches!(
        after_flag_edit,
        Err(RecordBatchError::ReviewTokenMismatch)
    ));
    assert!(documents(&conn, &books).is_empty());
    assert!(list_recent_batches(&conn, 10).unwrap().is_empty());
}

#[test]
fn a_stored_rule_that_does_not_compile_is_a_warning() {
    let conn = initialize_in_memory().unwrap();
    let books = books(&conn);
    ekasa_profile(&conn, &books, &[("ka(va", 0, 0), ("kava", 10000, 10000)]);

    let receipt = run_batch(
        &conn,
        batch(
            vec![receipt_item(
                &books,
                &billa(),
                &[Value::Null, by("claude", 0, 0), by("claude", 0, 0)],
            )],
            true,
        ),
    )
    .unwrap();

    let document = &receipt.documents[0];
    assert!(matches!(
        &document.warnings[..],
        [Warning::InvalidEkasaRule { position: 1, pattern, .. }] if pattern == "ka(va"
    ));
    assert_eq!(
        document.lines[0]
            .shares
            .as_ref()
            .unwrap()
            .deductible_pct_bps,
        10000
    );
}

#[test]
fn an_item_needs_a_document_or_a_receipt() {
    let err = parse(json!({ "personId": 1, "issueDate": "2026-10-12" })).unwrap_err();

    assert!(
        err.contains("`document`") && err.contains("`receipt`"),
        "{err}"
    );
}

#[test]
fn an_error_inside_a_receipt_line_keeps_its_path() {
    let item = json!({
        "receipt": { "file": "a.jpg", "code": BILLA, "readToken": "0" },
        "personId": 1, "date": "2026-10-12", "vendor": "BILLA",
        "lines": [{ "name": "KAVA", "grossMinor": 100, "vatRateBps": 2300,
                    "shares": { "deductiblePctBps": 0, "vatReclaimablePctBps": 0,
                                "by": "profile" } }],
    });

    let err = parse(item).unwrap_err();

    assert!(err.starts_with("items[0]: lines[0].shares.by: "), "{err}");
}
