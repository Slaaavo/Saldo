use std::cmp::Reverse;
use std::collections::HashMap;

use chrono::NaiveDate;

const NEAR_DAYS: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Counterpart {
    None,
    Unknown,
    Partner(i64),
    Own(i64),
}

impl Counterpart {
    pub(crate) fn account_id(self) -> Option<i64> {
        match self {
            Counterpart::None | Counterpart::Unknown => None,
            Counterpart::Partner(id) | Counterpart::Own(id) => Some(id),
        }
    }

    fn may_claim(self, item: &LedgerItem) -> bool {
        item.transfer_leg
            .as_ref()
            .is_none_or(|leg| self.may_claim_leg(leg))
    }

    fn may_claim_leg(self, leg: &TransferLeg) -> bool {
        match self {
            Counterpart::None => true,
            Counterpart::Own(account_id) => account_id == leg.other_account_id,
            Counterpart::Unknown => !leg.other_account_has_iban,
            Counterpart::Partner(_) => false,
        }
    }
}

#[derive(Debug, Clone, Hash)]
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

impl IncomingRow {
    fn key(&self) -> (NaiveDate, i64) {
        (self.day, self.amount_minor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct BankFacts {
    pub day: NaiveDate,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Hash)]
pub(crate) struct Recorded {
    pub bank_entry_event_id: i64,
    pub latest_data_id: i64,
    pub event_date: String,
    pub day: NaiveDate,
    pub amount_minor: i64,
    pub note: Option<String>,
    pub bank: Option<BankFacts>,
}

impl Recorded {
    fn key(&self) -> (NaiveDate, i64) {
        match self.bank {
            Some(bank) => (bank.day, bank.amount_minor),
            None => (self.day, self.amount_minor),
        }
    }
}

#[derive(Debug, Clone, Hash)]
pub(crate) struct TransferLeg {
    pub other_account_id: i64,
    pub other_account_name: String,
    pub other_account_has_iban: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Hash)]
pub(crate) struct LedgerItem {
    pub recorded: Recorded,
    pub transfer_leg: Option<TransferLeg>,
}

#[derive(Debug, Clone, Hash)]
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

pub(crate) fn match_rows<'a>(
    rows: &[IncomingRow],
    items: &'a [LedgerItem],
    cashflows: &'a [CounterpartCashflow],
) -> Vec<Match<'a>> {
    let mut file_order: Vec<usize> = (0..rows.len()).collect();
    file_order.sort_by_key(|&position| rows[position].index);
    let mut matches = vec![Match::New; rows.len()];
    let mut used = vec![false; items.len()];
    claim_legs_to_named_accounts(rows, &file_order, items, &mut used, &mut matches);
    claim_same_day_and_amount(rows, &file_order, items, &mut used, &mut matches);
    claim_near_legs_for_rows_naming_no_known_account(
        rows,
        &file_order,
        items,
        &mut used,
        &mut matches,
    );
    claim_mutually_unique_cashflows(rows, cashflows, &mut matches);
    matches
}

fn claim_legs_to_named_accounts<'a>(
    rows: &[IncomingRow],
    file_order: &[usize],
    items: &'a [LedgerItem],
    used: &mut [bool],
    matches: &mut [Match<'a>],
) {
    for &position in file_order {
        let row = &rows[position];
        let Counterpart::Own(account_id) = row.counterpart else {
            continue;
        };
        let claimed = claim_closest_leg(row, items, used, |recorded, leg| {
            leg.other_account_id == account_id
                && (recorded.key() == row.key() || near_without_bank_entry(row, recorded))
        });
        if let Some(matched) = claimed {
            matches[position] = matched;
        }
    }
}

fn claim_same_day_and_amount<'a>(
    rows: &[IncomingRow],
    file_order: &[usize],
    items: &'a [LedgerItem],
    used: &mut [bool],
    matches: &mut [Match<'a>],
) {
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
            (recorded.bank.is_none(), recorded.bank_entry_event_id)
        });
    }
    for &position in file_order {
        if !matches!(matches[position], Match::New) {
            continue;
        }
        let row = &rows[position];
        let Some(candidates) = by_key.get(&row.key()) else {
            continue;
        };
        if let Some(&item) = candidates
            .iter()
            .find(|&&item| !used[item] && row.counterpart.may_claim(&items[item]))
        {
            used[item] = true;
            matches[position] = Match::InSaldo(&items[item].recorded);
        }
    }
}

fn claim_near_legs_for_rows_naming_no_known_account<'a>(
    rows: &[IncomingRow],
    file_order: &[usize],
    items: &'a [LedgerItem],
    used: &mut [bool],
    matches: &mut [Match<'a>],
) {
    for &position in file_order {
        let row = &rows[position];
        let names_no_known_account =
            matches!(row.counterpart, Counterpart::None | Counterpart::Unknown);
        if !names_no_known_account || !matches!(matches[position], Match::New) {
            continue;
        }
        let claimed = claim_closest_leg(row, items, used, |recorded, leg| {
            row.counterpart.may_claim_leg(leg) && near_without_bank_entry(row, recorded)
        });
        if let Some(matched) = claimed {
            matches[position] = matched;
        }
    }
}

fn claim_closest_leg<'a>(
    row: &IncomingRow,
    items: &'a [LedgerItem],
    used: &mut [bool],
    fits: impl Fn(&Recorded, &TransferLeg) -> bool,
) -> Option<Match<'a>> {
    let (item, recorded, leg) = items
        .iter()
        .enumerate()
        .filter(|&(item, _)| !used[item])
        .filter_map(|(item, ledger_item)| {
            let leg = ledger_item.transfer_leg.as_ref()?;
            fits(&ledger_item.recorded, leg).then_some((item, &ledger_item.recorded, leg))
        })
        .min_by_key(|(_, recorded, leg)| {
            let key = recorded.key();
            (
                key != row.key(),
                days_apart(row.day, key.0),
                Reverse(leg.created_at.as_str()),
                recorded.bank_entry_event_id,
            )
        })?;
    used[item] = true;
    Some(if recorded.key() == row.key() {
        Match::InSaldo(recorded)
    } else {
        Match::TransferLeg {
            leg: recorded,
            other_account_name: &leg.other_account_name,
        }
    })
}

fn claim_mutually_unique_cashflows<'a>(
    rows: &[IncomingRow],
    cashflows: &'a [CounterpartCashflow],
    matches: &mut [Match<'a>],
) {
    let candidates_of: Vec<(usize, Vec<usize>)> = (0..rows.len())
        .filter(|&position| matches!(matches[position], Match::New))
        .filter_map(|position| {
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
}

fn near_without_bank_entry(row: &IncomingRow, leg: &Recorded) -> bool {
    leg.bank.is_none()
        && leg.amount_minor == row.amount_minor
        && days_apart(row.day, leg.day) <= NEAR_DAYS
}

fn days_apart(a: NaiveDate, b: NaiveDate) -> i64 {
    (a - b).num_days().abs()
}
