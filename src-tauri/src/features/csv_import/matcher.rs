use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;

use super::assignment::{cheapest_pairs, Edge};

const NEAR_DAYS: i64 = 5;
const CARD_HOLD_TEXT_CHARS: usize = 40;

/// Tatra banka's text of a card hold: 40 characters, the merchant and the town padded with runs
/// of spaces, and the country code (`SLOVNAFT-SK40106         Kosice       SK`). A booked row's own
/// text can carry 3 spaces too (`HTTP   M EKOLKY GOV SK`), so the run alone does not tell.
fn is_card_hold_text(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    chars.len() == CARD_HOLD_TEXT_CHARS
        && !chars[0].is_whitespace()
        && text.contains("   ")
        && chars[CARD_HOLD_TEXT_CHARS - 2..]
            .iter()
            .all(char::is_ascii_uppercase)
}

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

    fn line(&self) -> BankLine<'_> {
        BankLine {
            day: self.day,
            amount_minor: self.amount_minor,
            card: self.card.as_deref(),
        }
    }
}

/// A day, an amount and a card, as the bank showed a hold or a booking.
#[derive(Clone, Copy)]
struct BankLine<'a> {
    day: NaiveDate,
    amount_minor: i64,
    card: Option<&'a str>,
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
    Cancelled {
        by_day: NaiveDate,
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
    /// How a pending row sees the entry: the day and amount the bank held it at. An entry no
    /// import has seen shows its ledger day and amount; one the bank booked without a hold shows
    /// none.
    fn held_key(&self) -> Option<(NaiveDate, i64)> {
        let Some(bank) = &self.bank else {
            return Some(self.ledger_key());
        };
        match bank.state {
            BankState::Pending { .. } | BankState::Cancelled { .. } => {
                Some((bank.day, bank.amount_minor))
            }
            BankState::Posted {
                held_day,
                held_amount_minor,
            } => Some((held_day, held_amount_minor)),
            BankState::Booked => None,
        }
    }

    /// How a booked row sees the entry: the day and amount the bank booked it at. An entry no
    /// import has seen shows its ledger day and amount; a hold shows none, and a booked row
    /// reaches it only by posting it.
    fn booked_key(&self) -> Option<(NaiveDate, i64)> {
        let Some(bank) = &self.bank else {
            return Some(self.ledger_key());
        };
        match bank.state {
            BankState::Booked | BankState::Posted { .. } => Some((bank.day, bank.amount_minor)),
            BankState::Pending { .. } | BankState::Cancelled { .. } => None,
        }
    }

    /// The day, amount and card the bank held the entry at, as `held_key` sees it.
    fn held_line(&self) -> Option<BankLine<'_>> {
        let (day, amount_minor) = self.held_key()?;
        Some(BankLine {
            day,
            amount_minor,
            card: self.card(),
        })
    }

    /// The day, amount and card the bank booked the entry at, when an import saw the booking.
    fn bank_booked_line(&self) -> Option<BankLine<'_>> {
        self.bank.as_ref()?;
        let (day, amount_minor) = self.booked_key()?;
        Some(BankLine {
            day,
            amount_minor,
            card: self.card(),
        })
    }

    fn key_seen_by(&self, row: &IncomingRow) -> Option<(NaiveDate, i64)> {
        if row.pending {
            self.held_key()
        } else {
            self.booked_key()
        }
    }

    fn ledger_key(&self) -> (NaiveDate, i64) {
        (self.day, self.total_minor)
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

    fn note_is_a_card_hold_text(&self) -> bool {
        self.own_note.as_deref().is_some_and(is_card_hold_text)
    }

    /// A posted hold takes the booked text while its note is still the bank's text or a card
    /// hold's.
    pub(crate) fn note_once_booked(&self, booked_text: Option<&str>) -> Option<String> {
        let the_banks = self
            .bank
            .as_ref()
            .is_some_and(|bank| bank.text == self.own_note);
        if the_banks || self.note_is_a_card_hold_text() {
            booked_text.map(str::to_owned)
        } else {
            self.own_note.clone()
        }
    }

    /// A booked row that finds an entry no import has seen replaces a card hold's text the entry
    /// kept from before Saldo knew holds. An entry with bank facts keeps its note.
    pub(crate) fn note_once_seen_booked(&self, booked_text: Option<&str>) -> Option<String> {
        if self.bank.is_none() && self.note_is_a_card_hold_text() {
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
    /// The split hold's booked row at another amount: a posting onto a new entry.
    NewReplacingSplitHold(&'a Recorded),
    /// The bank lists a hold an import removed again, pending or booked, in an export as new as
    /// the one that removed it or newer. The row's new entry takes the hold's place.
    RemovedHoldListedAgain(&'a Recorded),
}

pub(crate) struct Matched<'a> {
    pub rows: Vec<Match<'a>>,
    pub cancelled_holds: Vec<&'a Recorded>,
}

/// The tiers run in this order. An entry shows each row one key: a pending row sees it by the day
/// and amount the bank held it at (`held_key`), a booked row by the day and amount the bank
/// booked it at (`booked_key`). Within a greedy tier every candidate pair is ranked by the tier's
/// cost, then the row's place in the file, then the entry's id, and a pair is taken while both
/// its row and its entry are free, so the file's order never lets a row take an entry another
/// row of the tier fits better.
///
/// The same-day tier and the hold stages take an assignment instead, because what one row takes
/// decides what is left for another: no order of greedy tiers can choose between a hold booked at
/// its amount days later and one booked on its day at another amount. The same-day tier takes as
/// many pairs as it can. A hold stage pairs a hold with a booking through `booking_cost`, and only
/// while a pair costs less than leaving its hold and its booking unexplained (`PAIR_WORTH`).
pub(crate) fn match_rows<'a>(
    rows: &'a [IncomingRow],
    items: &'a [LedgerItem],
    removed_holds: &'a [Recorded],
    cashflows: &'a [CounterpartCashflow],
) -> Matched<'a> {
    let mut claims = Claims {
        rows,
        items,
        removed_holds,
        newest_day: rows.iter().map(|row| row.day).max(),
        matches: vec![Match::New; rows.len()],
        used: vec![false; items.len()],
        removed_used: vec![false; removed_holds.len()],
    };
    claims.take_cheapest(legs_to_named_accounts(&claims));
    claims.take_assignment(same_day_and_amount(&claims), None);
    claims.take_cheapest(near_legs_for_rows_naming_no_known_account(&claims));
    claims.take_assignment(holds_booked_rows_post(&claims), Some(PAIR_WORTH));
    claims.take_cheapest(entries_no_import_has_seen(&claims));
    claims.take_assignment(removed_holds_booked_rows_take(&claims), Some(PAIR_WORTH));
    claim_mutually_unique_cashflows(rows, cashflows, &mut claims.matches);
    claims.take_assignment(entries_a_newer_export_booked(&claims), Some(PAIR_WORTH));
    Matched {
        cancelled_holds: cancelled_holds(&claims),
        rows: claims.matches,
    }
}

struct Claims<'a> {
    rows: &'a [IncomingRow],
    items: &'a [LedgerItem],
    removed_holds: &'a [Recorded],
    newest_day: Option<NaiveDate>,
    matches: Vec<Match<'a>>,
    used: Vec<bool>,
    removed_used: Vec<bool>,
}

/// An entry a row can take: one of the account's entries, or a hold an import removed.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
    Item(usize),
    RemovedHold(usize),
}

/// The row at `position` could take `target` as `matched`, at `cost`.
struct Pair<'a, C> {
    cost: C,
    position: usize,
    target: Target,
    matched: Match<'a>,
}

impl<'a> Claims<'a> {
    fn recorded(&self, target: Target) -> &'a Recorded {
        match target {
            Target::Item(item) => &self.items[item].recorded,
            Target::RemovedHold(hold) => &self.removed_holds[hold],
        }
    }

    fn is_free(&self, target: Target) -> bool {
        match target {
            Target::Item(item) => !self.used[item],
            Target::RemovedHold(hold) => !self.removed_used[hold],
        }
    }

    /// Whether a free pending hold the booked `row` may claim was held on its day and amount.
    fn has_a_hold_of_its_day_and_amount(&self, row: &IncomingRow) -> bool {
        self.items.iter().enumerate().any(|(item, ledger_item)| {
            let hold = &ledger_item.recorded;
            self.is_free(Target::Item(item))
                && row.counterpart.may_claim(ledger_item)
                && cards_fit(row, hold)
                && hold.is_pending_hold()
                && hold.held_key() == Some(row.key())
        })
    }

    /// Whether this file lists a hold an import removed again: it is as new as the export that
    /// removed the hold, or newer. An older one lists the hold from before.
    fn lists_again(&self, hold: &Recorded) -> bool {
        !matches!(
            (hold.state(), self.newest_day),
            (Some(BankState::Cancelled { by_day }), Some(newest)) if newest < by_day
        )
    }

    fn new_rows(&self) -> impl Iterator<Item = (usize, &'a IncomingRow)> + '_ {
        let rows = self.rows;
        rows.iter()
            .enumerate()
            .filter(|&(position, _)| matches!(self.matches[position], Match::New))
    }

    /// Every pair of a row no tier has matched yet and a free entry the row may claim, priced by
    /// `price` (`None`: not a candidate).
    fn pairs<C>(
        &self,
        price: impl Fn(&'a IncomingRow, &'a LedgerItem) -> Option<(C, Match<'a>)>,
    ) -> Vec<Pair<'a, C>> {
        let mut pairs = Vec::new();
        for (position, row) in self.new_rows() {
            for (item, ledger_item) in self.items.iter().enumerate() {
                if !self.is_free(Target::Item(item))
                    || !row.counterpart.may_claim(ledger_item)
                    || !cards_fit(row, &ledger_item.recorded)
                {
                    continue;
                }
                if let Some((cost, matched)) = price(row, ledger_item) {
                    pairs.push(Pair {
                        cost,
                        position,
                        target: Target::Item(item),
                        matched,
                    });
                }
            }
        }
        pairs
    }

    /// Every pair of a row no tier has matched yet and a free hold an import removed, with a
    /// fitting card, priced by `price` (`None`: not a candidate).
    fn removed_hold_pairs<C>(
        &self,
        price: impl Fn(&'a IncomingRow, &'a Recorded) -> Option<(C, Match<'a>)>,
    ) -> Vec<Pair<'a, C>> {
        let mut pairs = Vec::new();
        for (position, row) in self.new_rows() {
            for (hold, recorded) in self.removed_holds.iter().enumerate() {
                if !self.is_free(Target::RemovedHold(hold)) || !cards_fit(row, recorded) {
                    continue;
                }
                if let Some((cost, matched)) = price(row, recorded) {
                    pairs.push(Pair {
                        cost,
                        position,
                        target: Target::RemovedHold(hold),
                        matched,
                    });
                }
            }
        }
        pairs
    }

    fn take(&mut self, pair: &Pair<'a, impl Sized>) {
        match pair.target {
            Target::Item(item) => self.used[item] = true,
            Target::RemovedHold(hold) => self.removed_used[hold] = true,
        }
        self.matches[pair.position] = pair.matched;
    }

    /// The row's place in the file, then the entry's id.
    fn rank<C>(&self, pair: &Pair<'a, C>) -> (u32, i64) {
        (
            self.rows[pair.position].index,
            self.recorded(pair.target).bank_entry_event_id,
        )
    }

    fn take_cheapest<C: Ord>(&mut self, mut pairs: Vec<Pair<'a, C>>) {
        pairs.sort_by(|a, b| {
            a.cost
                .cmp(&b.cost)
                .then_with(|| self.rank(a).cmp(&self.rank(b)))
        });
        for pair in pairs {
            if self.is_free(pair.target) && matches!(self.matches[pair.position], Match::New) {
                self.take(&pair);
            }
        }
    }

    /// Takes the cheapest set of pairs (`cheapest_pairs`): with no `worth` as many as the rows and
    /// entries allow, else only while a pair costs less than `worth`. Between equally cheap sets
    /// the rows' places in the file decide, then the entries' ids.
    fn take_assignment(&mut self, mut pairs: Vec<Pair<'a, i64>>, worth: Option<i64>) {
        pairs.sort_by_key(|pair| self.rank(pair));
        let target_key =
            |pair: &Pair<'a, i64>| (self.recorded(pair.target).bank_entry_event_id, pair.target);
        let rows = numbered(pairs.iter().map(|pair| pair.position));
        let targets = numbered(pairs.iter().map(target_key));
        let edges: Vec<Edge<i64>> = pairs
            .iter()
            .map(|pair| Edge {
                row: rows[&pair.position],
                target: targets[&target_key(pair)],
                cost: pair.cost,
            })
            .collect();
        for taken in cheapest_pairs(&edges, rows.len(), targets.len(), worth) {
            self.take(&pairs[taken]);
        }
    }
}

/// Numbers the distinct keys from 0, in their order.
fn numbered<K: Ord>(keys: impl Iterator<Item = K>) -> BTreeMap<K, usize> {
    keys.collect::<BTreeSet<K>>()
        .into_iter()
        .enumerate()
        .map(|(number, key)| (key, number))
        .collect()
}

/// The cost of a same-day pair: an entry in the row's own state costs least.
#[derive(Clone, Copy)]
enum SameDayPreference {
    SameState = 0,
    OtherState = 1,
    Unseen = 2,
    RemovedHold = 3,
}

/// A row takes the entry it sees on its own day and amount: first one in its own state (a booked
/// row a booked or posted entry, a pending row a pending hold), then a posted hold a pending row
/// sees by the day and amount the bank held it at, then one no import has seen, then, for a
/// pending row, a hold an import removed (`lists_again` says whether the row writes anything).
/// The tier is an assignment, because a booked row that takes a posted entry by its booked day and
/// amount can leave the pending row that sees the entry's hold with nothing. A booked row posts a
/// hold of its own day and amount in the first hold stage instead, because another hold can be
/// its own, and waits for that stage before it takes an entry no import has seen.
fn same_day_and_amount<'a>(claims: &Claims<'a>) -> Vec<Pair<'a, i64>> {
    let mut pairs = claims.pairs(|row, ledger_item| {
        let recorded = &ledger_item.recorded;
        if recorded.key_seen_by(row) != Some(row.key()) {
            return None;
        }
        let preference = match (recorded.state(), row.pending) {
            (None, false) if claims.has_a_hold_of_its_day_and_amount(row) => return None,
            (None, _) => SameDayPreference::Unseen,
            (Some(BankState::Pending { .. }), true)
            | (Some(BankState::Booked | BankState::Posted { .. }), false) => {
                SameDayPreference::SameState
            }
            (Some(_), _) => SameDayPreference::OtherState,
        };
        Some((preference as i64, Match::InSaldo(recorded)))
    });
    pairs.extend(claims.removed_hold_pairs(|row, hold| {
        let matched = if claims.lists_again(hold) {
            Match::RemovedHoldListedAgain(hold)
        } else {
            Match::InSaldo(hold)
        };
        (row.pending && hold.held_key() == Some(row.key()))
            .then_some((SameDayPreference::RemovedHold as i64, matched))
    }));
    pairs
}

/// A booked row the first hold stage left new takes an entry no import has seen on its day and
/// amount.
fn entries_no_import_has_seen<'a>(claims: &Claims<'a>) -> Vec<Pair<'a, ()>> {
    claims.pairs(|row, ledger_item| {
        let recorded = &ledger_item.recorded;
        (!row.pending && recorded.bank.is_none() && recorded.ledger_key() == row.key())
            .then_some(((), Match::InSaldo(recorded)))
    })
}

/// A booking at the held amount a day after the hold weighs as much as one on the hold's own day
/// 10 % away from the held amount.
const DAY_IN_BASIS_POINTS: i64 = 1_000;

/// A hold stage pairs one more hold and booking only while that costs less than a booking 6 days
/// after its hold: every booking at the held amount within 5 days, one on the hold's day less than
/// 60 % away from the held amount, and a move of other bookings to farther holds only when the
/// whole move costs less. A purchase the bank never listed pending, next to a hold it cancelled,
/// stays another purchase unless it is that close.
const PAIR_WORTH: i64 = (NEAR_DAYS + 1) * DAY_IN_BASIS_POINTS;

/// How far `booked` is from `held`, in basis points of the held amount, when the booking can be
/// the hold's: at the held amount on the held day or up to 5 days after it, costing its days
/// apart, or on the held day at another amount of the same sign when both name one card, costing
/// its gap. Every tier already refused cards that are both known and differ.
///
/// A booking on the hold's own day less than 10 % away from the held amount costs less than one
/// at the held amount a day later: the bank books a hold at another amount (a currency
/// conversion) a little away from it. A booking further away costs more, because another purchase
/// of the card's day is then the likelier story.
fn booking_cost(held: BankLine<'_>, booked: BankLine<'_>) -> Option<i64> {
    let distance = if held.amount_minor == booked.amount_minor {
        booked_soon_after(held.day, booked.day)
            .then(|| i128::from(days_apart(held.day, booked.day) * DAY_IN_BASIS_POINTS))?
    } else {
        let one_card = held.card.is_some() && held.card == booked.card;
        let fits = one_card
            && held.day == booked.day
            && held.amount_minor.signum() == booked.amount_minor.signum();
        let gap = i128::from(held.amount_minor) - i128::from(booked.amount_minor);
        fits.then(|| (gap.abs() * 10_000).checked_div(i128::from(held.amount_minor).abs()))?
            .unwrap_or(i128::MAX)
    };
    Some(i64::try_from(distance).unwrap_or(i64::MAX))
}

/// A booked row posts a pending hold it can be the booking of.
fn holds_booked_rows_post<'a>(claims: &Claims<'a>) -> Vec<Pair<'a, i64>> {
    claims.pairs(|row, ledger_item| {
        let hold = &ledger_item.recorded;
        if row.pending || !hold.is_pending_hold() {
            return None;
        }
        let held = hold.held_line()?;
        let cost = booking_cost(held, row.line())?;
        let matched = match (held.amount_minor == row.amount_minor, hold.split_group_id) {
            (true, _) => Match::HoldPosted(hold),
            (false, None) => Match::HoldPostedAtOtherAmount(hold),
            (false, Some(_)) => Match::NewReplacingSplitHold(hold),
        };
        Some((cost, matched))
    })
}

/// A booked row every live hold left new takes the place of a hold an import removed that it can
/// be the booking of, when this export is as new as the one that removed the hold or newer.
fn removed_holds_booked_rows_take<'a>(claims: &Claims<'a>) -> Vec<Pair<'a, i64>> {
    claims.removed_hold_pairs(|row, hold| {
        if row.pending || !claims.lists_again(hold) {
            return None;
        }
        let cost = booking_cost(hold.held_line()?, row.line())?;
        Some((cost, Match::RemovedHoldListedAgain(hold)))
    })
}

/// A pending row of an older export finds the entry a newer export booked it as. At its amount
/// that includes a posted entry whose hold Saldo took for another day's, because the bank's own
/// data cannot always tell two holds of one amount apart. At another amount only an entry with
/// no hold Saldo knows of, on the row's own day: the days after it hold the card's other
/// bookings, which an older export does not list yet.
fn entries_a_newer_export_booked<'a>(claims: &Claims<'a>) -> Vec<Pair<'a, i64>> {
    claims.pairs(|row, ledger_item| {
        let recorded = &ledger_item.recorded;
        if !row.pending {
            return None;
        }
        let booked = recorded.bank_booked_line()?;
        let cost = booking_cost(row.line(), booked)?;
        let at_its_amount = booked.amount_minor == row.amount_minor;
        (at_its_amount || recorded.state() == Some(BankState::Booked))
            .then_some((cost, Match::InSaldo(recorded)))
    })
}

/// An unused pending hold on a day the file covers, in a file newer than the last export that
/// listed it.
fn cancelled_holds<'a>(claims: &Claims<'a>) -> Vec<&'a Recorded> {
    let days = || claims.rows.iter().map(|row| row.day);
    let (Some(oldest), Some(newest)) = (days().min(), days().max()) else {
        return Vec::new();
    };
    claims
        .items
        .iter()
        .zip(&claims.used)
        .filter(|&(ledger_item, &used)| {
            let hold = &ledger_item.recorded;
            !used
                && ledger_item.transfer_leg.is_none()
                && matches!(
                    (hold.state(), hold.held_key()),
                    (Some(BankState::Pending { seen_to }), Some((held_day, _)))
                        if held_day >= oldest && newest > seen_to
                )
        })
        .map(|(ledger_item, _)| &ledger_item.recorded)
        .collect()
}

/// A row naming own account X takes a leg to X on its day and amount, or one with no bank entry
/// and the same amount within 5 days, the closest first, then the newest.
fn legs_to_named_accounts<'a>(claims: &Claims<'a>) -> Vec<Pair<'a, LegCost<'a>>> {
    claims.pairs(|row, ledger_item| {
        let Counterpart::Own(account_id) = row.counterpart else {
            return None;
        };
        let leg = ledger_item.transfer_leg.as_ref()?;
        let recorded = &ledger_item.recorded;
        let on_its_day = recorded.key_seen_by(row) == Some(row.key());
        (leg.other_account_id == account_id
            && (on_its_day || near_without_bank_entry(row, recorded)))
        .then(|| leg_pair(row, recorded, leg))
    })
}

/// A row naming no known account takes a leg with no bank entry and the same amount within 5
/// days, the closest first, then the newest.
fn near_legs_for_rows_naming_no_known_account<'a>(
    claims: &Claims<'a>,
) -> Vec<Pair<'a, LegCost<'a>>> {
    claims.pairs(|row, ledger_item| {
        let names_no_known_account =
            matches!(row.counterpart, Counterpart::None | Counterpart::Unknown);
        let leg = ledger_item.transfer_leg.as_ref()?;
        let recorded = &ledger_item.recorded;
        (names_no_known_account && near_without_bank_entry(row, recorded))
            .then(|| leg_pair(row, recorded, leg))
    })
}

type LegCost<'a> = (bool, i64, Reverse<&'a str>);

fn leg_pair<'a>(
    row: &IncomingRow,
    recorded: &'a Recorded,
    leg: &'a TransferLeg,
) -> (LegCost<'a>, Match<'a>) {
    let key = recorded
        .key_seen_by(row)
        .unwrap_or_else(|| recorded.ledger_key());
    let on_its_day = key == row.key();
    let cost = (
        !on_its_day,
        days_apart(row.day, key.0),
        Reverse(leg.created_at.as_str()),
    );
    let matched = if on_its_day {
        Match::InSaldo(recorded)
    } else {
        Match::TransferLeg {
            leg: recorded,
            other_account_name: &leg.other_account_name,
        }
    };
    (cost, matched)
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
                    cashflow.account_id == account_id
                        && cashflow
                            .recorded
                            .booked_key()
                            .is_some_and(|(day, amount_minor)| {
                                amount_minor == other_side && days_apart(row.day, day) <= NEAR_DAYS
                            })
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
        outcome_with_removed_holds(rows, items, &[])
    }

    fn outcome_with_removed_holds(
        rows: &[IncomingRow],
        items: &[LedgerItem],
        removed_holds: &[Recorded],
    ) -> (Vec<String>, Vec<i64>) {
        let matched = match_rows(rows, items, removed_holds, &[]);
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
                Match::NewReplacingSplitHold(hold) => {
                    format!("new, replacing split {}", hold.bank_entry_event_id)
                }
                Match::RemovedHoldListedAgain(hold) => {
                    format!("new, listing removed {} again", hold.bank_entry_event_id)
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
    fn a_hold_at_another_amount_goes_to_the_closest_amount() {
        let github = || booked(DEBIT_CARD, row(0, "2026-04-30", -3_299));
        let two_holds = [
            hold(1, "2026-04-30", -3_400, DEBIT_CARD, "2026-05-01"),
            hold(2, "2026-04-30", -3_300, DEBIT_CARD, "2026-05-01"),
        ];
        let one_hold = [hold(1, "2026-04-30", -3_300, DEBIT_CARD, "2026-05-01")];
        let two_rows = [booked(DEBIT_CARD, row(0, "2026-04-30", -3_250)), {
            let mut github = github();
            github.index = 1;
            github
        }];

        assert_eq!(
            verdicts(&[github()], &two_holds),
            ["posts 2 at another amount"]
        );
        assert_eq!(
            verdicts(&two_rows, &one_hold),
            ["new", "posts 1 at another amount"]
        );
    }

    #[test]
    fn every_booking_posts_its_own_hold_when_the_cheapest_pair_would_leave_one_out() {
        let github = hold(1, "2026-04-30", -3_300, DEBIT_CARD, "2026-04-30");
        let ticket = hold(2, "2026-04-30", -50, DEBIT_CARD, "2026-04-30");
        let rows = [
            booked(DEBIT_CARD, row(0, "2026-04-30", -3_299)),
            booked(DEBIT_CARD, row(1, "2026-05-01", -50)),
        ];

        assert_eq!(
            verdicts(&rows, &[github, ticket]),
            ["posts 1 at another amount", "posts 2"]
        );
    }

    #[test]
    fn a_hold_of_the_rows_day_and_amount_yields_when_another_hold_is_the_rows_own() {
        let held_on_5_oct = hold(1, "2026-10-05", -3_300, SECOND_CARD, "2026-10-06");
        let held_on_6_oct = hold(2, "2026-10-06", -3_300, SECOND_CARD, "2026-10-06");
        let rows = [
            booked(SECOND_CARD, row(0, "2026-10-06", -3_300)),
            booked(SECOND_CARD, row(1, "2026-10-06", -3_298)),
        ];

        assert_eq!(
            outcome(&rows, &[held_on_5_oct, held_on_6_oct]),
            (
                vec!["posts 1".to_owned(), "posts 2 at another amount".to_owned()],
                vec![]
            )
        );
    }

    #[test]
    fn a_booking_far_from_a_holds_amount_is_another_purchase() {
        let items = [hold(1, "2026-10-04", -3_300, SECOND_CARD, "2026-10-04")];
        let rows = [
            booked(SECOND_CARD, row(0, "2026-10-04", -50)),
            booked(SECOND_CARD, row(1, "2026-10-06", -900)),
        ];

        assert_eq!(outcome(&rows, &items), (vec!["new".into(); 2], vec![1]));
        assert_eq!(
            verdicts(&[booked(SECOND_CARD, row(0, "2026-10-04", -1_400))], &items),
            ["posts 1 at another amount"],
            "less than 60 % away"
        );
    }

    #[test]
    fn a_booking_keeps_its_own_hold_rather_than_move_to_a_far_one_for_another_booking() {
        let ticket = hold(1, "2026-10-03", -50, SECOND_CARD, "2026-10-05");
        let shop = hold(2, "2026-10-04", -500, SECOND_CARD, "2026-10-05");
        let rows = [
            booked(SECOND_CARD, row(0, "2026-10-04", -50)),
            booked(SECOND_CARD, row(1, "2026-10-06", -50)),
            booked(SECOND_CARD, row(2, "2026-10-10", -900)),
        ];

        assert_eq!(
            outcome(&rows, &[ticket, shop]),
            (
                vec!["posts 1".to_owned(), "new".to_owned(), "new".to_owned()],
                vec![2]
            )
        );
    }

    #[test]
    fn an_older_export_again_leaves_each_pending_row_the_entry_it_sees() {
        let first_ticket = posted(7, ("2026-10-03", -50), CARD, ("2026-10-02", -50));
        let second_ticket = posted(10, ("2026-10-03", -50), CARD, ("2026-10-03", -50));
        let removed = with_bank(
            6,
            "2026-10-02",
            -50,
            Some(CARD),
            BankState::Cancelled {
                by_day: date("2026-10-05"),
            },
        )
        .recorded;
        let of_3_oct = [
            pending(CARD, row(0, "2026-10-02", -50)),
            booked(CARD, row(1, "2026-10-03", -50)),
            pending(CARD, row(2, "2026-10-03", -50)),
        ];

        assert_eq!(
            outcome_with_removed_holds(&of_3_oct, &[first_ticket, second_ticket], &[removed]).0,
            ["in Saldo 6", "in Saldo 7", "in Saldo 10"]
        );
    }

    #[test]
    fn a_booked_row_takes_a_removed_holds_place_only_after_every_live_hold() {
        let removed_by = |by_day| {
            with_bank(
                1,
                "2026-10-20",
                -1_999,
                Some(CARD),
                BankState::Cancelled {
                    by_day: date(by_day),
                },
            )
            .recorded
        };
        let booked_on_21_oct = || booked(CARD, row(0, "2026-10-21", -1_999));
        let lidl = || booked(CARD, row(1, "2026-10-23", -700));

        assert_eq!(
            outcome_with_removed_holds(
                &[booked_on_21_oct(), lidl()],
                &[],
                &[removed_by("2026-10-23")]
            )
            .0,
            ["new, listing removed 1 again", "new"]
        );
        assert_eq!(
            outcome_with_removed_holds(
                &[booked_on_21_oct(), lidl()],
                &[hold(2, "2026-10-20", -1_999, CARD, "2026-10-23")],
                &[removed_by("2026-10-23")]
            )
            .0,
            ["posts 2", "new"],
            "a live hold first"
        );
        assert_eq!(
            outcome_with_removed_holds(
                &[booked_on_21_oct(), lidl()],
                &[],
                &[removed_by("2026-10-24")]
            )
            .0,
            ["new", "new"],
            "an export older than the one that removed it"
        );
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
    fn a_split_hold_posts_at_its_amount_and_at_another_amount_is_replaced_by_the_row() {
        let items = [split(hold(
            1,
            "2026-04-30",
            -3_300,
            DEBIT_CARD,
            "2026-04-30",
        ))];
        let github = || booked(DEBIT_CARD, row(0, "2026-04-30", -3_299));
        let later = booked(DEBIT_CARD, row(1, "2026-05-02", -500));

        assert_eq!(
            verdicts(&[booked(DEBIT_CARD, row(0, "2026-04-30", -3_300))], &items),
            ["posts 1"]
        );
        assert_eq!(
            outcome(&[github(), later], &items),
            (
                vec!["new, replacing split 1".to_owned(), "new".to_owned()],
                vec![]
            ),
            "the replaced hold belongs to its row, not to the cancelled holds"
        );
    }

    #[test]
    fn a_hold_at_another_amount_on_its_day_wins_over_a_purchase_of_its_amount_the_next_day() {
        let items = [hold(1, "2026-10-02", -3_300, SECOND_CARD, "2026-10-02")];
        let rows = [
            booked(SECOND_CARD, row(0, "2026-10-02", -3_299)),
            booked(SECOND_CARD, row(1, "2026-10-03", -3_300)),
        ];

        assert_eq!(
            verdicts(&rows, &items),
            ["posts 1 at another amount", "new"]
        );
    }

    #[test]
    fn a_hold_posts_with_its_closest_booking_whatever_order_the_file_lists_them() {
        let items = [hold(1, "2026-10-03", -50, CARD, "2026-10-03")];
        let bought_on_5_oct = || booked(CARD, row(0, "2026-10-05", -50));
        let booked_on_4_oct = || booked(CARD, row(1, "2026-10-04", -50));

        assert_eq!(
            verdicts(&[bought_on_5_oct(), booked_on_4_oct()], &items),
            ["new", "posts 1"]
        );
        assert_eq!(
            verdicts(
                &[
                    IncomingRow {
                        index: 1,
                        ..bought_on_5_oct()
                    },
                    IncomingRow {
                        index: 0,
                        ..booked_on_4_oct()
                    },
                ],
                &items
            ),
            ["new", "posts 1"],
            "newest first"
        );
    }

    #[test]
    fn a_pending_row_sees_a_posted_hold_only_by_the_day_and_amount_the_bank_held_it_at() {
        let ticket_of_24_oct = posted(1, ("2026-10-25", -50), CARD, ("2026-10-24", -50));
        let ticket_of_25_oct = posted(2, ("2026-10-26", -50), CARD, ("2026-10-25", -50));
        let rows = [
            pending(CARD, row(0, "2026-10-24", -50)),
            pending(CARD, row(1, "2026-10-25", -50)),
        ];

        assert_eq!(
            verdicts(&rows, &[ticket_of_24_oct, ticket_of_25_oct]),
            ["in Saldo 1", "in Saldo 2"]
        );
    }

    #[test]
    fn a_booked_row_leaves_a_posted_entry_to_the_pending_row_that_sees_its_hold() {
        let posted_on_6_oct = posted(1, ("2026-10-06", -50), CARD, ("2026-10-05", -50));
        let booked_on_6_oct = booked_entry(2, "2026-10-06", -50, CARD);
        let rows = [
            booked(CARD, row(0, "2026-10-06", -50)),
            pending(CARD, row(1, "2026-10-05", -50)),
        ];

        assert_eq!(
            verdicts(&rows, &[posted_on_6_oct, booked_on_6_oct]),
            ["in Saldo 2", "in Saldo 1"]
        );
    }

    #[test]
    fn a_booked_row_imported_again_leaves_a_posted_hold_to_its_older_pending_row() {
        let ticket_posted = posted(1, ("2026-10-01", -50), CARD, ("2026-10-01", -50));
        let ticket_booked = booked_entry(2, "2026-10-01", -50, CARD);
        let removed = with_bank(
            3,
            "2026-10-01",
            -50,
            Some(CARD),
            BankState::Cancelled {
                by_day: date("2026-10-01"),
            },
        )
        .recorded;
        let of_1_oct = [
            pending(CARD, row(0, "2026-10-01", -50)),
            booked(CARD, row(1, "2026-10-01", -50)),
        ];

        assert_eq!(
            outcome_with_removed_holds(&of_1_oct, &[ticket_posted, ticket_booked], &[removed]).0,
            ["in Saldo 1", "in Saldo 2"],
            "the removed hold stays removed"
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
            held_again(CARD, "2026-09-16", -7_896, &tchibo),
            ["new"],
            "a hold after the booking is another purchase"
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
                &[held_on_24_oct()],
                &[posted(
                    1,
                    ("2026-10-25", -7_896),
                    CARD,
                    ("2026-10-20", -7_896)
                )]
            ),
            ["in Saldo 1"],
            "a booking Saldo posted from another day's hold"
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
    fn a_pending_row_of_an_older_export_finds_the_entry_booked_on_its_day_at_another_amount() {
        let github = || pending(DEBIT_CARD, row(0, "2026-04-30", -3_300));
        let booked_on = |day, amount_minor| booked_entry(1, day, amount_minor, DEBIT_CARD);

        assert_eq!(
            verdicts(&[github()], &[booked_on("2026-04-30", -3_299)]),
            ["in Saldo 1"]
        );
        assert_eq!(
            verdicts(
                &[github()],
                &[
                    booked_on("2026-04-30", -3_299),
                    booked_entry(2, "2026-05-04", -5_000, DEBIT_CARD),
                ]
            ),
            ["in Saldo 1"],
            "the card's later bookings an older export does not list yet"
        );
        for (case, row, items) in [
            (
                "another day",
                github(),
                vec![booked_on("2026-05-01", -3_299)],
            ),
            (
                "another card",
                pending(CARD, row(0, "2026-04-30", -3_300)),
                vec![booked_on("2026-04-30", -3_299)],
            ),
            (
                "no card",
                IncomingRow {
                    pending: true,
                    ..row(0, "2026-04-30", -3_300)
                },
                vec![booked_on("2026-04-30", -3_299)],
            ),
            ("a refund", github(), vec![booked_on("2026-04-30", 3_299)]),
            (
                "a posting of another hold",
                github(),
                vec![posted(
                    1,
                    ("2026-04-30", -3_299),
                    DEBIT_CARD,
                    ("2026-04-30", -3_250),
                )],
            ),
        ] {
            assert_eq!(verdicts(&[row], &items), ["new"], "{case}");
        }
        assert_eq!(
            verdicts(
                &[github()],
                &[
                    booked_entry(2, "2026-04-30", -1_000, DEBIT_CARD),
                    booked_on("2026-04-30", -3_299),
                ]
            ),
            ["in Saldo 1"],
            "the closest amount of two"
        );
    }

    #[test]
    fn an_older_exports_pending_rows_find_a_booking_at_another_amount_and_one_days_later() {
        let items = [
            booked_entry(1, "2026-10-05", -3_300, CARD),
            booked_entry(2, "2026-10-04", -3_299, CARD),
        ];
        let rows = [
            pending(CARD, row(0, "2026-10-04", -3_300)),
            pending(CARD, row(1, "2026-10-03", -3_300)),
        ];

        assert_eq!(verdicts(&rows, &items), ["in Saldo 2", "in Saldo 1"]);
    }

    #[test]
    fn a_pending_row_of_an_export_older_than_the_one_that_removed_its_hold_finds_the_hold() {
        let removed_by = |by_day| {
            with_bank(
                1,
                "2026-10-25",
                -1_999,
                Some(CARD),
                BankState::Cancelled {
                    by_day: date(by_day),
                },
            )
            .recorded
        };
        let export_to_30_oct = |held: IncomingRow| {
            outcome_with_removed_holds(
                &[held, booked(CARD, row(1, "2026-10-30", -500))],
                &[],
                &[removed_by("2026-10-31")],
            )
            .0
        };

        assert_eq!(
            export_to_30_oct(pending(CARD, row(0, "2026-10-25", -1_999))),
            ["in Saldo 1", "new"]
        );
        for (case, held) in [
            ("another day", pending(CARD, row(0, "2026-10-24", -1_999))),
            (
                "another amount",
                pending(CARD, row(0, "2026-10-25", -2_000)),
            ),
            (
                "another card",
                pending(SECOND_CARD, row(0, "2026-10-25", -1_999)),
            ),
            ("a booked row", booked(CARD, row(0, "2026-10-25", -1_999))),
        ] {
            assert_eq!(export_to_30_oct(held), ["new", "new"], "{case}");
        }
        assert_eq!(
            outcome_with_removed_holds(
                &[
                    pending(CARD, row(0, "2026-10-25", -1_999)),
                    booked(CARD, row(1, "2026-10-31", -500)),
                ],
                &[],
                &[removed_by("2026-10-31")],
            )
            .0,
            ["new, listing removed 1 again", "new"],
            "an export as new as the one that removed it lists the hold again"
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
            with_note(Some(SLOVNAFT_HOLD), None).note_once_booked(booked_text),
            booked_text.map(str::to_owned),
            "a card hold's text"
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

    const SLOVNAFT_HOLD: &str = "SLOVNAFT-SK40106         Kosice       SK";

    #[test]
    fn only_the_card_hold_layout_counts_as_a_holds_text() {
        let is_a_hold = |note: &str| {
            let mut item = typed(1, "2026-10-25", -1_999);
            item.recorded.own_note = Some(note.to_owned());
            item.recorded.note_once_seen_booked(Some("BOOKED")) == Some("BOOKED".to_owned())
        };

        assert!(is_a_hold(SLOVNAFT_HOLD));
        assert!(is_a_hold("LIDL DAKUJEME ZA NAKUP    BRATISLAVA  SK"));
        for (case, note) in [
            ("a booked eKolky text", "HTTP   M EKOLKY GOV SK"),
            (
                "the user's words after it",
                "HTTP   M EKOLKY GOV SK passport stamp",
            ),
            ("39 characters", "SLOVNAFT-SK40106        Kosice       SK"),
            (
                "no run of 3 spaces",
                "SLOVNAFT-SK40106-Kosice-SLOVENSKO-ABC SK",
            ),
            (
                "no country code",
                "SLOVNAFT-SK40106         Kosice       sk",
            ),
            ("leading spaces", "   SLOVNAFT-SK40106      Kosice       SK"),
        ] {
            assert!(!is_a_hold(note), "{case}");
        }
    }

    #[test]
    fn a_booked_row_never_rewrites_the_note_of_an_entry_with_bank_facts() {
        let booked_with_note = |note: &str| {
            let mut item = booked_entry(1, "2026-10-25", -1_999, CARD);
            item.recorded.own_note = Some(note.to_owned());
            item.recorded
        };

        for note in [SLOVNAFT_HOLD, "HTTP   M EKOLKY GOV SK passport stamp"] {
            assert_eq!(
                booked_with_note(note).note_once_seen_booked(Some("BOOKED")),
                Some(note.to_owned())
            );
        }
    }
}
