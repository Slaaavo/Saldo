use serde::Serialize;

use super::calculation::{BudgetCell, MonthLine, YearMonth};
use crate::features::transactions::models::Valuation;

/// An account whose incoming cashflows count as the income budget's Received.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CountedAccount {
    pub id: i64,
    pub name: String,
}

/// What the dialog opens with. `day` and `moment_month` come from the dashboard's moment, so the
/// UI never decides which month has started.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomeBudget {
    pub day: String,
    pub moment_month: YearMonth,
    pub months: Vec<BudgetCell>,
    pub counted_accounts: Vec<CountedAccount>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomeBudgetPreview {
    pub value_minor: i64,
    pub valuation: Valuation,
    pub lines: Vec<MonthLine>,
}
