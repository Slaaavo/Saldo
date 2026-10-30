use schemars::generate::SchemaSettings;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use super::open::{open_saldo_db, OpenRefusal, SaldoDb};
use super::protocol::{ToolHost, ToolOutcome};
use crate::error::AppError;
use crate::features::accounts::repository::get_account_currency_id;
use crate::features::currency::repository::{get_consolidation_currency, list_currencies};
use crate::features::ekasa::reading::{read_receipt_file, EkasaApi, ReceiptReading};
use crate::features::entry_batches::models::{
    BatchReceipt, BatchSummary, RecordBatchError, RecordBatchInput, UndoBatchError, UndoBatchInput,
    UndoReceipt,
};
use crate::features::entry_batches::{self, list_recent_batches};
use crate::features::persons::repository::list_persons;
use crate::features::tax_models::commands::calculate_tax_model_inner;
use crate::features::tax_models::models::TaxCalculationResult;
use crate::features::tax_models::repository::list_tax_models;
use crate::features::transactions::models::{DocumentEntry, Payment, RecordedSource, TaxableKind};
use crate::features::transactions::repository::{
    find_document_entries, find_payments, find_recorded_source, FindDocumentEntriesParams,
    FindPaymentsParams,
};
use crate::features::transactions::source_ref::invoice_issuer_prefix;
use crate::features::transactions::SourceRef;
use crate::shared::{validate_iso_date, with_read_txn, WriteTxn};

pub(super) struct Server {
    app_data_dir: PathBuf,
    outside: Outside,
}

impl Server {
    pub(super) fn new(app_data_dir: PathBuf, outside: Outside) -> Self {
        Self {
            app_data_dir,
            outside,
        }
    }
}

pub(super) struct Outside {
    pub resource_dir: Option<PathBuf>,
    pub ekasa: Box<dyn EkasaApi>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Write,
}

type Run = Box<dyn Fn(&Outside, &SaldoDb, Value) -> Result<Value, ToolFailure>>;

struct Tool {
    name: &'static str,
    description: &'static str,
    access: Access,
    /// Reads files or calls a service outside Saldo: true only for a tool built with
    /// `read_tool_after_outside`.
    open_world: bool,
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
        read_tool_after_outside(
            "read_receipt",
            "Read a Slovak eKasa receipt from a photo, screenshot or PDF of its QR code (png, \
             jpg, jpeg or pdf, by absolute path). Saldo decodes the QR code. For an online \
             receipt it asks Finančná správa's eKasa service and returns the date, the shop and \
             the lines. An offline receipt's code holds only its date and total: read the shop \
             and every line (name as printed, gross amount, VAT rate) from the receipt yourself. \
             When the QR code does not decode, call it again with the online receipt id printed \
             on the receipt as code; Saldo still reads the QR code first. Put receipt into \
             record_batch unchanged, with date and an online receipt's lines as returned. \
             alreadyRecorded names what already carries this receipt.",
            read_receipt_outside,
            read_receipt_recorded,
        ),
        write_tool(
            "record_batch",
            "Record documents (invoices, credit notes) and eKasa receipts as revenue, expense \
             and prepaid expense entries and assets, all of them or none. Saldo dates an \
             invoice by its delivery date (else the issue date) and a receipt by its own date, \
             makes a split group of two or more lines, prices an asset without VAT when the VAT \
             is reclaimed, and links the given payments. A receipt line's shares: the user's \
             win, then the person's eKasa rule that matches the line, then yours; the dry run \
             says where each came from. A document or receipt already in Saldo is refused. Call \
             it with dryRun true first and show the user the table. Only after the user \
             approved it, call it again with the same batch, dryRun false and the reviewToken \
             the dry run returned. A batch that differs from the reviewed one in any field, or \
             whose eKasa rules or VAT-payer flag changed in Saldo since, is refused.",
            record_batch,
        ),
        write_tool(
            "undo_batch",
            "Remove everything one batch recorded: entries, their prepaid shares and split \
             legs, assets and their depreciation, payment links. Undoing twice changes nothing. \
             When something was changed in Saldo since (an entry edited, a leg or an asset value \
             added, an asset edited, a payment link added or removed), it refuses and lists the \
             changes.",
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
        open_world: false,
        input_schema: input_schema::<I>,
        run: Box::new(move |_, db, arguments| {
            let input = parse_arguments(arguments)?;
            output_value(with_read_txn(&db.conn, || handler(db, input))?)
        }),
    }
}

fn read_tool_after_outside<I, P, O>(
    name: &'static str,
    description: &'static str,
    outside: fn(&Outside, I) -> Result<P, ToolFailure>,
    handler: fn(&SaldoDb, P) -> Result<O, ToolFailure>,
) -> Tool
where
    I: DeserializeOwned + JsonSchema + 'static,
    P: 'static,
    O: Serialize + 'static,
{
    Tool {
        name,
        description,
        access: Access::Read,
        open_world: true,
        input_schema: input_schema::<I>,
        run: Box::new(move |env, db, arguments| {
            let prepared = outside(env, parse_arguments(arguments)?)?;
            output_value(with_read_txn(&db.conn, || handler(db, prepared))?)
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
        open_world: false,
        input_schema: input_schema::<I>,
        run: Box::new(move |_, db, arguments| {
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
                let mut annotations = json!({ "readOnlyHint": tool.access == Access::Read });
                if tool.open_world {
                    annotations["openWorldHint"] = json!(true);
                }
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": (tool.input_schema)(),
                    "annotations": annotations,
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
            .and_then(|db| (tool.run)(&self.outside, &db, arguments));
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
                            run of this exact batch, and this one is missing, belongs to a \
                            different batch, or Saldo's eKasa rules or VAT-payer flag changed \
                            since. Run record_batch with dryRun true again, show the user the \
                            new table, and after their yes send that same batch with dryRun \
                            false and the new reviewToken.",
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

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadReceiptInput {
    /// The receipt file's absolute path.
    file: String,
    /// The online receipt id printed on the receipt, O- or V- and 32 hex digits. Saldo reads the
    /// QR code first and uses this id only when the QR code does not decode. It refuses an id
    /// that names another receipt than the QR code. An offline receipt cannot be read this way.
    code: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadReceiptOutput {
    #[serde(flatten)]
    reading: ReceiptReading,
    /// The live entry or asset that already carries this receipt. record_batch would refuse
    /// it as ALREADY_RECORDED.
    already_recorded: Option<RecordedSource>,
}

fn read_receipt_outside(
    outside: &Outside,
    input: ReadReceiptInput,
) -> Result<ReceiptReading, ToolFailure> {
    if !Path::new(&input.file).is_absolute() {
        return Err(ToolFailure::InvalidArguments {
            path: "file".into(),
            message: "must be an absolute path: saldo mcp does not run in the user's folder".into(),
        });
    }
    Ok(read_receipt_file(
        &input.file,
        input.code.as_deref(),
        outside.resource_dir.as_deref(),
        outside.ekasa.as_ref(),
    )?)
}

fn read_receipt_recorded(
    db: &SaldoDb,
    reading: ReceiptReading,
) -> Result<ReadReceiptOutput, ToolFailure> {
    Ok(ReadReceiptOutput {
        already_recorded: find_recorded_source(&db.conn, &reading.source_ref)?,
        reading,
    })
}

fn record_batch(txn: WriteTxn<'_>, input: RecordBatchInput) -> Result<BatchReceipt, ToolFailure> {
    entry_batches::record_batch(txn, input).map_err(ToolFailure::Batch)
}

fn undo_batch(txn: WriteTxn<'_>, input: UndoBatchInput) -> Result<UndoReceipt, ToolFailure> {
    entry_batches::undo_batch(txn, input).map_err(ToolFailure::Undo)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
