use rusqlite::Connection;

use super::models::Verification;
use super::repository::{self, BalanceSeenByCheckParams, StoredCheck};

fn bank_minus_saldo(
    conn: &Connection,
    account_id: i64,
    check: &StoredCheck,
) -> rusqlite::Result<i64> {
    let saldo_minor = repository::balance_seen_by_check(
        conn,
        BalanceSeenByCheckParams {
            account_id,
            checked_at: check.checked_at.clone(),
            day: check.day.clone(),
        },
    )?;
    Ok(check.bank_balance_minor - saldo_minor)
}

pub(crate) fn verification_of(
    conn: &Connection,
    account_id: i64,
) -> rusqlite::Result<Option<Verification>> {
    let checks = repository::newest_checks(conn, account_id)?;
    let Some((newest, older)) = checks.split_first() else {
        return Ok(None);
    };
    let difference_minor = bank_minus_saldo(conn, account_id, newest)?;
    if difference_minor == 0 {
        return Ok(Some(Verification::Verified {
            check_id: newest.id,
            day: newest.day.clone(),
        }));
    }
    let mut last_verified_on = None;
    for check in older {
        if bank_minus_saldo(conn, account_id, check)? == 0 {
            last_verified_on = Some(check.day.clone());
            break;
        }
    }
    Ok(Some(Verification::Differs {
        check_id: newest.id,
        day: newest.day.clone(),
        difference_minor,
        last_verified_on,
    }))
}
