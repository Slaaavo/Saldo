use rusqlite::Connection;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use super::checks::verification_of;
use super::models::Verification;
use super::repository::{self, CheckedAccount};
use crate::error::AppError;
use crate::shared::run_in_savepoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakingChecks {
    Refuse,
    Allow,
}

impl From<bool> for BreakingChecks {
    fn from(allow: bool) -> Self {
        if allow {
            Self::Allow
        } else {
            Self::Refuse
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrokenCheck {
    pub account_id: i64,
    pub account_name: String,
    pub day: String,
    pub difference_minor: i64,
    pub currency_code: String,
    pub currency_minor_units: i64,
}

#[derive(Debug)]
pub enum WriteError {
    App(AppError),
    BreaksVerifiedChecks(Vec<BrokenCheck>),
}

impl From<AppError> for WriteError {
    fn from(error: AppError) -> Self {
        Self::App(error)
    }
}

impl From<rusqlite::Error> for WriteError {
    fn from(error: rusqlite::Error) -> Self {
        Self::App(error.into())
    }
}

impl Serialize for WriteError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::App(error) => error.serialize(serializer),
            Self::BreaksVerifiedChecks(checks) => {
                let mut refusal = serializer.serialize_struct("WriteError", 3)?;
                refusal.serialize_field("code", "BREAKS_VERIFIED_CHECKS")?;
                refusal.serialize_field(
                    "message",
                    "This change makes a verified bank check differ.",
                )?;
                refusal.serialize_field("checks", checks)?;
                refusal.end()
            }
        }
    }
}

pub(crate) fn guard_verified_checks<T>(
    conn: &Connection,
    breaking: BreakingChecks,
    write: impl FnOnce() -> Result<T, AppError>,
) -> Result<T, WriteError> {
    run_in_savepoint(conn, || {
        let verified = match breaking {
            BreakingChecks::Refuse => verified_accounts(conn)?,
            BreakingChecks::Allow => Vec::new(),
        };
        let written = write()?;
        let broken = broken_checks(conn, verified)?;
        if broken.is_empty() {
            Ok(written)
        } else {
            Err(WriteError::BreaksVerifiedChecks(broken))
        }
    })
}

fn verified_accounts(conn: &Connection) -> rusqlite::Result<Vec<CheckedAccount>> {
    let mut verified = Vec::new();
    for account in repository::checked_accounts(conn)? {
        if let Some(Verification::Verified { .. }) = verification_of(conn, account.id)? {
            verified.push(account);
        }
    }
    Ok(verified)
}

fn broken_checks(
    conn: &Connection,
    verified: Vec<CheckedAccount>,
) -> rusqlite::Result<Vec<BrokenCheck>> {
    let mut broken = Vec::new();
    for account in verified {
        if let Some(Verification::Differs {
            day,
            difference_minor,
            ..
        }) = verification_of(conn, account.id)?
        {
            broken.push(BrokenCheck {
                account_id: account.id,
                account_name: account.name,
                day,
                difference_minor,
                currency_code: account.currency_code,
                currency_minor_units: account.currency_minor_units,
            });
        }
    }
    Ok(broken)
}
