use rusqlite::Connection;

use super::models::Verification;
use super::repository::{self, BalanceSeenByCheckParams};
use crate::features::transactions::models::{AccountKind, SnapshotRow};

/// Sets each own account row's verification. Call it on a live snapshot only.
pub(crate) fn attach_verifications(
    conn: &Connection,
    rows: &mut [SnapshotRow],
) -> rusqlite::Result<()> {
    for row in rows
        .iter_mut()
        .filter(|row| row.account_type == AccountKind::Account)
    {
        row.verification = verification_of(conn, row.account_id)?;
    }
    Ok(())
}

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
            check_id: Some(newest.id),
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
