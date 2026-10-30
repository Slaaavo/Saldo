use schemars::generate::SchemaSettings;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

use super::open::{open_saldo_db, OpenRefusal, SaldoDb};
use super::protocol::{ToolHost, ToolOutcome};
use crate::error::AppError;
use crate::features::accounts::repository::get_account_currency_id;
use crate::features::currency::repository::{get_consolidation_currency, list_currencies};
use crate::features::entry_batches::models::{
    BatchReceipt, BatchSummary, RecordBatchError, RecordBatchInput, UndoBatchError, UndoBatchInput,
    UndoReceipt,
};
use crate::features::entry_batches::{self, list_recent_batches};
use crate::features::persons::repository::list_persons;
use crate::features::tax_models::commands::calculate_tax_model_inner;
use crate::features::tax_models::models::TaxCalculationResult;
use crate::features::tax_models::repository::list_tax_models;
use crate::features::transactions::models::{DocumentEntry, Payment, TaxableKind};
use crate::features::transactions::repository::{
    find_document_entries, find_payments, FindDocumentEntriesParams, FindPaymentsParams,
};
use crate::features::transactions::source_ref::invoice_issuer_prefix;
use crate::features::transactions::SourceRef;
use crate::shared::{validate_iso_date, with_read_txn, WriteTxn};

pub(super) struct Server {
    app_data_dir: PathBuf,
}

impl Server {
    pub(super) fn new(app_data_dir: PathBuf) -> Self {
        Self { app_data_dir }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Write,
}

type Run = Box<dyn Fn(&SaldoDb, Value) -> Result<Value, ToolFailure>>;

struct Tool {
    name: &'static str,
    description: &'static str,
    access: Access,
    input_schema: fn() -> Value,
    run: Run,
}

fn tools() -> Vec<Tool> {
    vec![
        read_tool(
            "saldo_context",
            "Persons with their VAT-payer flag and default revenue and expense accounts, the tax \
             models, the consolidation currency and the database path. Call this first.",
            saldo_context,
        ),
        read_tool(
            "find_entries",
            "Recorded revenue, expense and prepaid expense documents, newest first, with a split \
             group's lines grouped into one document. Use issuer and number to check whether a \
             document is already recorded, and issuer alone to reuse a supplier's earlier \
             deductible and VAT shares.",
            find_entries,
        ),
        read_tool(
            "find_payments",
            "Bank and card payments (cash flows), newest first. The note holds the bank text. A \
             foreign-currency payment carries its original amount and currency next to the \
             amount in the account's currency.",
            find_payments_tool,
        ),
        read_tool(
            "get_forecast",
            "The tax forecast of a tax model, as Saldo's tax model page shows it.",
            get_forecast,
        ),
        write_tool(
            "record_batch",
            "Record documents (invoices, credit notes) as revenue, expense and prepaid expense \
             entries and assets, all of them or none. Saldo dates each entry by the delivery \
             date (else the issue date), makes a split group of two or more lines, prices an \
             asset without VAT when the VAT is reclaimed, and links the given payments. A \
             document already in Saldo is refused. Call it with dryRun true first and show the \
             user the table. Only after the user approved it, call it again with the same \
             batch, dryRun false and the reviewToken the dry run returned. A batch that differs \
             from the reviewed one in any field is refused.",
            record_batch,
        ),
        write_tool(
            "undo_batch",
            "Remove everything one batch recorded: entries, their prepaid shares and split \
             legs, assets and their depreciation, payment links. Undoing twice changes nothing. \
             When rows were changed in Saldo since, it refuses and lists them.",
            undo_batch,
        ),
    ]
}

fn read_tool<I, O>(
    name: &'static str,
    description: &'static str,
    handler: fn(&SaldoDb, I) -> Result<O, ToolFailure>,
) -> Tool
where
    I: DeserializeOwned + JsonSchema + 'static,
    O: Serialize + 'static,
{
    Tool {
        name,
        description,
        access: Access::Read,
        input_schema: input_schema::<I>,
        run: Box::new(move |db, arguments| {
            let input = parse_arguments(arguments)?;
            output_value(with_read_txn(&db.conn, || handler(db, input))?)
        }),
    }
}

/// A write tool gets its transaction from `begin_write`, which re-checks the database under
/// the write lock.
fn write_tool<I, O>(
    name: &'static str,
    description: &'static str,
    handler: fn(WriteTxn<'_>, I) -> Result<O, ToolFailure>,
) -> Tool
where
    I: DeserializeOwned + JsonSchema + 'static,
    O: Serialize + 'static,
{
    Tool {
        name,
        description,
        access: Access::Write,
        input_schema: input_schema::<I>,
        run: Box::new(move |db, arguments| {
            let input = parse_arguments(arguments)?;
            let txn = db.begin_write().map_err(ToolFailure::Unavailable)?;
            output_value(handler(txn, input)?)
        }),
    }
}

fn parse_arguments<I: DeserializeOwned>(arguments: Value) -> Result<I, ToolFailure> {
    serde_path_to_error::deserialize(arguments).map_err(|e| ToolFailure::InvalidArguments {
        path: e.path().to_string(),
        message: e.inner().to_string(),
    })
}

fn output_value<O: Serialize>(output: O) -> Result<Value, ToolFailure> {
    serde_json::to_value(output).map_err(|e| ToolFailure::App(e.to_string().into()))
}

fn input_schema<I: JsonSchema>() -> Value {
    SchemaSettings::draft2020_12()
        .with(|settings| {
            settings.inline_subschemas = true;
            settings.meta_schema = None;
        })
        .into_generator()
        .into_root_schema_for::<I>()
        .to_value()
}

impl ToolHost for Server {
    fn list(&self) -> Vec<Value> {
        tools()
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": (tool.input_schema)(),
                    "annotations": { "readOnlyHint": tool.access == Access::Read },
                })
            })
            .collect()
    }

    fn call(&self, name: &str, arguments: Value) -> ToolOutcome {
        let tools = tools();
        let Some(tool) = tools.iter().find(|tool| tool.name == name) else {
            return ToolOutcome::UnknownTool;
        };
        let outcome = open_saldo_db(&self.app_data_dir)
            .map_err(ToolFailure::Unavailable)
            .and_then(|db| (tool.run)(&db, arguments));
        match outcome {
            Ok(value) => ToolOutcome::Done(value),
            Err(failure) => ToolOutcome::Refused(failure.into_json()),
        }
    }
}

enum ToolFailure {
    Unavailable(OpenRefusal),
    InvalidArguments { path: String, message: String },
    App(AppError),
    Batch(RecordBatchError),
    Undo(UndoBatchError),
}

impl From<AppError> for ToolFailure {
    fn from(e: AppError) -> Self {
        ToolFailure::App(e)
    }
}

impl From<rusqlite::Error> for ToolFailure {
    fn from(e: rusqlite::Error) -> Self {
        ToolFailure::App(e.into())
    }
}

impl ToolFailure {
    fn into_json(self) -> Value {
        match self {
            ToolFailure::Unavailable(refusal) => {
                json!({ "code": refusal.code(), "message": refusal.message() })
            }
            ToolFailure::InvalidArguments { path, message } => json!({
                "code": "INVALID_ARGUMENTS",
                "path": path,
                "message": format!("{path}: {message}"),
            }),
            ToolFailure::App(e)
            | ToolFailure::Batch(RecordBatchError::App(e))
            | ToolFailure::Undo(UndoBatchError::App(e)) => {
                json!({ "code": e.code, "message": e.message })
            }
            ToolFailure::Batch(RecordBatchError::Items {
                dry_run,
                errors,
                passed,
            }) => json!({
                "code": "BATCH_REFUSED",
                "message": format!(
                    "{} of {} documents failed. Nothing was recorded.",
                    errors.len(),
                    errors.len() + passed.len()
                ),
                "dryRun": dry_run,
                "errors": errors,
                "passed": passed,
            }),
            ToolFailure::Batch(RecordBatchError::ReviewTokenMismatch) => json!({
                "code": "REVIEW_TOKEN_MISMATCH",
                "message": "Nothing was recorded: a real write needs the reviewToken of a dry \
                            run of this exact batch, and this one is missing or belongs to a \
                            different batch. Run record_batch with dryRun true again, show the \
                            user the new table, and after their yes send that same batch with \
                            dryRun false and the new reviewToken.",
            }),
            ToolFailure::Undo(UndoBatchError::NotFound { batch_id }) => json!({
                "code": "NOT_FOUND",
                "message": format!("There is no batch {batch_id}"),
            }),
            ToolFailure::Undo(UndoBatchError::Changed { batch_id, rows }) => json!({
                "code": "BATCH_CHANGED",
                "message": format!(
                    "Rows of batch {batch_id} were changed in Saldo since it was recorded. \
                     Nothing was removed. Ask the user whether to remove them too, then call \
                     undo_batch with includeChanged."
                ),
                "batchId": batch_id,
                "rows": rows,
            }),
        }
    }
}

fn validate_dates(dates: &[(&Option<String>, &str)]) -> Result<(), AppError> {
    for (date, field) in dates {
        if let Some(date) = date {
            validate_iso_date(date, field)?;
        }
    }
    Ok(())
}

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAX_PAGE_SIZE: u32 = 500;

fn page_size(limit: Option<u32>) -> u32 {
    limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE)
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SaldoContext {
    database_path: String,
    consolidation_currency: CurrencyRef,
    persons: Vec<PersonContext>,
    tax_models: Vec<TaxModelContext>,
    recent_batches: Vec<BatchSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrencyRef {
    id: i64,
    code: String,
    minor_units: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PersonContext {
    id: i64,
    name: String,
    person_type: String,
    is_default: bool,
    vat_payer: bool,
    revenue_account: AccountRef,
    expense_account: AccountRef,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountRef {
    id: i64,
    currency: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TaxModelContext {
    id: i64,
    name: String,
    person_id: i64,
    year: i64,
}

fn saldo_context(db: &SaldoDb, _: NoArgs) -> Result<SaldoContext, ToolFailure> {
    let conn = &db.conn;
    let currencies = list_currencies(conn, Some(true))?;
    let account_ref = |account_id: i64| -> Result<AccountRef, ToolFailure> {
        let currency_id = get_account_currency_id(conn, account_id)?;
        let currency = currencies
            .iter()
            .find(|c| c.id == currency_id)
            .map(|c| c.code.clone())
            .unwrap_or_default();
        Ok(AccountRef {
            id: account_id,
            currency,
        })
    };
    let persons = list_persons(conn)?
        .into_iter()
        .map(|person| {
            Ok(PersonContext {
                revenue_account: account_ref(person.default_revenue_account_id)?,
                expense_account: account_ref(person.default_expense_account_id)?,
                id: person.id,
                name: person.name,
                person_type: person.person_type,
                is_default: person.is_default,
                vat_payer: person.vat_payer,
            })
        })
        .collect::<Result<Vec<_>, ToolFailure>>()?;
    let tax_models = list_tax_models(conn)?
        .into_iter()
        .map(|model| TaxModelContext {
            id: model.id,
            name: model.name,
            person_id: model.person_id,
            year: model.calendar_year,
        })
        .collect();
    let consolidation = get_consolidation_currency(conn)?;
    Ok(SaldoContext {
        database_path: db.path.display().to_string(),
        consolidation_currency: CurrencyRef {
            id: consolidation.id,
            code: consolidation.code,
            minor_units: consolidation.minor_units,
        },
        persons,
        tax_models,
        recent_batches: list_recent_batches(conn, 20)?,
    })
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FindEntriesInput {
    person_id: Option<i64>,
    /// First day, YYYY-MM-DD.
    from: Option<String>,
    /// Last day, YYYY-MM-DD.
    to: Option<String>,
    /// The document's total in minor units, VAT included; negative for a credit note.
    total_gross_minor: Option<i64>,
    /// Part of a note or of the document reference, ignoring case and accents.
    text: Option<String>,
    /// The issuer as record_batch takes it (IČO when printed): every document from this issuer.
    issuer: Option<String>,
    /// With issuer: exactly that document.
    number: Option<String>,
    batch_id: Option<i64>,
    #[serde(default)]
    kinds: Vec<TaxableKind>,
    /// Most documents to return. Default 50, at most 500.
    limit: Option<u32>,
}

#[derive(Serialize)]
struct EntriesOutput {
    entries: Vec<DocumentEntry>,
}

fn find_entries(db: &SaldoDb, input: FindEntriesInput) -> Result<EntriesOutput, ToolFailure> {
    validate_dates(&[(&input.from, "from"), (&input.to, "to")])?;
    let (source_ref, source_ref_prefix) = match (&input.issuer, &input.number) {
        (Some(issuer), Some(number)) => (
            Some(SourceRef::invoice(issuer, number)?.as_str().to_owned()),
            None,
        ),
        (Some(issuer), None) => (None, Some(invoice_issuer_prefix(issuer)?)),
        (None, Some(_)) => {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "number needs issuer: invoice numbers repeat across issuers".into(),
            }
            .into())
        }
        (None, None) => (None, None),
    };
    let entries = find_document_entries(
        &db.conn,
        FindDocumentEntriesParams {
            person_id: input.person_id,
            from_date: input.from,
            to_date: input.to,
            total_gross_minor: input.total_gross_minor,
            text: input.text,
            source_ref_prefix,
            source_ref,
            batch_id: input.batch_id,
            kinds: input.kinds,
            limit: page_size(input.limit),
        },
    )?;
    Ok(EntriesOutput { entries })
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FindPaymentsInput {
    person_id: Option<i64>,
    /// First day, YYYY-MM-DD.
    from: Option<String>,
    /// Last day, YYYY-MM-DD.
    to: Option<String>,
    /// Minor units, either sign. Matches the amount or the original amount.
    amount_minor: Option<i64>,
    /// ISO code, such as USD: only payments made in that currency.
    original_currency: Option<String>,
    /// Part of the bank text, ignoring case and accents.
    note_contains: Option<String>,
    /// Only payments not linked to any entry yet.
    #[serde(default)]
    unlinked_only: bool,
    /// Most payments to return. Default 50, at most 500.
    limit: Option<u32>,
}

#[derive(Serialize)]
struct PaymentsOutput {
    payments: Vec<Payment>,
}

fn find_payments_tool(
    db: &SaldoDb,
    input: FindPaymentsInput,
) -> Result<PaymentsOutput, ToolFailure> {
    validate_dates(&[(&input.from, "from"), (&input.to, "to")])?;
    let payments = find_payments(
        &db.conn,
        FindPaymentsParams {
            person_id: input.person_id,
            from_date: input.from,
            to_date: input.to,
            amount_either_sign_minor: input.amount_minor,
            original_currency: input.original_currency,
            note_contains: input.note_contains,
            unlinked_only: input.unlinked_only,
            event_ids: None,
            limit: page_size(input.limit),
        },
    )?;
    Ok(PaymentsOutput { payments })
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GetForecastInput {
    tax_model_id: i64,
}

fn get_forecast(
    db: &SaldoDb,
    input: GetForecastInput,
) -> Result<TaxCalculationResult, ToolFailure> {
    Ok(calculate_tax_model_inner(&db.conn, input.tax_model_id)?)
}

fn record_batch(txn: WriteTxn<'_>, input: RecordBatchInput) -> Result<BatchReceipt, ToolFailure> {
    entry_batches::record_batch(txn, input).map_err(ToolFailure::Batch)
}

fn undo_batch(txn: WriteTxn<'_>, input: UndoBatchInput) -> Result<UndoReceipt, ToolFailure> {
    entry_batches::undo_batch(txn, input).map_err(ToolFailure::Undo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DB_FILENAME;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
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

    fn call(dir: &tempfile::TempDir, name: &str, arguments: Value) -> (bool, Value) {
        match Server::new(dir.path().to_path_buf()).call(name, arguments) {
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
            other => panic!("give the read tool {other} sample arguments here"),
        }
    }

    #[test]
    fn every_read_tool_leaves_the_database_unchanged() {
        let seeded = seeded();
        for tool in tools().iter().filter(|tool| tool.access == Access::Read) {
            let db = open_saldo_db(seeded.dir.path()).unwrap();

            let result = (tool.run)(&db, sample_arguments(tool.name, &seeded));

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
        let listed = Server::new(PathBuf::new()).list();

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
        let record_batch = listed[4]["inputSchema"].to_string();
        let required = listed[4]["inputSchema"]["required"].as_array().unwrap();
        assert!(required.contains(&json!("dryRun")), "{required:?}");
        for field in [
            "reviewToken",
            "deliveryDate",
            "deductiblePctBps",
            "prepaidUntil",
            "depreciationMonths",
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
        let listed = Server::new(PathBuf::new()).list();
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
        let reviewed = (record.run)(&db, batch.clone()).ok().unwrap();
        let folder = seeded.dir.path().join("cloud");
        std::fs::create_dir_all(&folder).unwrap();
        let switched_to = crate::db::initialize_db(&folder.join(DB_FILENAME)).unwrap();
        crate::config::write_db_location(seeded.dir.path(), &folder).unwrap();

        let refusal = (record.run)(&db, approved(batch, &reviewed))
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
}
