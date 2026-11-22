use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use chrono::NaiveDate;
use rusqlite::Connection;
use tauri::State;

use super::matcher::{self, Counterpart, CounterpartCashflow, IncomingRow, LedgerItem, Match};
use super::models::{
    ApplyCsvImportInput, CsvImportPreview, CsvImportResult, IncomingRowInput,
    PreviewCsvImportInput, RowChoice, RowPreview, Verdict,
};
use super::repository::{self, CounterpartCashflowsParams, InsertBankEntryParams};
use crate::error::AppError;
use crate::features::accounts::repository::get_account_type;
use crate::features::transactions::models::SplitGroupEntry;
use crate::features::transactions::repository::{
    complete_transfer, create_cashflow_or_transfer, create_split_group_with_legs, update_event,
    CashflowEntry, CreateSplitGroupWithLegsParams, UpdateEventParams,
};
use crate::shared::{div_round_i128, validate_event_date, with_read_txn, Finish, WriteTxn};
use crate::AppState;

const OWN_ACCOUNT: &str = "account";
const PARTNER: &str = "partner";

#[tauri::command]
pub fn preview_csv_import(
    state: State<'_, AppState>,
    input: PreviewCsvImportInput,
) -> Result<CsvImportPreview, AppError> {
    let conn = state.conn()?;
    preview_csv_import_inner(&conn, input)
}

#[tauri::command]
pub fn apply_csv_import(
    state: State<'_, AppState>,
    input: ApplyCsvImportInput,
) -> Result<CsvImportResult, AppError> {
    let conn = state.conn()?;
    apply_csv_import_inner(&conn, input)
}

pub(crate) fn preview_csv_import_inner(
    conn: &Connection,
    input: PreviewCsvImportInput,
) -> Result<CsvImportPreview, AppError> {
    with_read_txn(conn, || {
        let read = read_import(conn, input.account_id, &input.rows)?;
        let matches = matcher::match_rows(&read.rows, &read.items, &read.cashflows);
        let update_days: HashSet<NaiveDate> =
            repository::balance_update_days(conn, input.account_id)?
                .into_iter()
                .collect();
        let row_days: BTreeSet<NaiveDate> = read.rows.iter().map(|row| row.day).collect();
        Ok(CsvImportPreview {
            rows: read
                .rows
                .iter()
                .zip(&matches)
                .map(|(row, &matched)| RowPreview {
                    index: row.index,
                    verdict: verdict_of(matched),
                })
                .collect(),
            balance_update_days: row_days
                .into_iter()
                .filter(|day| update_days.contains(day))
                .map(|day| day.format("%Y-%m-%d").to_string())
                .collect(),
            review_token: review_token(input.account_id, &input.rows, &matches),
        })
    })
}

pub(crate) fn apply_csv_import_inner(
    conn: &Connection,
    input: ApplyCsvImportInput,
) -> Result<CsvImportResult, AppError> {
    let txn = WriteTxn::begin(conn)?;
    let read = read_import(conn, input.account_id, &input.rows)?;
    let choices = parse_choices(conn, input.account_id, &read.rows, input.choices)?;
    let matches = matcher::match_rows(&read.rows, &read.items, &read.cashflows);
    if review_token(input.account_id, &input.rows, &matches) != input.review_token {
        return Err(AppError {
            code: "IMPORT_CHANGED".into(),
            message: "Saldo changed since this import was reviewed. Review it again.".into(),
        });
    }
    let mut imported = 0;
    for ((row, &matched), choice) in read.rows.iter().zip(&matches).zip(choices) {
        imported += write_row(conn, input.account_id, row, matched, choice)?;
    }
    txn.finish(Finish::Commit)?;
    Ok(CsvImportResult { imported })
}

/// What the user chose for a row, checked against the row.
enum Choice {
    Skip,
    Create { bucket_id: Option<i64> },
    Split(Vec<SplitLeg>),
}

struct SplitLeg {
    amount_minor: i64,
    note: Option<String>,
    counterpart: Counterpart,
    bucket_id: Option<i64>,
}

struct ImportRead {
    rows: Vec<IncomingRow>,
    items: Vec<LedgerItem>,
    cashflows: Vec<CounterpartCashflow>,
}

fn read_import(
    conn: &Connection,
    account_id: i64,
    rows: &[IncomingRowInput],
) -> Result<ImportRead, AppError> {
    match get_account_type(conn, account_id)?.as_deref() {
        None => return Err(validation("Account not found")),
        Some(PARTNER) => return Err(validation("Cannot create events on partner accounts")),
        Some(OWN_ACCOUNT) => {}
        Some(_) => {
            return Err(validation(
                "A bank export can only be imported into an own account",
            ))
        }
    }
    let rows = parse_rows(conn, account_id, rows)?;
    let own_counterparts: BTreeSet<i64> = rows
        .iter()
        .filter_map(|row| match row.counterpart {
            Counterpart::Own(id) => Some(id),
            Counterpart::None | Counterpart::Partner(_) => None,
        })
        .collect();
    let mut cashflows = Vec::new();
    for counterpart in own_counterparts {
        cashflows.extend(repository::counterpart_cashflows(
            conn,
            CounterpartCashflowsParams {
                account_id: counterpart,
                importing_account_id: account_id,
            },
        )?);
    }
    Ok(ImportRead {
        rows,
        items: repository::ledger_items(conn, account_id)?,
        cashflows,
    })
}

fn parse_rows(
    conn: &Connection,
    account_id: i64,
    rows: &[IncomingRowInput],
) -> Result<Vec<IncomingRow>, AppError> {
    let mut indexes = HashSet::new();
    rows.iter()
        .map(|row| {
            if !indexes.insert(row.index) {
                return Err(validation(format!("Row {} appears twice", row.index)));
            }
            Ok(IncomingRow {
                index: row.index,
                event_date: row.event_date.clone(),
                day: day_of(&row.event_date)?,
                amount_minor: row.amount_minor,
                note: row.note.clone(),
                counterpart: parse_counterpart(conn, account_id, row.counterpart_account_id)?,
                original_currency_id: row.original_currency_id,
                original_amount_minor: row.original_amount_minor,
                fx_rate_mantissa: row.fx_rate_mantissa,
                fx_rate_exponent: row.fx_rate_exponent,
            })
        })
        .collect()
}

fn day_of(event_date: &str) -> Result<NaiveDate, AppError> {
    validate_event_date(event_date)?;
    event_date
        .get(..10)
        .and_then(|day| NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
        .ok_or_else(|| validation("event_date must start with a YYYY-MM-DD date"))
}

fn parse_counterpart(
    conn: &Connection,
    account_id: i64,
    counterpart_account_id: Option<i64>,
) -> Result<Counterpart, AppError> {
    let Some(id) = counterpart_account_id else {
        return Ok(Counterpart::None);
    };
    if id == account_id {
        return Err(validation("Cannot transfer to the same account"));
    }
    match get_account_type(conn, id)?.as_deref() {
        None => Err(validation("Counterpart account not found")),
        Some(OWN_ACCOUNT) => Ok(Counterpart::Own(id)),
        Some(PARTNER) => Ok(Counterpart::Partner(id)),
        Some(_) => Err(validation(
            "A counterpart must be an own account or a partner",
        )),
    }
}

/// One choice per row, in the order of `rows`.
fn parse_choices(
    conn: &Connection,
    account_id: i64,
    rows: &[IncomingRow],
    choices: Vec<RowChoice>,
) -> Result<Vec<Choice>, AppError> {
    let mut by_index: HashMap<u32, RowChoice> = HashMap::new();
    for choice in choices {
        let index = choice.index;
        if by_index.insert(index, choice).is_some() {
            return Err(validation(format!("Row {index} has two choices")));
        }
    }
    if by_index.len() != rows.len() {
        return Err(validation("Every row needs exactly one choice"));
    }
    rows.iter()
        .map(|row| {
            let choice = by_index
                .remove(&row.index)
                .ok_or_else(|| validation(format!("Row {} has no choice", row.index)))?;
            parse_choice(conn, account_id, row, choice)
        })
        .collect()
}

fn parse_choice(
    conn: &Connection,
    account_id: i64,
    row: &IncomingRow,
    choice: RowChoice,
) -> Result<Choice, AppError> {
    if !choice.selected {
        return Ok(Choice::Skip);
    }
    let Some(legs) = choice.split else {
        return Ok(Choice::Create {
            bucket_id: choice.bucket_id,
        });
    };
    if legs.len() < 2 {
        return Err(validation("A split needs at least 2 legs"));
    }
    let total: i128 = legs.iter().map(|leg| i128::from(leg.amount_minor)).sum();
    if total != i128::from(row.amount_minor) {
        return Err(validation("A split's legs must add up to the row's amount"));
    }
    legs.into_iter()
        .map(|leg| {
            Ok(SplitLeg {
                amount_minor: leg.amount_minor,
                note: leg.note,
                counterpart: parse_counterpart(conn, account_id, leg.counterpart_account_id)?,
                bucket_id: leg.bucket_id,
            })
        })
        .collect::<Result<_, AppError>>()
        .map(Choice::Split)
}

/// Writes what the row's verdict and choice call for. Returns the entries it created.
fn write_row(
    conn: &Connection,
    account_id: i64,
    row: &IncomingRow,
    matched: Match<'_>,
    choice: Choice,
) -> Result<u32, AppError> {
    match (choice, matched) {
        (Choice::Skip, Match::New | Match::CompletesTransfer(_)) => Ok(0),
        (Choice::Skip, Match::InSaldo(recorded)) => {
            if recorded.bank.is_none() {
                record_bank_entry(conn, recorded.event_id, row)?;
            }
            Ok(0)
        }
        (Choice::Skip, Match::TransferLeg { leg, .. }) => {
            update_event(
                conn,
                UpdateEventParams {
                    event_id: leg.event_id,
                    amount_minor: leg.amount_minor,
                    event_date: row.event_date.clone(),
                    note: leg.note.clone(),
                },
            )?;
            record_bank_entry(conn, leg.event_id, row)?;
            Ok(0)
        }
        (Choice::Create { bucket_id }, Match::CompletesTransfer(cashflow)) => {
            let leg_id = complete_transfer(
                conn,
                &cashflow_entry(account_id, row, bucket_id),
                cashflow.recorded.event_id,
            )?;
            record_bank_entry(conn, leg_id, row)?;
            Ok(1)
        }
        (Choice::Create { bucket_id }, _) => {
            let (event_id, _) =
                create_cashflow_or_transfer(conn, &cashflow_entry(account_id, row, bucket_id))?;
            record_bank_entry(conn, event_id, row)?;
            Ok(1)
        }
        (Choice::Split(legs), _) => {
            let leg_count = legs.len() as u32;
            let split_group_id = create_split_group_with_legs(
                conn,
                CreateSplitGroupWithLegsParams {
                    account_id,
                    group_note: row.note.clone(),
                    legs: split_entries(row, legs)?,
                },
            )?;
            record_bank_entry(conn, repository::lowest_leg_id(conn, split_group_id)?, row)?;
            Ok(leg_count)
        }
    }
}

fn cashflow_entry(account_id: i64, row: &IncomingRow, bucket_id: Option<i64>) -> CashflowEntry {
    CashflowEntry {
        account_id,
        amount_minor: row.amount_minor,
        event_date: row.event_date.clone(),
        note: row.note.clone(),
        counterpart_account_id: row.counterpart.account_id(),
        bucket_id,
        original_currency_id: row.original_currency_id,
        original_amount_minor: row.original_amount_minor,
        fx_rate_mantissa: row.fx_rate_mantissa,
        fx_rate_exponent: row.fx_rate_exponent,
    }
}

/// The legs take the row's date and FX rate, and share its original amount by each leg's share
/// of the row, the last leg taking the remainder.
fn split_entries(row: &IncomingRow, legs: Vec<SplitLeg>) -> Result<Vec<SplitGroupEntry>, AppError> {
    let last = legs.len() - 1;
    let mut allocated: i128 = 0;
    legs.into_iter()
        .enumerate()
        .map(|(position, leg)| {
            let original_amount_minor =
                match row.original_amount_minor {
                    None => None,
                    Some(original) => {
                        let share = if position == last {
                            i128::from(original) - allocated
                        } else {
                            original_share(original, leg.amount_minor, row.amount_minor)
                        };
                        allocated += share;
                        Some(i64::try_from(share).map_err(|_| {
                            validation("A split leg's original amount is out of range")
                        })?)
                    }
                };
            Ok(SplitGroupEntry {
                amount_minor: leg.amount_minor,
                event_date: row.event_date.clone(),
                note: leg.note,
                counterpart_account_id: leg.counterpart.account_id(),
                bucket_id: leg.bucket_id,
                original_currency_id: row.original_currency_id,
                original_amount_minor,
                fx_rate_mantissa: row.fx_rate_mantissa,
                fx_rate_exponent: row.fx_rate_exponent,
            })
        })
        .collect()
}

fn original_share(original: i64, leg_amount: i64, row_amount: i64) -> i128 {
    if row_amount == 0 {
        return 0;
    }
    let numerator = i128::from(original) * i128::from(leg_amount);
    let denominator = i128::from(row_amount);
    div_round_i128(numerator * denominator.signum(), denominator.abs())
}

fn record_bank_entry(conn: &Connection, event_id: i64, row: &IncomingRow) -> rusqlite::Result<()> {
    repository::insert_bank_entry(
        conn,
        InsertBankEntryParams {
            event_id,
            day: row.day,
            amount_minor: row.amount_minor,
            text: row.note.clone(),
        },
    )
}

fn verdict_of(matched: Match<'_>) -> Verdict {
    match matched {
        Match::New => Verdict::New,
        Match::InSaldo(recorded) => Verdict::InSaldo {
            event_id: recorded.event_id,
            event_date: recorded.event_date.clone(),
            note: recorded.note.clone(),
        },
        Match::TransferLeg {
            leg,
            other_account_name,
        } => Verdict::TransferLeg {
            event_id: leg.event_id,
            other_account_name: other_account_name.to_owned(),
            event_date: leg.event_date.clone(),
        },
        Match::CompletesTransfer(cashflow) => Verdict::CompletesTransfer {
            event_id: cashflow.recorded.event_id,
            account_id: cashflow.account_id,
            account_name: cashflow.account_name.clone(),
            event_date: cashflow.recorded.event_date.clone(),
            note: cashflow.recorded.note.clone(),
        },
    }
}

/// Covers the rows as sent and each verdict with the version of the entry it names, so an apply
/// writes only what the user reviewed.
fn review_token(account_id: i64, rows: &[IncomingRowInput], matches: &[Match<'_>]) -> String {
    let mut hasher = DefaultHasher::new();
    (account_id, rows).hash(&mut hasher);
    for &matched in matches {
        let version = matched.recorded().map(|recorded| recorded.latest_data_id);
        (verdict_of(matched), version).hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

fn validation(message: impl Into<String>) -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message: message.into(),
    }
}
