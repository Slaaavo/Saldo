use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BucketLink {
    pub id: i64,
    pub event_id: i64,
    pub source_account_id: i64,
    pub source_account_name: String,
    pub source_currency_id: i64,
    pub source_currency_code: String,
    pub source_currency_minor_units: i64,
}

/// Fixed once the bucket is created. An income budget's value is computed from its typed months,
/// so it owns no entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BucketKind {
    #[default]
    Balance,
    IncomeBudget,
}

impl BucketKind {
    pub const ALL: [BucketKind; 2] = [BucketKind::Balance, BucketKind::IncomeBudget];

    pub const fn as_sql(self) -> &'static str {
        match self {
            BucketKind::Balance => "balance",
            BucketKind::IncomeBudget => "income_budget",
        }
    }
}

impl ToSql for BucketKind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_sql().into())
    }
}

impl FromSql for BucketKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        BucketKind::ALL
            .into_iter()
            .find(|kind| kind.as_sql() == text)
            .ok_or_else(|| FromSqlError::Other(format!("not a bucket kind: {text}").into()))
    }
}

/// Only bucket rows carry one. `enabled` is today's switch, also for a past moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketState {
    pub kind: BucketKind,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinkConflict {
    pub source_account_id: i64,
    pub source_account_name: String,
    pub conflict_date: String,
    pub other_bucket_id: i64,
    pub other_bucket_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::ts_vocabulary::{serde_names, ts_string_list};

    #[test]
    fn bucket_kind_vocabulary_matches_typescript() {
        let types_ts = include_str!("../../../../src/shared/types/index.ts");

        assert_eq!(
            ts_string_list(types_ts, "BUCKET_KINDS"),
            serde_names(&BucketKind::ALL)
        );
    }
}
