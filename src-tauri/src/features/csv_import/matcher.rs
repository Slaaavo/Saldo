use std::cmp::Reverse;
use std::collections::HashMap;

use chrono::NaiveDate;

const NEAR_DAYS: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Counterpart {
    None,
    Partner(i64),
    Own(i64),
}

impl Counterpart {
    pub(crate) fn account_id(self) -> Option<i64> {
        match self {
            Counterpart::None => None,
            Counterpart::Partner(id) | Counterpart::Own(id) => Some(id),
        }
    }
}

/// A file row as the wizard parsed it.
#[derive(Debug, Clone)]
pub(crate) struct IncomingRow {
    pub index: u32,
    pub event_date: String,
    pub day: NaiveDate,
    pub amount_minor: i64,
    pub note: Option<String>,
    pub counterpart: Counterpart,
    pub original_currency_id: Option<i64>,
    pub original_amount_minor: Option<i64>,
    pub fx_rate_mantissa: Option<i64>,
    pub fx_rate_exponent: Option<i64>,
}

/// What the bank's own export said about an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BankFacts {
    pub day: NaiveDate,
    pub amount_minor: i64,
}

/// An entry in Saldo as it stands now. For a split group, the event is its lowest-id live leg,
/// the amount the legs' sum and the note the group's own.
#[derive(Debug, Clone)]
pub(crate) struct Recorded {
    pub event_id: i64,
    pub latest_data_id: i64,
    pub event_date: String,
    pub day: NaiveDate,
    pub amount_minor: i64,
    pub note: Option<String>,
    pub bank: Option<BankFacts>,
}

impl Recorded {
    /// The bank's facts when the entry has them, else the ledger's.
    fn key(&self) -> (NaiveDate, i64) {
        match self.bank {
            Some(bank) => (bank.day, bank.amount_minor),
            None => (self.day, self.amount_minor),
        }
    }
}

/// What a single transfer leg adds to its entry.
#[derive(Debug, Clone)]
pub(crate) struct TransferLeg {
    pub other_account_id: i64,
    pub other_account_name: String,
    pub created_at: String,
}

/// A live cash flow or transfer leg on the account outside a split group, or a live split group
/// on the account.
#[derive(Debug, Clone)]
pub(crate) struct LedgerItem {
    pub recorded: Recorded,
    pub transfer_leg: Option<TransferLeg>,
}

/// A plain cash flow on an own account that a row names as its counterpart.
#[derive(Debug, Clone)]
pub(crate) struct CounterpartCashflow {
    pub recorded: Recorded,
    pub account_id: i64,
    pub account_name: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Match<'a> {
    New,
    InSaldo(&'a Recorded),
    TransferLeg {
        leg: &'a Recorded,
        other_account_name: &'a str,
    },
    CompletesTransfer(&'a CounterpartCashflow),
}

impl<'a> Match<'a> {
    /// The entry in Saldo the match names.
    pub(crate) fn recorded(self) -> Option<&'a Recorded> {
        match self {
            Match::New => None,
            Match::InSaldo(recorded) | Match::TransferLeg { leg: recorded, .. } => Some(recorded),
            Match::CompletesTransfer(cashflow) => Some(&cashflow.recorded),
        }
    }
}

/// One match per row, in the order of `rows`. Rows claim in file order, tier by tier, and each
/// item and cash flow is claimed once.
pub(crate) fn match_rows<'a>(
    rows: &[IncomingRow],
    items: &'a [LedgerItem],
    cashflows: &'a [CounterpartCashflow],
) -> Vec<Match<'a>> {
    let mut file_order: Vec<usize> = (0..rows.len()).collect();
    file_order.sort_by_key(|&position| rows[position].index);
    let mut matches = vec![Match::New; rows.len()];
    let mut used = vec![false; items.len()];

    let mut by_key: HashMap<(NaiveDate, i64), Vec<usize>> = HashMap::new();
    for (item, ledger_item) in items.iter().enumerate() {
        by_key
            .entry(ledger_item.recorded.key())
            .or_default()
            .push(item);
    }
    for candidates in by_key.values_mut() {
        candidates.sort_by_key(|&item| {
            let recorded = &items[item].recorded;
            (recorded.bank.is_none(), recorded.event_id)
        });
    }
    for &position in &file_order {
        let row = &rows[position];
        let Some(candidates) = by_key.get(&(row.day, row.amount_minor)) else {
            continue;
        };
        if let Some(&item) = candidates.iter().find(|&&item| !used[item]) {
            used[item] = true;
            matches[position] = Match::InSaldo(&items[item].recorded);
        }
    }

    for &position in &file_order {
        if !matches!(matches[position], Match::New) {
            continue;
        }
        let row = &rows[position];
        let leg = items
            .iter()
            .enumerate()
            .filter(|&(item, _)| !used[item])
            .filter_map(|(item, ledger_item)| {
                let leg = ledger_item.transfer_leg.as_ref()?;
                claims_leg(row, leg.other_account_id, &ledger_item.recorded).then_some((
                    item,
                    &ledger_item.recorded,
                    leg,
                ))
            })
            .min_by_key(|(_, recorded, leg)| {
                (
                    days_apart(row.day, recorded.day),
                    Reverse(leg.created_at.as_str()),
                    recorded.event_id,
                )
            });
        if let Some((item, recorded, leg)) = leg {
            used[item] = true;
            matches[position] = Match::TransferLeg {
                leg: recorded,
                other_account_name: &leg.other_account_name,
            };
        }
    }

    let candidates_of: Vec<(usize, Vec<usize>)> = file_order
        .iter()
        .filter(|&&position| matches!(matches[position], Match::New))
        .filter_map(|&position| {
            let row = &rows[position];
            let Counterpart::Own(account_id) = row.counterpart else {
                return None;
            };
            let other_side = row.amount_minor.checked_neg()?;
            let candidates = cashflows
                .iter()
                .enumerate()
                .filter(|(_, cashflow)| {
                    let (day, amount_minor) = cashflow.recorded.key();
                    cashflow.account_id == account_id
                        && amount_minor == other_side
                        && days_apart(row.day, day) <= NEAR_DAYS
                })
                .map(|(cashflow, _)| cashflow)
                .collect();
            Some((position, candidates))
        })
        .collect();
    let mut claimants = vec![0usize; cashflows.len()];
    for (_, candidates) in &candidates_of {
        for &cashflow in candidates {
            claimants[cashflow] += 1;
        }
    }
    for (position, candidates) in candidates_of {
        if let [cashflow] = candidates[..] {
            if claimants[cashflow] == 1 {
                matches[position] = Match::CompletesTransfer(&cashflows[cashflow]);
            }
        }
    }

    matches
}

/// A row claims a transfer leg the other account's import made: same amount, a near day, and
/// the row names no counterpart or the leg's other account. A partner never claims one.
fn claims_leg(row: &IncomingRow, other_account_id: i64, leg: &Recorded) -> bool {
    let counterpart_fits = match row.counterpart {
        Counterpart::None => true,
        Counterpart::Own(account_id) => account_id == other_account_id,
        Counterpart::Partner(_) => false,
    };
    counterpart_fits
        && leg.bank.is_none()
        && leg.amount_minor == row.amount_minor
        && days_apart(row.day, leg.day) <= NEAR_DAYS
}

fn days_apart(a: NaiveDate, b: NaiveDate) -> i64 {
    (a - b).num_days().abs()
}
