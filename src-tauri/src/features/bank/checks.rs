use rusqlite::Connection;

use super::models::Verification;
use super::repository::{self, SaldoSideParams, StoredCheck};

/// The bank's balance minus Saldo's side of the check; zero when it matches.
fn difference_of(conn: &Connection, account_id: i64, check: &StoredCheck) -> rusqlite::Result<i64> {
    let saldo_minor = repository::saldo_side(
        conn,
        SaldoSideParams {
            account_id,
            checked_at: check.checked_at.clone(),
            checked_on: check.checked_on.clone(),
        },
    )?;
    Ok(check.bank_balance_minor - saldo_minor)
}

/// None when the account was never checked. Otherwise the newest check decides, and an older
/// check that still matches tells how far back Saldo is still verified.
pub(crate) fn verification_of(
    conn: &Connection,
    account_id: i64,
) -> rusqlite::Result<Option<Verification>> {
    let checks = repository::newest_checks(conn, account_id)?;
    let Some((newest, older)) = checks.split_first() else {
        return Ok(None);
    };
    let difference_minor = difference_of(conn, account_id, newest)?;
    if difference_minor == 0 {
        return Ok(Some(Verification::Verified {
            check_id: newest.id,
            checked_on: newest.checked_on.clone(),
        }));
    }
    let mut last_verified_on = None;
    for check in older {
        if difference_of(conn, account_id, check)? == 0 {
            last_verified_on = Some(check.checked_on.clone());
            break;
        }
    }
    Ok(Some(Verification::Differs {
        check_id: newest.id,
        checked_on: newest.checked_on.clone(),
        difference_minor,
        last_verified_on,
    }))
}
