use rusqlite::Connection;

use super::models::Verification;
use super::repository::{self, BalanceSeenByCheckParams};

pub(crate) fn verification_of(
    conn: &Connection,
    account_id: i64,
) -> rusqlite::Result<Option<Verification>> {
    let Some(newest) = repository::newest_check(conn, account_id)? else {
        return Ok(None);
    };
    let saldo_minor = repository::balance_seen_by_check(
        conn,
        BalanceSeenByCheckParams {
            account_id,
            checked_at: newest.checked_at,
            day: newest.day.clone(),
        },
    )?;
    let difference_minor = newest.bank_balance_minor - saldo_minor;
    Ok(Some(if difference_minor == 0 {
        Verification::Verified {
            check_id: newest.id,
            day: newest.day,
        }
    } else {
        Verification::Differs {
            check_id: newest.id,
            day: newest.day,
            difference_minor,
        }
    }))
}
