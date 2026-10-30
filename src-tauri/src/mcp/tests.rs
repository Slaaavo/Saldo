use super::*;
use crate::config::DB_FILENAME;
use crate::features::accounts::repository::{create_account, CreateAccountParams};
use crate::features::ekasa::reading::fake::FakeEkasaApi;
use crate::features::persons::repository::get_default_expense_account_id;
use crate::features::tax_models::models::ActualMonths;
use crate::features::tax_models::repository::{
    create_tax_model, BracketInput, CreateTaxModelParams,
};
use crate::features::transactions::repository::{
    create_cashflow, create_taxable_event, link_cashflows_to_taxable, CashflowEntry,
    CreateTaxableEventParams, LinkCashflowsParams,
};
use rusqlite::Connection;

struct Seeded {
    dir: tempfile::TempDir,
    person_id: i64,
    tax_model_id: i64,
    entry_id: i64,
    payment_id: i64,
}

fn seeded() -> Seeded {
    let dir = tempfile::tempdir().unwrap();
    let conn = crate::db::initialize_db(&dir.path().join(DB_FILENAME)).unwrap();
    let person_id: i64 = conn
        .query_row("SELECT id FROM person WHERE is_default = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    let entry_id = create_taxable_event(
        &conn,
        CreateTaxableEventParams {
            account_id: get_default_expense_account_id(&conn, person_id).unwrap(),
            event_type: "expense".to_owned(),
            amount_minor: 2_399,
            event_date: "2026-09-30".to_owned(),
            vat_rate_bps: Some(2300),
            expense_deductible_pct_bps: Some(8000),
            vat_reclaimable_pct_bps: Some(8000),
            source_ref: Some(SourceRef::invoice("35697270", "FV2026091044").unwrap()),
            ..Default::default()
        },
    )
    .unwrap();
    let payment_id = bank_payment(&conn, person_id, -2_399);
    link_cashflows_to_taxable(
        &conn,
        LinkCashflowsParams {
            taxable_event_id: entry_id,
            cashflow_event_ids: vec![payment_id],
        },
    )
    .unwrap();
    bank_payment(&conn, person_id, -4_512);
    let tax_model_id = create_tax_model(
        &conn,
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
    Seeded {
        dir,
        person_id,
        tax_model_id,
        entry_id,
        payment_id,
    }
}

fn bank_payment(conn: &Connection, person_id: i64, amount_minor: i64) -> i64 {
    let account_id = create_account(
        conn,
        CreateAccountParams {
            name: format!("Bank {amount_minor}"),
            currency_id: 1,
            account_type: "account".to_owned(),
            person_id: Some(person_id),
            ..Default::default()
        },
    )
    .unwrap();
    create_cashflow(
        conn,
        &CashflowEntry {
            account_id,
            amount_minor,
            event_date: "2026-10-02".to_owned(),
            note: Some("ORANGE SLOVENSKO".to_owned()),
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

const BILLA: &str = "O-7A3F0C1E9B2D4C58A6E1F0D3B5C7A9E2";

fn outside() -> Outside {
    Outside {
        resource_dir: None,
        ekasa: Box::new(FakeEkasaApi {
            receipt_id: BILLA,
            lines: vec![("KÁVA LAVAZZA 1KG", 1599, 2300), ("MLIEKO 1L", 129, 500)],
        }),
    }
}

fn call(dir: &tempfile::TempDir, name: &str, arguments: Value) -> (bool, Value) {
    match Server::new(dir.path().to_path_buf(), outside()).call(name, arguments) {
        ToolOutcome::Done(value) => (true, value),
        ToolOutcome::Refused(value) => (false, value),
        ToolOutcome::UnknownTool => panic!("unknown tool {name}"),
    }
}

fn sample_arguments(name: &str, seeded: &Seeded) -> Value {
    match name {
        "saldo_context" => json!({}),
        "find_entries" => json!({ "issuer": "35697270", "kinds": ["expense"] }),
        "find_payments" => json!({ "personId": seeded.person_id, "unlinkedOnly": true }),
        "get_forecast" => json!({ "taxModelId": seeded.tax_model_id }),
        "read_receipt" => json!({ "file": "/r/billa.jpg", "code": BILLA }),
        other => panic!("give the read tool {other} sample arguments here"),
    }
}

#[test]
fn every_read_tool_leaves_the_database_unchanged() {
    let seeded = seeded();
    for tool in tools().iter().filter(|tool| tool.access == Access::Read) {
        let db = open_saldo_db(seeded.dir.path()).unwrap();

        let result = (tool.run)(&outside(), &db, sample_arguments(tool.name, &seeded));

        assert!(result.is_ok(), "{} failed", tool.name);
        let total_changes: i64 = db
            .conn
            .query_row("SELECT total_changes()", [], |row| row.get(0))
            .unwrap();
        assert_eq!(total_changes, 0, "{} wrote to the database", tool.name);
    }
}

#[test]
fn tools_list_carries_each_schema_and_whether_the_tool_only_reads() {
    let listed = Server::new(PathBuf::new(), outside()).list();

    let read_only: Vec<(&str, bool)> = listed
        .iter()
        .map(|t| {
            (
                t["name"].as_str().unwrap(),
                t["annotations"]["readOnlyHint"] == true,
            )
        })
        .collect();
    assert_eq!(
        read_only,
        [
            ("saldo_context", true),
            ("find_entries", true),
            ("find_payments", true),
            ("get_forecast", true),
            ("read_receipt", true),
            ("record_batch", false),
            ("undo_batch", false),
        ]
    );
    for tool in &listed {
        assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
        assert!(tool["description"].as_str().is_some_and(|d| !d.is_empty()));
    }
    let find_entries = &listed[1]["inputSchema"];
    assert!(find_entries["properties"]["issuer"].is_object());
    assert_eq!(find_entries["additionalProperties"], false);
    assert_eq!(listed[3]["inputSchema"]["required"], json!(["taxModelId"]));
    assert_eq!(listed[4]["inputSchema"]["required"], json!(["file"]));
    assert!(listed[4]["inputSchema"]["properties"]["code"].is_object());
    let record_batch = listed[5]["inputSchema"].to_string();
    let required = listed[5]["inputSchema"]["required"].as_array().unwrap();
    assert!(required.contains(&json!("dryRun")), "{required:?}");
    for field in [
        "reviewToken",
        "deliveryDate",
        "deductiblePctBps",
        "prepaidUntil",
        "depreciationMonths",
        "readToken",
        "vendor",
        "asset",
        "\"by\"",
    ] {
        assert!(
            record_batch.contains(field),
            "record_batch's schema lacks {field}"
        );
    }
    assert!(
        !record_batch.contains("$ref"),
        "subschemas are inlined for clients that do not follow references"
    );
}

#[test]
fn the_limit_text_in_the_schema_names_the_page_size_constants() {
    let listed = Server::new(PathBuf::new(), outside()).list();
    let named = format!("Default {DEFAULT_PAGE_SIZE}, at most {MAX_PAGE_SIZE}");

    for tool in [&listed[1], &listed[2]] {
        let limit = tool["inputSchema"]["properties"]["limit"].to_string();
        assert!(limit.contains(&named), "{}: {limit}", tool["name"]);
    }
}

/// A dry run of one Orange invoice.
fn orange_invoice(seeded: &Seeded, number: &str) -> Value {
    json!({
        "label": "Invoices 2026-10",
        "dryRun": true,
        "items": [{
            "document": { "issuer": "35697270", "number": number, "file": "orange.pdf" },
            "personId": seeded.person_id,
            "issueDate": "2026-10-31",
            "content": { "kind": "expense", "lines": [{
                "grossMinor": 2399, "vatRateBps": 2300,
                "deductiblePctBps": 8000, "vatReclaimablePctBps": 8000 }] },
        }],
    })
}

/// The same batch as a real write, carrying the token its dry run returned.
fn approved(mut dry_run: Value, receipt: &Value) -> Value {
    dry_run["dryRun"] = json!(false);
    dry_run["reviewToken"] = receipt["reviewToken"].clone();
    dry_run
}

#[test]
fn a_recorded_batch_shows_in_the_context_and_undoes() {
    let seeded = seeded();
    let batch = orange_invoice(&seeded, "FV2");
    let (_, reviewed) = call(&seeded.dir, "record_batch", batch.clone());

    let (ok, receipt) = call(&seeded.dir, "record_batch", approved(batch, &reviewed));
    let (_, context) = call(&seeded.dir, "saldo_context", json!({}));
    let batch_id = receipt["batchId"].clone();
    let (undone, undo) = call(&seeded.dir, "undo_batch", json!({ "batchId": batch_id }));

    assert!(ok, "{receipt}");
    assert_eq!(receipt["documents"][0]["shape"]["kind"], "single");
    assert_eq!(receipt["affectedTaxModelIds"], json!([seeded.tax_model_id]));
    assert_eq!(context["recentBatches"][0]["id"], batch_id);
    assert!(undone, "{undo}");
    assert_eq!(undo["alreadyUndone"], false);
}

fn entries_with_ref(conn: &Connection, source_ref: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM event WHERE source_ref = ?1",
        [source_ref],
        |row| row.get(0),
    )
    .unwrap()
}

#[test]
fn a_write_without_the_reviewed_token_is_refused_and_writes_nothing() {
    let seeded = seeded();
    let batch = orange_invoice(&seeded, "FV2");
    let (_, reviewed) = call(&seeded.dir, "record_batch", batch.clone());
    let mut changed = approved(batch, &reviewed);
    changed["items"][0]["content"]["lines"][0]["grossMinor"] = json!(2400);

    let (ok, refusal) = call(&seeded.dir, "record_batch", changed);
    let (_, found) = call(
        &seeded.dir,
        "find_entries",
        json!({ "issuer": "35697270", "number": "FV2" }),
    );

    assert!(!ok);
    assert_eq!(refusal["code"], "REVIEW_TOKEN_MISMATCH");
    assert_eq!(found["entries"], json!([]));
}

#[test]
fn a_write_after_saldo_switched_its_database_is_refused_and_writes_nothing() {
    let seeded = seeded();
    let db = open_saldo_db(seeded.dir.path()).unwrap();
    let record = tools()
        .into_iter()
        .find(|tool| tool.name == "record_batch")
        .unwrap();
    let batch = orange_invoice(&seeded, "FV2");
    let reviewed = (record.run)(&outside(), &db, batch.clone()).ok().unwrap();
    let folder = seeded.dir.path().join("cloud");
    std::fs::create_dir_all(&folder).unwrap();
    let switched_to = crate::db::initialize_db(&folder.join(DB_FILENAME)).unwrap();
    crate::config::write_db_location(seeded.dir.path(), &folder).unwrap();

    let refusal = (record.run)(&outside(), &db, approved(batch, &reviewed))
        .unwrap_err()
        .into_json();

    assert_eq!(refusal["code"], "DATABASE_CHANGED");
    assert!(db.conn.is_autocommit());
    assert_eq!(entries_with_ref(&db.conn, "invoice:35697270/FV2"), 0);
    assert_eq!(entries_with_ref(&switched_to, "invoice:35697270/FV2"), 0);
}

#[test]
fn a_refused_batch_is_an_error_result_listing_each_document() {
    let seeded = seeded();

    let (ok, refusal) = call(
        &seeded.dir,
        "record_batch",
        orange_invoice(&seeded, "FV2026091044"),
    );

    assert!(!ok);
    assert_eq!(refusal["code"], "BATCH_REFUSED");
    assert_eq!(refusal["errors"][0]["code"], "ALREADY_RECORDED");
    assert_eq!(refusal["errors"][0]["file"], "orange.pdf");
    assert_eq!(refusal["errors"][0]["recorded"]["eventId"], seeded.entry_id);
}

#[test]
fn a_call_without_a_database_is_a_refusal_the_model_can_relay() {
    let empty = tempfile::tempdir().unwrap();

    let (ok, result) = call(&empty, "saldo_context", json!({}));

    assert!(!ok);
    assert_eq!(result["code"], "NO_DATABASE");
    assert!(result["message"]
        .as_str()
        .is_some_and(|m| m.contains("Open Saldo")));
}

#[test]
fn arguments_that_do_not_parse_are_named_by_their_path() {
    let seeded = seeded();

    let (ok, bad_kind) = call(
        &seeded.dir,
        "find_entries",
        json!({ "kinds": ["expense", "invoice"] }),
    );
    let (_, unknown_field) = call(&seeded.dir, "find_entries", json!({ "isuer": "1" }));

    assert!(!ok);
    assert_eq!(bad_kind["code"], "INVALID_ARGUMENTS");
    assert_eq!(bad_kind["path"], "kinds[1]");
    assert_eq!(unknown_field["code"], "INVALID_ARGUMENTS");
}

#[test]
fn saldo_context_lists_persons_accounts_and_tax_models() {
    let seeded = seeded();

    let (ok, context) = call(&seeded.dir, "saldo_context", json!({}));

    assert!(ok, "{context}");
    assert_eq!(context["consolidationCurrency"]["code"], "EUR");
    let person = &context["persons"][0];
    assert_eq!(person["id"], seeded.person_id);
    assert_eq!(person["expenseAccount"]["currency"], "EUR");
    assert_eq!(context["taxModels"][0]["id"], seeded.tax_model_id);
    assert_eq!(context["taxModels"][0]["year"], 2026);
    assert!(context["databasePath"]
        .as_str()
        .is_some_and(|p| p.ends_with(DB_FILENAME)));
}

#[test]
fn find_entries_finds_a_recorded_invoice_by_issuer_and_number() {
    let seeded = seeded();

    let (ok, found) = call(
        &seeded.dir,
        "find_entries",
        json!({ "issuer": "35697270", "number": "fv 2026091044" }),
    );
    let (_, other) = call(
        &seeded.dir,
        "find_entries",
        json!({ "issuer": "35697270", "number": "FV2026091045" }),
    );

    assert!(ok, "{found}");
    let entry = &found["entries"][0];
    assert_eq!(entry["sourceRef"], "invoice:35697270/FV2026091044");
    assert_eq!(entry["lines"][0]["eventId"], seeded.entry_id);
    assert_eq!(entry["linkedPaymentIds"], json!([seeded.payment_id]));
    assert_eq!(other["entries"], json!([]));
}

#[test]
fn find_entries_refuses_a_number_without_its_issuer() {
    let seeded = seeded();

    let (ok, result) = call(&seeded.dir, "find_entries", json!({ "number": "FV1" }));

    assert!(!ok);
    assert_eq!(result["code"], "VALIDATION");
}

#[test]
fn find_payments_finds_the_unlinked_payment() {
    let seeded = seeded();

    let (ok, found) = call(
        &seeded.dir,
        "find_payments",
        json!({ "personId": seeded.person_id, "unlinkedOnly": true }),
    );

    assert!(ok, "{found}");
    let payments = found["payments"].as_array().unwrap();
    assert_eq!(payments.len(), 1);
    assert_eq!(payments[0]["amountMinor"], -4_512);
    assert_eq!(payments[0]["linkedEntryIds"], json!([]));
}

#[test]
fn get_forecast_counts_the_recorded_expense() {
    let seeded = seeded();

    let (ok, forecast) = call(
        &seeded.dir,
        "get_forecast",
        json!({ "taxModelId": seeded.tax_model_id }),
    );

    assert!(ok, "{forecast}");
    let counted = forecast["eventBreakdowns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["eventId"] == seeded.entry_id);
    assert!(counted, "{forecast}");
}

/// The batch Claude builds from read_receipt's answer: the receipt, its date and vendor, and
/// its lines as read, each with Claude's judgment of its shares.
fn receipt_batch(seeded: &Seeded, read: &Value) -> Value {
    let lines: Vec<Value> = read["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| {
            let mut line = line.clone();
            line["shares"] = json!({ "deductiblePctBps": 10000,
                                     "vatReclaimablePctBps": 10000, "by": "claude" });
            line
        })
        .collect();
    json!({
        "label": "Receipts 2026-10",
        "dryRun": true,
        "items": [{
            "receipt": read["receipt"],
            "personId": seeded.person_id,
            "date": read["date"],
            "vendor": read["vendor"],
            "lines": lines,
        }],
    })
}

#[test]
fn a_receipt_read_then_recorded_is_already_recorded_when_read_again() {
    let seeded = seeded();
    let receipt = json!({ "file": "/Users/me/Receipts/billa.jpg", "code": BILLA });

    let (ok, read) = call(&seeded.dir, "read_receipt", receipt.clone());
    let batch = receipt_batch(&seeded, &read);
    let (_, reviewed) = call(&seeded.dir, "record_batch", batch.clone());
    let (written, recorded) = call(&seeded.dir, "record_batch", approved(batch, &reviewed));
    let (_, read_again) = call(&seeded.dir, "read_receipt", receipt);

    assert!(ok, "{read}");
    assert_eq!(read["kind"], "online");
    assert_eq!(read["sourceRef"], format!("online:{BILLA}"));
    assert_eq!(read["receipt"]["file"], "/Users/me/Receipts/billa.jpg");
    assert_eq!(read["receipt"]["code"], BILLA);
    assert_eq!(read["date"], "2026-10-12");
    assert_eq!(read["vendor"], "BILLA s.r.o.");
    assert_eq!(read["totalMinor"], 1728);
    assert_eq!(
        read["lines"],
        json!([{ "name": "KÁVA LAVAZZA 1KG", "grossMinor": 1599, "vatRateBps": 2300 },
               { "name": "MLIEKO 1L", "grossMinor": 129, "vatRateBps": 500 }])
    );
    assert_eq!(read["warnings"], json!([]));
    assert_eq!(read["alreadyRecorded"], Value::Null);
    assert_eq!(
        reviewed["documents"][0]["dateSource"], "receipt_date",
        "{reviewed}"
    );
    assert_eq!(
        reviewed["documents"][0]["lines"][1]["shares"]["from"],
        "claude"
    );
    assert!(written, "{recorded}");
    assert_eq!(read_again["alreadyRecorded"]["kind"], "entry");
    assert_eq!(
        read_again["alreadyRecorded"]["batchId"],
        recorded["batchId"]
    );
}

#[test]
fn read_receipt_reads_the_qr_code_of_an_offline_receipt_file() {
    let seeded = seeded();
    let file = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ekasa/offline-receipt.png"
    );

    let (ok, read) = call(&seeded.dir, "read_receipt", json!({ "file": file }));

    assert!(ok, "{read}");
    assert_eq!(read["kind"], "offline");
    assert_eq!(read["date"], "2018-02-13");
    assert_eq!(read["totalMinor"], 23723);
    assert_eq!(read["lines"], Value::Null);
    assert_eq!(read["vendor"], Value::Null);
}

#[test]
fn read_receipt_refuses_a_relative_path() {
    let seeded = seeded();

    let (ok, refusal) = call(
        &seeded.dir,
        "read_receipt",
        json!({ "file": "billa.jpg", "code": BILLA }),
    );

    assert!(!ok);
    assert_eq!(refusal["code"], "INVALID_ARGUMENTS");
    assert_eq!(refusal["path"], "file");
}

#[test]
fn read_receipt_says_when_the_ekasa_service_does_not_know_the_receipt() {
    let seeded = seeded();

    let (ok, refusal) = call(
        &seeded.dir,
        "read_receipt",
        json!({ "file": "/r/a.jpg", "code": "O-00000000000000000000000000000000" }),
    );

    assert!(!ok);
    assert_eq!(refusal["code"], "RECEIPT_NOT_FOUND");
}
