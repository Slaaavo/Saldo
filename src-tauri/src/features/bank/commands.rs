use rusqlite::Connection;
use serde::Deserialize;
use tauri::State;

use super::checks::verification_of;
use super::models::{BookedDifference, Verification};
use super::repository::{
    self, InsertCheckParams, InsertUnexplainedDifferenceParams, SaldoSideParams,
};
use crate::error::AppError;
use crate::features::accounts::repository::get_account_type;
use crate::shared::{local_now, with_savepoint_app, MAX_AMOUNT_MINOR};
use crate::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmBankMatchInput {
    pub account_id: i64,
    pub seen_balance_minor: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookUnexplainedDifferenceInput {
    pub account_id: i64,
    pub bank_balance_minor: i64,
    pub seen_balance_minor: i64,
}

#[tauri::command]
pub fn confirm_bank_match(
    state: State<'_, AppState>,
    input: ConfirmBankMatchInput,
) -> Result<i64, AppError> {
    let conn = state.conn()?;
    confirm_bank_match_inner(&conn, input)
}

#[tauri::command]
pub fn book_unexplained_difference(
    state: State<'_, AppState>,
    input: BookUnexplainedDifferenceInput,
) -> Result<BookedDifference, AppError> {
    let conn = state.conn()?;
    book_unexplained_difference_inner(&conn, input)
}

#[tauri::command]
pub fn delete_balance_check(state: State<'_, AppState>, check_id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    delete_balance_check_inner(&conn, check_id)
}

/// Records that the bank app shows Saldo's live balance, the one the user saw.
pub(crate) fn confirm_bank_match_inner(
    conn: &Connection,
    input: ConfirmBankMatchInput,
) -> Result<i64, AppError> {
    with_savepoint_app(conn, || {
        let live = live_balance_as_seen(conn, input.account_id, input.seen_balance_minor)?;
        Ok(repository::insert_check(
            conn,
            InsertCheckParams {
                account_id: input.account_id,
                checked_on: live.checked_on,
                bank_balance_minor: live.balance_minor,
                checked_at: None,
            },
        )?)
    })
}

/// Books bank minus Saldo as one entry dated now, then records a check that matches it.
pub(crate) fn book_unexplained_difference_inner(
    conn: &Connection,
    input: BookUnexplainedDifferenceInput,
) -> Result<BookedDifference, AppError> {
    if input.bank_balance_minor.abs() > MAX_AMOUNT_MINOR {
        return Err(validation(
            "The bank balance is larger than Saldo can store.",
        ));
    }
    with_savepoint_app(conn, || {
        let live = live_balance_as_seen(conn, input.account_id, input.seen_balance_minor)?;
        let amount_minor = input.bank_balance_minor - live.balance_minor;
        if amount_minor == 0 {
            return Err(AppError {
                code: "NOTHING_TO_BOOK".into(),
                message: "Saldo already matches the bank, so there is no difference to book."
                    .into(),
            });
        }
        let event_id = repository::insert_unexplained_difference(
            conn,
            InsertUnexplainedDifferenceParams {
                account_id: input.account_id,
                amount_minor,
                event_date: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
            },
        )?;
        let check_id = repository::insert_check(
            conn,
            InsertCheckParams {
                account_id: input.account_id,
                checked_on: live.checked_on,
                bank_balance_minor: input.bank_balance_minor,
                checked_at: None,
            },
        )?;
        // A balance update dated after now sets the balance, so an entry booked now is left out.
        if !matches!(
            verification_of(conn, input.account_id)?,
            Some(Verification::Verified { check_id: verified, .. }) if verified == check_id
        ) {
            return Err(validation(
                "A balance update dated after now sets this account's balance, so an entry \
                 booked now would not change it. Move that balance update first.",
            ));
        }
        Ok(BookedDifference { event_id, check_id })
    })
}

pub(crate) fn delete_balance_check_inner(conn: &Connection, check_id: i64) -> Result<(), AppError> {
    if repository::delete_check(conn, check_id)? {
        Ok(())
    } else {
        Err(AppError {
            code: "NOT_FOUND".into(),
            message: "This check no longer exists.".into(),
        })
    }
}

struct LiveBalance {
    checked_on: String,
    balance_minor: i64,
}

/// Saldo's live balance now, read inside the write, refused when it is not the one the user
/// looked at (an import or `saldo mcp` wrote since).
fn live_balance_as_seen(
    conn: &Connection,
    account_id: i64,
    seen_balance_minor: i64,
) -> Result<LiveBalance, AppError> {
    match get_account_type(conn, account_id)?.as_deref() {
        Some("account") => {}
        Some(_) => {
            return Err(validation(
                "Only an own account can be checked against the bank.",
            ))
        }
        None => {
            return Err(AppError {
                code: "NOT_FOUND".into(),
                message: "Account not found".into(),
            })
        }
    }
    let checked_on = local_now();
    let balance_minor = repository::saldo_side(
        conn,
        SaldoSideParams {
            account_id,
            checked_at: repository::clock_now(conn)?,
            checked_on: checked_on.clone(),
        },
    )?;
    if balance_minor != seen_balance_minor {
        return Err(AppError {
            code: "CHECK_CHANGED".into(),
            message: "The balance changed since you looked. Look at it again.".into(),
        });
    }
    Ok(LiveBalance {
        checked_on,
        balance_minor,
    })
}

fn validation(message: &str) -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message: message.into(),
    }
}
