use std::cmp::Reverse;
use std::collections::HashMap;

use chrono::NaiveDate;

const NEAR_DAYS: i64 = 5;
/// A run of 3 or more spaces marks a card hold's text in a bank export.
const HOLD_PADDING: &str = "   ";

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
    pub card: Option<String>,
    pub pending: bool,
}

impl IncomingRow {
    fn key(&self) -> (NaiveDate, i64) {
        (self.day, self.amount_minor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BankState {
    Booked,
    Pending {
        seen_to: NaiveDate,
    },
    Posted {
        held_day: NaiveDate,
        held_amount_minor: i64,
    },
}

pub(crate) fn posting_change_minor(total_before_minor: i64, total_after_minor: i64) -> i64 {
    total_after_minor - total_before_minor
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct BankFacts {
    pub day: NaiveDate,
    pub amount_minor: i64,
    pub text: Option<String>,
    pub card: Option<String>,
    pub state: BankState,
}

#[derive(Debug, Clone, Hash)]
pub(crate) struct Recorded {
    pub bank_entry_event_id: i64,
    pub latest_data_id: i64,
    pub split_group_id: Option<i64>,
    pub event_date: String,
    pub day: NaiveDate,
    pub total_minor: i64,
    pub own_note: Option<String>,
    pub bank: Option<BankFacts>,
}

impl Recorded {
    fn key(&self) -> (NaiveDate, i64) {
        match &self.bank {
            Some(bank) => (bank.day, bank.amount_minor),
            None => (self.day, self.total_minor),
        }
    }

    fn key_while_held(&self) -> Option<(NaiveDate, i64)> {
        match self.state()? {
            BankState::Posted {
                held_day,
                held_amount_minor,
            } => Some((held_day, held_amount_minor)),
            BankState::Booked | BankState::Pending { .. } => None,
        }
    }

    fn card(&self) -> Option<&str> {
        self.bank.as_ref()?.card.as_deref()
    }

    fn state(&self) -> Option<BankState> {
        Some(self.bank.as_ref()?.state)
    }

    fn is_pending_hold(&self) -> bool {
        matches!(self.state(), Some(BankState::Pending { .. }))
    }

    pub(crate) fn note_once_booked(&self, booked_text: Option<&str>) -> Option<String> {
        let padded = self
            .own_note
            .as_deref()
            .is_some_and(|note| note.contains(HOLD_PADDING));
        let the_banks = self
            .bank
            .as_ref()
            .is_some_and(|bank| bank.text == self.own_note);
        if padded || the_banks {
            booked_text.map(str::to_owned)
        } else {
            self.own_note.clone()
        }
    }
}

fn cards_fit(row: &IncomingRow, recorded: &Recorded) -> bool {
    match (row.card.as_deref(), recorded.card()) {
        (Some(row_card), Some(recorded_card)) => row_card == recorded_card,
        _ => true,
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
    HoldPosted(&'a Recorded),
    HoldPostedAtOtherAmount(&'a Recorded),
}

pub(crate) struct Matched<'a> {
    pub rows: Vec<Match<'a>>,
    pub cancelled_holds: Vec<&'a Recorded>,
}

pub(crate) fn match_rows<'a>(
    rows: &[IncomingRow],
    items: &'a [LedgerItem],
    cashflows: &'a [CounterpartCashflow],
) -> Matched<'a> {
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
    claim_holds_booked_on_another_day(rows, &file_order, items, &mut used, &mut matches);
    claim_holds_booked_at_another_amount(rows, items, &mut used, &mut matches);
    claim_mutually_unique_cashflows(rows, cashflows, &mut matches);
    claim_posted_holds_listed_again(rows, &file_order, items, &mut used, &mut matches);
    claim_entries_a_newer_export_booked_later(rows, &file_order, items, &mut used, &mut matches);
    Matched {
        cancelled_holds: cancelled_holds(rows, items, &used),
        rows: matches,
    }
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
    for allowed in [SameDayPreference::SameState, SameDayPreference::Unseen] {
        for &position in file_order {
            if !matches!(matches[position], Match::New) {
                continue;
            }
            let row = &rows[position];
            let Some(candidates) = by_key.get(&row.key()) else {
                continue;
            };
            let taken = candidates
                .iter()
                .copied()
                .map(|item| (item, same_day_preference(row, &items[item].recorded)))
                .filter(|&(item, preference)| {
                    !used[item]
                        && preference <= allowed
                        && row.counterpart.may_claim(&items[item])
                        && cards_fit(row, &items[item].recorded)
                })
                .min_by_key(|&(item, preference)| {
                    (preference, items[item].recorded.bank_entry_event_id)
                });
            if let Some((item, _)) = taken {
                used[item] = true;
                let recorded = &items[item].recorded;
                matches[position] = if !row.pending && recorded.is_pending_hold() {
                    Match::HoldPosted(recorded)
                } else {
                    Match::InSaldo(recorded)
                };
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SameDayPreference {
    SameState,
    OtherState,
    Unseen,
}

fn same_day_preference(row: &IncomingRow, recorded: &Recorded) -> SameDayPreference {
    match recorded.state() {
        Some(BankState::Pending { .. }) if row.pending => SameDayPreference::SameState,
        Some(BankState::Booked | BankState::Posted { .. }) if !row.pending => {
            SameDayPreference::SameState
        }
        Some(_) => SameDayPreference::OtherState,
        None => SameDayPreference::Unseen,
    }
}

fn claim_holds_booked_on_another_day<'a>(
    rows: &[IncomingRow],
    file_order: &[usize],
    items: &'a [LedgerItem],
    used: &mut [bool],
    matches: &mut [Match<'a>],
) {
    for &position in file_order {
        let row = &rows[position];
        if row.pending || !matches!(matches[position], Match::New) {
            continue;
        }
        let claimed = claim_closest(row, items, used, |recorded| {
            let (hold_day, hold_amount_minor) = recorded.key();
            recorded.is_pending_hold()
                && hold_amount_minor == row.amount_minor
                && booked_soon_after(hold_day, row.day)
        });
        if let Some(hold) = claimed {
            matches[position] = Match::HoldPosted(hold);
        }
    }
}

fn claim_posted_holds_listed_again<'a>(
    rows: &[IncomingRow],
    file_order: &[usize],
    items: &'a [LedgerItem],
    used: &mut [bool],
    matches: &mut [Match<'a>],
) {
    claim_for_pending_rows(rows, file_order, items, used, matches, |row, recorded| {
        recorded.key_while_held() == Some(row.key())
    });
}

fn claim_entries_a_newer_export_booked_later<'a>(
    rows: &[IncomingRow],
    file_order: &[usize],
    items: &'a [LedgerItem],
    used: &mut [bool],
    matches: &mut [Match<'a>],
) {
    claim_for_pending_rows(rows, file_order, items, used, matches, |row, recorded| {
        let (booked_day, booked_amount_minor) = recorded.key();
        recorded.state() == Some(BankState::Booked)
            && booked_amount_minor == row.amount_minor
            && booked_soon_after(row.day, booked_day)
    });
}

fn claim_for_pending_rows<'a>(
    rows: &[IncomingRow],
    file_order: &[usize],
    items: &'a [LedgerItem],
    used: &mut [bool],
    matches: &mut [Match<'a>],
    fits: impl Fn(&IncomingRow, &Recorded) -> bool,
) {
    for &position in file_order {
        let row = &rows[position];
        if !row.pending || !matches!(matches[position], Match::New) {
            continue;
        }
        if let Some(recorded) = claim_closest(row, items, used, |recorded| fits(row, recorded)) {
            matches[position] = Match::InSaldo(recorded);
        }
    }
}

fn claim_closest<'a>(
    row: &IncomingRow,
    items: &'a [LedgerItem],
    used: &mut [bool],
    fits: impl Fn(&Recorded) -> bool,
) -> Option<&'a Recorded> {
    let (item, recorded) = items
        .iter()
        .enumerate()
        .filter(|&(item, ledger_item)| {
            let recorded = &ledger_item.recorded;
            !used[item]
                && row.counterpart.may_claim(ledger_item)
                && cards_fit(row, recorded)
                && fits(recorded)
        })
        .map(|(item, ledger_item)| (item, &ledger_item.recorded))
        .min_by_key(|(_, recorded)| {
            (
                days_apart(row.day, recorded.key().0),
                recorded.bank_entry_event_id,
            )
        })?;
    used[item] = true;
    Some(recorded)
}

fn claim_holds_booked_at_another_amount<'a>(
    rows: &[IncomingRow],
    items: &'a [LedgerItem],
    used: &mut [bool],
    matches: &mut [Match<'a>],
) {
    let candidates_of = (0..rows.len())
        .filter(|&position| {
            let row = &rows[position];
            !row.pending && row.card.is_some() && matches!(matches[position], Match::New)
        })
        .map(|position| {
            let row = &rows[position];
            let holds = items
                .iter()
                .enumerate()
                .filter(|&(item, ledger_item)| {
                    let hold = &ledger_item.recorded;
                    let (day, amount_minor) = hold.key();
                    !used[item]
                        && hold.is_pending_hold()
                        && hold.split_group_id.is_none()
                        && row.counterpart.may_claim(ledger_item)
                        && hold.card() == row.card.as_deref()
                        && day == row.day
                        && amount_minor.signum() == row.amount_minor.signum()
                        && amount_minor != row.amount_minor
                })
                .map(|(item, _)| item)
                .collect();
            (position, holds)
        })
        .collect();
    for (position, item) in only_candidates_of_each_other(candidates_of, items.len()) {
        used[item] = true;
        matches[position] = Match::HoldPostedAtOtherAmount(&items[item].recorded);
    }
}

fn cancelled_holds<'a>(
    rows: &[IncomingRow],
    items: &'a [LedgerItem],
    used: &[bool],
) -> Vec<&'a Recorded> {
    let days = || rows.iter().map(|row| row.day);
    let (Some(oldest), Some(newest)) = (days().min(), days().max()) else {
        return Vec::new();
    };
    items
        .iter()
        .zip(used)
        .filter(|&(ledger_item, &used)| !used && ledger_item.transfer_leg.is_none())
        .map(|(ledger_item, _)| &ledger_item.recorded)
        .filter(|hold| match hold.state() {
            Some(BankState::Pending { seen_to }) => hold.key().0 >= oldest && newest > seen_to,
            _ => false,
        })
        .collect()
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
    for (position, cashflow) in only_candidates_of_each_other(candidates_of, cashflows.len()) {
        matches[position] = Match::CompletesTransfer(&cashflows[cashflow]);
    }
}

fn only_candidates_of_each_other(
    candidates_of: Vec<(usize, Vec<usize>)>,
    targets: usize,
) -> Vec<(usize, usize)> {
    let mut claimants = vec![0usize; targets];
    for (_, candidates) in &candidates_of {
        for &target in candidates {
            claimants[target] += 1;
        }
    }
    candidates_of
        .into_iter()
        .filter_map(|(position, candidates)| match candidates[..] {
            [target] if claimants[target] == 1 => Some((position, target)),
            _ => None,
        })
        .collect()
}

fn near_without_bank_entry(row: &IncomingRow, leg: &Recorded) -> bool {
    leg.bank.is_none()
        && leg.total_minor == row.amount_minor
        && days_apart(row.day, leg.day) <= NEAR_DAYS
}

fn days_apart(a: NaiveDate, b: NaiveDate) -> i64 {
    (a - b).num_days().abs()
}

fn booked_soon_after(held_day: NaiveDate, booked_day: NaiveDate) -> bool {
    (0..=NEAR_DAYS).contains(&(booked_day - held_day).num_days())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: &str = "433703******2103";
    const SECOND_CARD: &str = "433703******7708";
    const DEBIT_CARD: &str = "423473******0501";

    fn date(day: &str) -> NaiveDate {
        NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap()
    }

    fn row(index: u32, day: &str, amount_minor: i64) -> IncomingRow {
        IncomingRow {
            index,
            event_date: format!("{day}T12:00:00"),
            day: date(day),
            amount_minor,
            note: Some(format!("row {index}")),
            counterpart: Counterpart::None,
            original_currency_id: None,
            original_amount_minor: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
            card: None,
            pending: false,
        }
    }

    fn booked(card: &str, row: IncomingRow) -> IncomingRow {
        IncomingRow {
            card: Some(card.to_owned()),
            ..row
        }
    }

    fn pending(card: &str, row: IncomingRow) -> IncomingRow {
        IncomingRow {
            pending: true,
            ..booked(card, row)
        }
    }

    fn typed(id: i64, day: &str, amount_minor: i64) -> LedgerItem {
        LedgerItem {
            recorded: Recorded {
                bank_entry_event_id: id,
                latest_data_id: id,
                split_group_id: None,
                event_date: format!("{day}T12:00:00"),
                day: date(day),
                total_minor: amount_minor,
                own_note: None,
                bank: None,
            },
            transfer_leg: None,
        }
    }

    fn with_bank(
        id: i64,
        day: &str,
        amount_minor: i64,
        card: Option<&str>,
        state: BankState,
    ) -> LedgerItem {
        let mut item = typed(id, day, amount_minor);
        item.recorded.bank = Some(BankFacts {
            day: date(day),
            amount_minor,
            text: None,
            card: card.map(str::to_owned),
            state,
        });
        item
    }

    fn hold(id: i64, day: &str, amount_minor: i64, card: &str, seen_to: &str) -> LedgerItem {
        with_bank(
            id,
            day,
            amount_minor,
            Some(card),
            BankState::Pending {
                seen_to: date(seen_to),
            },
        )
    }

    fn booked_entry(id: i64, day: &str, amount_minor: i64, card: &str) -> LedgerItem {
        with_bank(id, day, amount_minor, Some(card), BankState::Booked)
    }

    fn posted(
        id: i64,
        (day, amount_minor): (&str, i64),
        card: &str,
        (held_day, held_amount_minor): (&str, i64),
    ) -> LedgerItem {
        with_bank(
            id,
            day,
            amount_minor,
            Some(card),
            BankState::Posted {
                held_day: date(held_day),
                held_amount_minor,
            },
        )
    }

    fn split(mut item: LedgerItem) -> LedgerItem {
        item.recorded.split_group_id = Some(item.recorded.bank_entry_event_id);
        item
    }

    fn outcome(rows: &[IncomingRow], items: &[LedgerItem]) -> (Vec<String>, Vec<i64>) {
        let matched = match_rows(rows, items, &[]);
        let verdicts = matched
            .rows
            .iter()
            .map(|matched| match matched {
                Match::New => "new".to_owned(),
                Match::InSaldo(recorded) => format!("in Saldo {}", recorded.bank_entry_event_id),
                Match::HoldPosted(hold) => format!("posts {}", hold.bank_entry_event_id),
                Match::HoldPostedAtOtherAmount(hold) => {
                    format!("posts {} at another amount", hold.bank_entry_event_id)
                }
                Match::TransferLeg { leg, .. } => format!("leg {}", leg.bank_entry_event_id),
                Match::CompletesTransfer(cashflow) => {
                    format!("completes {}", cashflow.recorded.bank_entry_event_id)
                }
            })
            .collect();
        let cancelled = matched
            .cancelled_holds
            .iter()
            .map(|hold| hold.bank_entry_event_id)
            .collect();
        (verdicts, cancelled)
    }

    fn verdicts(rows: &[IncomingRow], items: &[LedgerItem]) -> Vec<String> {
        outcome(rows, items).0
    }

    #[test]
    fn a_booked_row_posts_the_hold_with_its_card_day_and_amount() {
        let items = [hold(1, "2026-10-25", -1_999, CARD, "2026-10-25")];

        assert_eq!(
            verdicts(&[booked(CARD, row(0, "2026-10-25", -1_999))], &items),
            ["posts 1"]
        );
    }

    #[test]
    fn two_tickets_of_one_day_on_two_cards_post_their_own_holds() {
        let items = [
            hold(1, "2026-10-20", -50, CARD, "2026-10-25"),
            hold(2, "2026-10-20", -50, SECOND_CARD, "2026-10-25"),
        ];
        let rows = [
            booked(SECOND_CARD, row(0, "2026-10-20", -50)),
            booked(CARD, row(1, "2026-10-20", -50)),
        ];

        assert_eq!(verdicts(&rows, &items), ["posts 2", "posts 1"]);
    }

    #[test]
    fn a_hold_on_another_card_is_never_posted_and_a_covering_export_cancels_it() {
        let items = [hold(1, "2026-10-20", -50, CARD, "2026-10-25")];
        let rows = [
            booked(SECOND_CARD, row(0, "2026-10-20", -50)),
            booked(SECOND_CARD, row(1, "2026-10-31", -1_000)),
        ];

        assert_eq!(outcome(&rows, &items), (vec!["new".into(); 2], vec![1]));
    }

    #[test]
    fn a_row_with_no_card_posts_a_hold_with_one() {
        let items = [hold(1, "2026-10-20", -50, CARD, "2026-10-25")];

        assert_eq!(verdicts(&[row(0, "2026-10-20", -50)], &items), ["posts 1"]);
    }

    #[test]
    fn a_hold_booked_up_to_five_days_later_posts_closest_first() {
        let items = [
            hold(1, "2026-09-10", -7_896, CARD, "2026-09-16"),
            hold(2, "2026-09-14", -7_896, CARD, "2026-09-16"),
        ];

        assert_eq!(
            verdicts(&[booked(CARD, row(0, "2026-09-15", -7_896))], &items),
            ["posts 2"]
        );
        assert_eq!(
            verdicts(&[booked(CARD, row(0, "2026-09-20", -7_896))], &items[..1]),
            ["new"],
            "six days after the hold"
        );
    }

    #[test]
    fn a_booked_row_never_posts_a_hold_dated_after_it() {
        let items = [
            hold(1, "2026-09-16", -7_896, CARD, "2026-09-16"),
            hold(2, "2026-09-13", -7_896, CARD, "2026-09-16"),
        ];
        let tchibo = || booked(CARD, row(0, "2026-09-15", -7_896));

        assert_eq!(verdicts(&[tchibo()], &items), ["posts 2"]);
        assert_eq!(verdicts(&[tchibo()], &items[..1]), ["new"]);
    }

    #[test]
    fn a_same_day_entry_wins_over_a_hold_of_another_day() {
        let items = [
            hold(1, "2026-09-14", -7_896, CARD, "2026-09-16"),
            booked_entry(2, "2026-09-15", -7_896, CARD),
        ];

        assert_eq!(
            verdicts(&[booked(CARD, row(0, "2026-09-15", -7_896))], &items),
            ["in Saldo 2"]
        );
    }

    #[test]
    fn a_row_takes_an_entry_in_its_own_state_first() {
        let items = [
            typed(1, "2026-10-20", -50),
            hold(2, "2026-10-20", -50, CARD, "2026-10-25"),
            booked_entry(3, "2026-10-20", -50, CARD),
        ];

        assert_eq!(
            verdicts(&[pending(CARD, row(0, "2026-10-20", -50))], &items),
            ["in Saldo 2"]
        );
        assert_eq!(
            verdicts(&[booked(CARD, row(0, "2026-10-20", -50))], &items),
            ["in Saldo 3"]
        );
        assert_eq!(
            verdicts(
                &[
                    booked(CARD, row(0, "2026-10-20", -50)),
                    booked(CARD, row(1, "2026-10-20", -50)),
                    booked(CARD, row(2, "2026-10-20", -50)),
                ],
                &items
            ),
            ["in Saldo 3", "posts 2", "in Saldo 1"]
        );
    }

    #[test]
    fn a_row_in_the_other_state_waits_until_every_row_took_an_entry_in_its_own_state() {
        let ticket_of_24_oct = hold(1, "2026-10-24", -50, CARD, "2026-10-24");
        let ticket_of_23_oct = hold(2, "2026-10-23", -50, CARD, "2026-10-24");
        let ticket_posted = posted(1, ("2026-11-19", -50), CARD, ("2026-11-18", -50));

        assert_eq!(
            verdicts(
                &[
                    booked(CARD, row(0, "2026-10-24", -50)),
                    pending(CARD, row(1, "2026-10-24", -50)),
                ],
                &[ticket_of_24_oct, ticket_of_23_oct]
            ),
            ["posts 2", "in Saldo 1"],
            "the 23 Oct ticket booked on the 24th"
        );
        assert_eq!(
            verdicts(
                &[
                    pending(CARD, row(0, "2026-11-19", -50)),
                    booked(CARD, row(1, "2026-11-19", -50)),
                ],
                &[ticket_posted]
            ),
            ["new", "in Saldo 1"],
            "the next ticket still pending"
        );
    }

    #[test]
    fn a_pending_row_imported_again_is_its_own_hold() {
        let items = [hold(1, "2026-10-31", -1_200, DEBIT_CARD, "2026-11-01")];

        assert_eq!(
            outcome(&[pending(DEBIT_CARD, row(0, "2026-10-31", -1_200))], &items),
            (vec!["in Saldo 1".to_owned()], vec![])
        );
    }

    #[test]
    fn a_hold_booked_at_another_amount_on_its_day_and_card_is_repriced() {
        let items = [hold(1, "2026-04-30", -3_300, DEBIT_CARD, "2026-05-01")];

        assert_eq!(
            verdicts(&[booked(DEBIT_CARD, row(0, "2026-04-30", -3_299))], &items),
            ["posts 1 at another amount"]
        );
    }

    #[test]
    fn a_hold_at_another_amount_is_refused_unless_each_is_the_others_only_candidate() {
        let github = || booked(DEBIT_CARD, row(0, "2026-04-30", -3_299));
        let two_holds = [
            hold(1, "2026-04-30", -3_300, DEBIT_CARD, "2026-05-01"),
            hold(2, "2026-04-30", -3_400, DEBIT_CARD, "2026-05-01"),
        ];
        let one_hold = [hold(1, "2026-04-30", -3_300, DEBIT_CARD, "2026-05-01")];
        let two_rows = [github(), booked(DEBIT_CARD, row(1, "2026-04-30", -3_250))];

        assert_eq!(verdicts(&[github()], &two_holds), ["new"]);
        assert_eq!(verdicts(&two_rows, &one_hold), ["new", "new"]);
    }

    #[test]
    fn a_hold_at_another_amount_needs_the_same_known_card_day_and_sign() {
        let items = [hold(1, "2026-04-30", -3_300, DEBIT_CARD, "2026-05-01")];
        let refused = [
            ("no card", row(0, "2026-04-30", -3_299)),
            ("another card", booked(CARD, row(0, "2026-04-30", -3_299))),
            (
                "another day",
                booked(DEBIT_CARD, row(0, "2026-05-01", -3_299)),
            ),
            ("a refund", booked(DEBIT_CARD, row(0, "2026-04-30", 3_299))),
            (
                "a pending row",
                pending(DEBIT_CARD, row(0, "2026-04-30", -3_299)),
            ),
        ];

        for (case, row) in refused {
            assert_eq!(verdicts(&[row], &items), ["new"], "{case}");
        }
    }

    #[test]
    fn a_split_hold_posts_at_its_amount_but_is_never_repriced() {
        let items = [split(hold(
            1,
            "2026-04-30",
            -3_300,
            DEBIT_CARD,
            "2026-04-30",
        ))];
        let later = booked(DEBIT_CARD, row(1, "2026-05-02", -500));

        assert_eq!(
            verdicts(&[booked(DEBIT_CARD, row(0, "2026-04-30", -3_300))], &items),
            ["posts 1"]
        );
        assert_eq!(
            outcome(
                &[booked(DEBIT_CARD, row(0, "2026-04-30", -3_299)), later],
                &items
            ),
            (vec!["new".to_owned(), "new".to_owned()], vec![1]),
            "the newer export no longer lists the hold"
        );
    }

    #[test]
    fn an_older_export_imported_after_the_posting_finds_the_hold_by_its_own_day_and_amount() {
        let tchibo = posted(1, ("2026-09-15", -7_896), CARD, ("2026-09-14", -7_896));
        let github = posted(
            2,
            ("2026-04-30", -3_299),
            DEBIT_CARD,
            ("2026-04-30", -3_300),
        );
        let held_again = |card, day, amount_minor, posted: &LedgerItem| {
            verdicts(
                &[pending(card, row(0, day, amount_minor))],
                &[posted.clone()],
            )
        };

        assert_eq!(
            held_again(CARD, "2026-09-14", -7_896, &tchibo),
            ["in Saldo 1"]
        );
        assert_eq!(
            held_again(DEBIT_CARD, "2026-04-30", -3_300, &github),
            ["in Saldo 2"]
        );
        assert_eq!(
            held_again(CARD, "2026-09-13", -7_896, &tchibo),
            ["new"],
            "a hold of another day is another purchase"
        );
        assert_eq!(
            held_again(DEBIT_CARD, "2026-04-30", -3_400, &github),
            ["new"],
            "a hold of another amount is another purchase"
        );
        assert_eq!(
            held_again(SECOND_CARD, "2026-09-14", -7_896, &tchibo),
            ["new"],
            "a hold on another card is another purchase"
        );
    }

    #[test]
    fn a_pending_row_of_an_older_export_finds_the_entry_booked_up_to_five_days_later() {
        let booked_on = |day| booked_entry(1, day, -7_896, CARD);
        let held_on_24_oct = || pending(CARD, row(0, "2026-10-24", -7_896));

        for (case, day) in [
            ("a day later", "2026-10-25"),
            ("5 days later", "2026-10-29"),
        ] {
            assert_eq!(
                verdicts(&[held_on_24_oct()], &[booked_on(day)]),
                ["in Saldo 1"],
                "{case}"
            );
        }
        for (case, day) in [
            ("6 days later", "2026-10-30"),
            ("a day before", "2026-10-23"),
        ] {
            assert_eq!(
                verdicts(&[held_on_24_oct()], &[booked_on(day)]),
                ["new"],
                "{case}"
            );
        }
        assert_eq!(
            verdicts(
                &[held_on_24_oct()],
                &[
                    booked_entry(1, "2026-10-27", -7_896, CARD),
                    booked_entry(2, "2026-10-25", -7_896, CARD),
                ]
            ),
            ["in Saldo 2"],
            "the closest day first"
        );
        assert_eq!(
            verdicts(
                &[pending(SECOND_CARD, row(0, "2026-10-24", -7_896))],
                &[booked_on("2026-10-25")]
            ),
            ["new"],
            "another card"
        );
        assert_eq!(
            verdicts(&[held_on_24_oct()], &[typed(1, "2026-10-25", -7_896)]),
            ["new"],
            "an entry no import has seen"
        );
        assert_eq!(
            verdicts(
                &[held_on_24_oct(), booked(CARD, row(1, "2026-10-25", -7_896))],
                &[booked_on("2026-10-25")]
            ),
            ["new", "in Saldo 1"],
            "the file's own booked row claims first"
        );
    }

    #[test]
    fn only_a_newer_export_covering_the_holds_day_cancels_it() {
        let items = [hold(1, "2026-10-25", -1_999, CARD, "2026-10-25")];
        let other = |index, day| row(index, day, -500);

        let cancelled = |rows: &[IncomingRow]| outcome(rows, &items).1;
        assert_eq!(
            cancelled(&[other(0, "2026-10-20"), other(1, "2026-10-31")]),
            [1]
        );
        assert_eq!(
            cancelled(&[other(0, "2026-10-25"), other(1, "2026-10-31")]),
            [1],
            "a file starting on the hold's day covers it"
        );
        assert_eq!(
            cancelled(&[other(0, "2026-10-20"), other(1, "2026-10-25")]),
            Vec::<i64>::new(),
            "no newer than the export that listed it"
        );
        assert_eq!(
            cancelled(&[other(0, "2026-10-26"), other(1, "2026-10-31")]),
            Vec::<i64>::new(),
            "starts after the hold's day"
        );
        assert_eq!(cancelled(&[]), Vec::<i64>::new());
    }

    #[test]
    fn a_hold_the_file_still_lists_or_posts_is_not_cancelled() {
        let items = [
            hold(1, "2026-10-25", -1_999, CARD, "2026-10-25"),
            hold(2, "2026-10-26", -2_500, CARD, "2026-10-26"),
        ];
        let rows = [
            booked(CARD, row(0, "2026-10-25", -1_999)),
            pending(CARD, row(1, "2026-10-26", -2_500)),
            booked(CARD, row(2, "2026-10-31", -500)),
        ];

        assert_eq!(
            outcome(&rows, &items),
            (
                vec!["posts 1".into(), "in Saldo 2".into(), "new".into()],
                vec![]
            )
        );
    }

    #[test]
    fn a_pending_transfer_leg_is_never_cancelled() {
        let mut leg = hold(1, "2026-10-25", -1_999, CARD, "2026-10-25");
        leg.transfer_leg = Some(TransferLeg {
            other_account_id: 9,
            other_account_name: "Savings".to_owned(),
            other_account_has_iban: true,
            created_at: "2026-10-25T10:00:00.000".to_owned(),
        });

        assert_eq!(
            outcome(&[booked(CARD, row(0, "2026-10-31", -500))], &[leg]).1,
            Vec::<i64>::new()
        );
    }

    #[test]
    fn a_note_follows_the_booked_text_only_while_it_is_the_banks() {
        let with_note = |note: Option<&str>, bank_text: Option<&str>| {
            let mut item = hold(1, "2026-10-25", -1_999, CARD, "2026-10-25");
            item.recorded.own_note = note.map(str::to_owned);
            item.recorded.bank.as_mut().unwrap().text = bank_text.map(str::to_owned);
            item.recorded
        };
        let hold_text = "BUDE DOPLNENÝ PO ZÚČT.";
        let booked_text = Some("TCHIBO PRAHA");

        assert_eq!(
            with_note(Some(hold_text), Some(hold_text)).note_once_booked(booked_text),
            booked_text.map(str::to_owned)
        );
        assert_eq!(
            with_note(Some("TCHIBO    "), None).note_once_booked(booked_text),
            booked_text.map(str::to_owned),
            "a hold's padding"
        );
        assert_eq!(
            with_note(Some("coffee for the office"), Some(hold_text)).note_once_booked(booked_text),
            Some("coffee for the office".to_owned())
        );
        assert_eq!(
            typed(1, "2026-10-25", -1_999)
                .recorded
                .note_once_booked(booked_text),
            None,
            "an entry no import has seen keeps no note"
        );
    }
}
