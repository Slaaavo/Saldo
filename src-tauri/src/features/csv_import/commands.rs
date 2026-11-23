use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use chrono::NaiveDate;
use rusqlite::Connection;
use tauri::State;

use super::matcher::{
    self, posting_change_minor, BankState, Counterpart, CounterpartCashflow, IncomingRow,
    LedgerItem, Match, Recorded,
};
use super::models::{
    ApplyCsvImportInput, CancelledHold, CancelledHoldChoice, CsvImportPreview, CsvImportResult,
    IncomingRowInput, PreviewCsvImportInput, RowChoice, RowPreview, Verdict,
};
use super::repository::{
    self, CancelHoldParams, CounterpartCashflowsParams, InsertBankEntryParams, PostHoldParams,
    RaiseHoldSeenToParams, SetSplitGroupNoteParams,
};
use crate::error::AppError;
use crate::features::accounts::repository::get_account_type;
use crate::features::transactions::commands::{delete_event_inner, delete_split_group_inner};
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
        let matched = read.matched();
        let update_days: HashSet<NaiveDate> =
            repository::balance_update_days(conn, input.account_id)?
                .into_iter()
                .collect();
        let row_days: BTreeSet<NaiveDate> = read.rows.iter().map(|row| row.day).collect();
        Ok(CsvImportPreview {
            rows: read
                .rows
                .iter()
                .zip(&matched.rows)
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
            cancelled_holds: matched
                .cancelled_holds
                .iter()
                .map(|hold| CancelledHold {
                    event_id: hold.bank_entry_event_id,
                    event_date: hold.event_date.clone(),
                    amount_minor: hold.total_minor,
                    note: hold.own_note.clone(),
                })
                .collect(),
            review_token: review_token(input.account_id, &read),
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
    if review_token(input.account_id, &read) != input.review_token {
        return Err(AppError {
            code: "IMPORT_CHANGED".into(),
            message: "Saldo changed since this import was reviewed. Review it again.".into(),
        });
    }
    let matched = read.matched();
    let removals = parse_removals(&matched.cancelled_holds, input.cancelled_holds)?;
    let mut result = CsvImportResult {
        imported: 0,
        removed_holds: removals.len() as u32,
        repriced_holds: 0,
    };
    if let Some(newest_day) = read.rows.iter().map(|row| row.day).max() {
        let writer = Writer {
            conn,
            account_id: input.account_id,
            newest_day,
        };
        for ((row, &matched), choice) in read.rows.iter().zip(&matched.rows).zip(choices) {
            match writer.write_row(row, matched, choice)? {
                Written::Entries(count) => result.imported += count,
                Written::RepricedHold => result.repriced_holds += 1,
            }
        }
        for hold in removals {
            writer.remove_cancelled_hold(hold)?;
        }
    }
    txn.finish(Finish::Commit)?;
    Ok(result)
}

enum Written {
    Entries(u32),
    RepricedHold,
}

enum Choice {
    Unticked,
    Ticked { bucket_id: Option<i64> },
    Split(Vec<SplitLeg>),
}

struct SplitLeg {
    amount_minor: i64,
    note: Option<String>,
    counterpart: Counterpart,
    bucket_id: Option<i64>,
}

#[derive(Hash)]
struct ImportRead {
    rows: Vec<IncomingRow>,
    items: Vec<LedgerItem>,
    removed_holds: Vec<Recorded>,
    cashflows: Vec<CounterpartCashflow>,
}

impl ImportRead {
    fn matched(&self) -> matcher::Matched<'_> {
        matcher::match_rows(
            &self.rows,
            &self.items,
            &self.removed_holds,
            &self.cashflows,
        )
    }
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
            Counterpart::None | Counterpart::Unknown | Counterpart::Partner(_) => None,
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
        removed_holds: repository::removed_holds(conn, account_id)?,
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
                counterpart: row_counterpart(conn, account_id, row)?,
                original_currency_id: row.original_currency_id,
                original_amount_minor: row.original_amount_minor,
                fx_rate_mantissa: row.fx_rate_mantissa,
                fx_rate_exponent: row.fx_rate_exponent,
                card: row
                    .card
                    .as_deref()
                    .map(str::trim)
                    .filter(|card| !card.is_empty())
                    .map(str::to_owned),
                pending: row.pending,
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

fn row_counterpart(
    conn: &Connection,
    account_id: i64,
    row: &IncomingRowInput,
) -> Result<Counterpart, AppError> {
    match (row.names_unknown_account, row.counterpart_account_id) {
        (false, counterpart_account_id) => {
            parse_counterpart(conn, account_id, counterpart_account_id)
        }
        (true, None) => Ok(Counterpart::Unknown),
        (true, Some(_)) => Err(validation(format!(
            "Row {} names both an unknown IBAN and an account",
            row.index
        ))),
    }
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
        return Ok(Choice::Unticked);
    }
    let Some(legs) = choice.split else {
        return Ok(Choice::Ticked {
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

fn parse_removals<'a>(
    cancelled_holds: &[&'a Recorded],
    choices: Vec<CancelledHoldChoice>,
) -> Result<Vec<&'a Recorded>, AppError> {
    let mut remove: HashMap<i64, bool> = HashMap::new();
    for choice in choices {
        if remove.insert(choice.event_id, choice.remove).is_some() {
            return Err(validation(format!(
                "Cancelled hold {} has two choices",
                choice.event_id
            )));
        }
    }
    if remove.len() != cancelled_holds.len() {
        return Err(validation(
            "Every cancelled card hold needs exactly one choice",
        ));
    }
    cancelled_holds
        .iter()
        .filter_map(|&hold| match remove.get(&hold.bank_entry_event_id) {
            None => Some(Err(validation(format!(
                "Cancelled hold {} has no choice",
                hold.bank_entry_event_id
            )))),
            Some(true) => Some(Ok(hold)),
            Some(false) => None,
        })
        .collect()
}

struct Writer<'c> {
    conn: &'c Connection,
    account_id: i64,
    newest_day: NaiveDate,
}

impl Writer<'_> {
    fn write_row(
        &self,
        row: &IncomingRow,
        matched: Match<'_>,
        choice: Choice,
    ) -> Result<Written, AppError> {
        let conn = self.conn;
        if let (Choice::Ticked { .. } | Choice::Split(_), Match::HoldPosted(hold)) =
            (&choice, matched)
        {
            repository::raise_hold_seen_to(
                conn,
                RaiseHoldSeenToParams {
                    event_id: hold.bank_entry_event_id,
                    seen_to: self.newest_day,
                },
            )?;
        }
        match (choice, matched) {
            (
                Choice::Unticked,
                Match::New
                | Match::NewReplacingSplitHold(_)
                | Match::CompletesTransfer(_)
                | Match::HoldPostedAtOtherAmount(_),
            ) => Ok(Written::Entries(0)),
            (Choice::Unticked, Match::InSaldo(recorded)) => {
                self.catch_up_with_bank_line(row, recorded)?;
                Ok(Written::Entries(0))
            }
            (Choice::Unticked, Match::HoldPosted(hold)) => {
                self.post_hold(row, hold, hold.total_minor)?;
                Ok(Written::Entries(0))
            }
            (Choice::Ticked { .. }, Match::HoldPostedAtOtherAmount(hold)) => {
                self.post_hold(row, hold, row.amount_minor)?;
                Ok(Written::RepricedHold)
            }
            (Choice::Split(_), Match::HoldPostedAtOtherAmount(_)) => Err(validation(format!(
                "Row {} posts a card hold at another amount and cannot be split",
                row.index
            ))),
            (Choice::Unticked, Match::TransferLeg { leg, .. }) => {
                update_event(
                    conn,
                    UpdateEventParams {
                        event_id: leg.bank_entry_event_id,
                        amount_minor: leg.total_minor,
                        event_date: row.event_date.clone(),
                        note: leg.own_note.clone(),
                    },
                )?;
                self.record_bank_entry(leg.bank_entry_event_id, row)?;
                Ok(Written::Entries(0))
            }
            (Choice::Ticked { bucket_id }, Match::CompletesTransfer(cashflow)) => {
                let leg_id = complete_transfer(
                    conn,
                    &cashflow_entry(self.account_id, row, bucket_id),
                    cashflow.recorded.bank_entry_event_id,
                )?;
                self.record_bank_entry(leg_id, row)?;
                Ok(Written::Entries(1))
            }
            (
                Choice::Ticked { bucket_id },
                Match::New
                | Match::NewReplacingSplitHold(_)
                | Match::InSaldo(_)
                | Match::TransferLeg { .. }
                | Match::HoldPosted(_),
            ) => {
                let (event_id, _) = create_cashflow_or_transfer(
                    conn,
                    &cashflow_entry(self.account_id, row, bucket_id),
                )?;
                self.record_bank_entry(event_id, row)?;
                Ok(Written::Entries(1))
            }
            (
                Choice::Split(legs),
                Match::New
                | Match::NewReplacingSplitHold(_)
                | Match::InSaldo(_)
                | Match::TransferLeg { .. }
                | Match::CompletesTransfer(_)
                | Match::HoldPosted(_),
            ) => {
                let leg_count = legs.len() as u32;
                let split_group_id = create_split_group_with_legs(
                    conn,
                    CreateSplitGroupWithLegsParams {
                        account_id: self.account_id,
                        group_note: row.note.clone(),
                        legs: split_entries(row, legs)?,
                    },
                )?;
                self.record_bank_entry(repository::lowest_leg_id(conn, split_group_id)?, row)?;
                Ok(Written::Entries(leg_count))
            }
        }
    }

    fn catch_up_with_bank_line(
        &self,
        row: &IncomingRow,
        recorded: &Recorded,
    ) -> Result<(), AppError> {
        match (
            recorded.bank.as_ref().map(|bank| bank.state),
            self.hold_seen_to(row),
        ) {
            (None, _) => self.record_bank_entry(recorded.bank_entry_event_id, row)?,
            (Some(BankState::Pending { .. }), Some(seen_to)) => repository::raise_hold_seen_to(
                self.conn,
                RaiseHoldSeenToParams {
                    event_id: recorded.bank_entry_event_id,
                    seen_to,
                },
            )?,
            (Some(_), _) => {}
        }
        if !row.pending {
            let note = recorded.note_without_hold_padding(row.note.as_deref());
            if note != recorded.own_note {
                self.set_note(recorded, note)?;
            }
        }
        Ok(())
    }

    fn post_hold(
        &self,
        row: &IncomingRow,
        hold: &Recorded,
        amount_minor: i64,
    ) -> Result<(), AppError> {
        let note = hold.note_once_booked(row.note.as_deref());
        if amount_minor != hold.total_minor {
            update_event(
                self.conn,
                UpdateEventParams {
                    event_id: hold.bank_entry_event_id,
                    amount_minor,
                    event_date: hold.event_date.clone(),
                    note,
                },
            )?;
        } else if note != hold.own_note {
            self.set_note(hold, note)?;
        }
        self.record_posting(row, hold, amount_minor)
    }

    fn record_posting(
        &self,
        row: &IncomingRow,
        hold: &Recorded,
        total_after_minor: i64,
    ) -> Result<(), AppError> {
        repository::post_hold(
            self.conn,
            PostHoldParams {
                event_id: hold.bank_entry_event_id,
                day: row.day,
                amount_minor: row.amount_minor,
                text: row.note.clone(),
                card: row.card.clone(),
                posting_change_minor: posting_change_minor(hold.total_minor, total_after_minor),
            },
        )?;
        Ok(())
    }

    fn set_note(&self, recorded: &Recorded, note: Option<String>) -> Result<(), AppError> {
        match recorded.split_group_id {
            None => update_event(
                self.conn,
                UpdateEventParams {
                    event_id: recorded.bank_entry_event_id,
                    amount_minor: recorded.total_minor,
                    event_date: recorded.event_date.clone(),
                    note,
                },
            ),
            Some(split_group_id) => repository::set_split_group_note(
                self.conn,
                SetSplitGroupNoteParams {
                    split_group_id,
                    note,
                },
            )
            .map_err(AppError::from),
        }
    }

    fn remove_cancelled_hold(&self, hold: &Recorded) -> Result<(), AppError> {
        match hold.split_group_id {
            None => delete_event_inner(self.conn, hold.bank_entry_event_id)?,
            Some(split_group_id) => delete_split_group_inner(self.conn, split_group_id)?,
        }
        repository::cancel_hold(
            self.conn,
            CancelHoldParams {
                event_id: hold.bank_entry_event_id,
                by_day: self.newest_day,
            },
        )?;
        Ok(())
    }

    fn record_bank_entry(&self, event_id: i64, row: &IncomingRow) -> rusqlite::Result<()> {
        repository::insert_bank_entry(
            self.conn,
            InsertBankEntryParams {
                event_id,
                day: row.day,
                amount_minor: row.amount_minor,
                text: row.note.clone(),
                card: row.card.clone(),
                hold_seen_to: self.hold_seen_to(row),
            },
        )
    }

    fn hold_seen_to(&self, row: &IncomingRow) -> Option<NaiveDate> {
        row.pending.then_some(self.newest_day)
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

fn verdict_of(matched: Match<'_>) -> Verdict {
    match matched {
        Match::New => Verdict::New,
        Match::InSaldo(recorded) => Verdict::InSaldo {
            event_id: recorded.bank_entry_event_id,
            event_date: recorded.event_date.clone(),
            note: recorded.own_note.clone(),
        },
        Match::TransferLeg {
            leg,
            other_account_name,
        } => Verdict::TransferLeg {
            event_id: leg.bank_entry_event_id,
            other_account_name: other_account_name.to_owned(),
            event_date: leg.event_date.clone(),
        },
        Match::CompletesTransfer(cashflow) => Verdict::CompletesTransfer {
            event_id: cashflow.recorded.bank_entry_event_id,
            account_id: cashflow.account_id,
            account_name: cashflow.account_name.clone(),
            event_date: cashflow.recorded.event_date.clone(),
            note: cashflow.recorded.own_note.clone(),
        },
        Match::HoldPosted(hold) => Verdict::HoldPosted {
            event_id: hold.bank_entry_event_id,
            event_date: hold.event_date.clone(),
            note: hold.own_note.clone(),
        },
        Match::HoldPostedAtOtherAmount(hold) => Verdict::HoldPostedAtOtherAmount {
            event_id: hold.bank_entry_event_id,
            event_date: hold.event_date.clone(),
            note: hold.own_note.clone(),
            hold_amount_minor: hold.total_minor,
        },
        Match::NewReplacingSplitHold(hold) => Verdict::NewReplacingSplitHold {
            event_id: hold.bank_entry_event_id,
            event_date: hold.event_date.clone(),
            hold_amount_minor: hold.total_minor,
        },
    }
}

fn review_token(account_id: i64, read: &ImportRead) -> String {
    let mut hasher = DefaultHasher::new();
    (account_id, read).hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn validation(message: impl Into<String>) -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message: message.into(),
    }
}
