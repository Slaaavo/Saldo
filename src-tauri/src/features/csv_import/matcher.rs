use std::cmp::Reverse;

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

    /// The day and amount the bank booked the entry at, when an import saw the booking.
    fn bank_booked_key(&self) -> Option<(NaiveDate, i64)> {
        self.bank.as_ref().and_then(|_| self.booked_key())
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

    pub(crate) fn note_once_booked(&self, booked_text: Option<&str>) -> Option<String> {
        let the_banks = self
            .bank
            .as_ref()
            .is_some_and(|bank| bank.text == self.own_note);
        if the_banks {
            booked_text.map(str::to_owned)
        } else {
            self.note_without_hold_padding(booked_text)
        }
    }

    pub(crate) fn note_without_hold_padding(&self, booked_text: Option<&str>) -> Option<String> {
        let padded = self
            .own_note
            .as_deref()
            .is_some_and(|note| note.contains(HOLD_PADDING));
        if padded {
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
    /// A pending row of an export as new as the one that removed this hold, or newer: the bank
    /// lists the hold again.
    RemovedHoldListedAgain(&'a Recorded),
}

pub(crate) struct Matched<'a> {
    pub rows: Vec<Match<'a>>,
    pub cancelled_holds: Vec<&'a Recorded>,
}

/// The tiers run in this order. An entry shows each row one key: a pending row sees it by the day
/// and amount the bank held it at (`held_key`), a booked row by the day and amount the bank
/// booked it at (`booked_key`). Within a tier every candidate pair is ranked by the tier's cost,
/// then the row's place in the file, then the entry's id, and a pair is taken while both its row
/// and its entry are free, so the file's order never lets a row take an entry another row of the
/// tier fits better. A hold booked on its own day at another amount goes before one booked days
/// later at its amount: the same day is the stronger sign.
pub(crate) fn match_rows<'a>(
    rows: &'a [IncomingRow],
    items: &'a [LedgerItem],
    removed_holds: &'a [Recorded],
    cashflows: &'a [CounterpartCashflow],
) -> Matched<'a> {
    let mut claims = Claims {
        rows,
        items,
        matches: vec![Match::New; rows.len()],
        used: vec![false; items.len()],
    };
    claims.take_cheapest(legs_to_named_accounts(&claims));
    claims.take_cheapest(same_day_and_amount(&claims));
    claims.take_cheapest(near_legs_for_rows_naming_no_known_account(&claims));
    claims.take_mutually_unique(holds_booked_at_another_amount(&claims));
    claims.take_cheapest(holds_booked_on_a_later_day(&claims));
    claim_mutually_unique_cashflows(rows, cashflows, &mut claims.matches);
    claim_removed_holds_listed_again(rows, removed_holds, &mut claims.matches);
    claims.take_cheapest(entries_a_newer_export_booked_later(&claims));
    claims.take_mutually_unique(entries_a_newer_export_booked_at_another_amount(&claims));
    Matched {
        cancelled_holds: cancelled_holds(&claims),
        rows: claims.matches,
    }
}

struct Claims<'a> {
    rows: &'a [IncomingRow],
    items: &'a [LedgerItem],
    matches: Vec<Match<'a>>,
    used: Vec<bool>,
}

/// The row at `position` could take the entry at `item` as `matched`, at `cost`.
struct Pair<'a, C> {
    cost: C,
    position: usize,
    item: usize,
    matched: Match<'a>,
}

impl<'a> Claims<'a> {
    /// Every pair of a row no tier has matched yet and a free entry the row may claim, priced by
    /// `price` (`None`: not a candidate).
    fn pairs<C>(
        &self,
        price: impl Fn(&'a IncomingRow, &'a LedgerItem) -> Option<(C, Match<'a>)>,
    ) -> Vec<Pair<'a, C>> {
        let mut pairs = Vec::new();
        for (position, row) in self.rows.iter().enumerate() {
            if !matches!(self.matches[position], Match::New) {
                continue;
            }
            for (item, ledger_item) in self.items.iter().enumerate() {
                if self.used[item]
                    || !row.counterpart.may_claim(ledger_item)
                    || !cards_fit(row, &ledger_item.recorded)
                {
                    continue;
                }
                if let Some((cost, matched)) = price(row, ledger_item) {
                    pairs.push(Pair {
                        cost,
                        position,
                        item,
                        matched,
                    });
                }
            }
        }
        pairs
    }

    fn take_cheapest<C: Ord>(&mut self, mut pairs: Vec<Pair<'a, C>>) {
        let (rows, items) = (self.rows, self.items);
        pairs.sort_by(|a, b| {
            let rank = |pair: &Pair<'a, C>| {
                (
                    rows[pair.position].index,
                    items[pair.item].recorded.bank_entry_event_id,
                )
            };
            a.cost.cmp(&b.cost).then_with(|| rank(a).cmp(&rank(b)))
        });
        for pair in pairs {
            if matches!(self.matches[pair.position], Match::New) && !self.used[pair.item] {
                self.used[pair.item] = true;
                self.matches[pair.position] = pair.matched;
            }
        }
    }

    /// Takes the pairs whose row has no other candidate and whose entry no other row's.
    fn take_mutually_unique(&mut self, pairs: Vec<Pair<'a, ()>>) {
        let mut candidates_of: Vec<(usize, Vec<usize>)> = Vec::new();
        for pair in &pairs {
            match candidates_of.last_mut() {
                Some((position, items)) if *position == pair.position => items.push(pair.item),
                _ => candidates_of.push((pair.position, vec![pair.item])),
            }
        }
        for (position, item) in only_candidates_of_each_other(candidates_of, self.items.len()) {
            if let Some(pair) = pairs
                .iter()
                .find(|pair| pair.position == position && pair.item == item)
            {
                self.used[item] = true;
                self.matches[position] = pair.matched;
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

/// A row takes the entry it sees on its own day and amount: first one in its own state (a booked
/// row a booked or posted entry, a pending row a pending hold), then one in the other state (a
/// booked row posts a hold, a pending row finds a posted hold), then one no import has seen.
fn same_day_and_amount<'a>(claims: &Claims<'a>) -> Vec<Pair<'a, SameDayPreference>> {
    claims.pairs(|row, ledger_item| {
        let recorded = &ledger_item.recorded;
        if recorded.key_seen_by(row) == Some(row.key()) {
            let preference = match (recorded.state(), row.pending) {
                (None, _) => SameDayPreference::Unseen,
                (Some(BankState::Pending { .. }), true)
                | (Some(BankState::Booked | BankState::Posted { .. }), false) => {
                    SameDayPreference::SameState
                }
                (Some(_), _) => SameDayPreference::OtherState,
            };
            Some((preference, Match::InSaldo(recorded)))
        } else if !row.pending
            && recorded.is_pending_hold()
            && recorded.held_key() == Some(row.key())
        {
            Some((SameDayPreference::OtherState, Match::HoldPosted(recorded)))
        } else {
            None
        }
    })
}

/// A booked row posts a hold with its amount held up to 5 days before it, the closest first.
fn holds_booked_on_a_later_day<'a>(claims: &Claims<'a>) -> Vec<Pair<'a, i64>> {
    claims.pairs(|row, ledger_item| {
        let hold = &ledger_item.recorded;
        let (held_day, held_amount_minor) = hold.held_key()?;
        (!row.pending
            && hold.is_pending_hold()
            && held_amount_minor == row.amount_minor
            && booked_soon_after(held_day, row.day))
        .then(|| (days_apart(held_day, row.day), Match::HoldPosted(hold)))
    })
}

/// A booked row with a card posts the one hold on that card and day held at another amount.
fn holds_booked_at_another_amount<'a>(claims: &Claims<'a>) -> Vec<Pair<'a, ()>> {
    claims.pairs(|row, ledger_item| {
        let hold = &ledger_item.recorded;
        let (held_day, held_amount_minor) = hold.held_key()?;
        let fits = !row.pending
            && row.card.is_some()
            && hold.is_pending_hold()
            && hold.card() == row.card.as_deref()
            && held_day == row.day
            && held_amount_minor.signum() == row.amount_minor.signum()
            && held_amount_minor != row.amount_minor;
        fits.then(|| {
            let matched = match hold.split_group_id {
                None => Match::HoldPostedAtOtherAmount(hold),
                Some(_) => Match::NewReplacingSplitHold(hold),
            };
            ((), matched)
        })
    })
}

/// A pending row of an older export finds the entry a newer export booked on its day or up to 5
/// days after it, the closest first. That includes a posted entry whose hold Saldo took for
/// another day's: the bank's own data cannot always tell two holds of one amount apart.
fn entries_a_newer_export_booked_later<'a>(claims: &Claims<'a>) -> Vec<Pair<'a, i64>> {
    claims.pairs(|row, ledger_item| {
        let recorded = &ledger_item.recorded;
        let (booked_day, booked_amount_minor) = recorded.bank_booked_key()?;
        (row.pending
            && booked_amount_minor == row.amount_minor
            && booked_soon_after(row.day, booked_day))
        .then(|| (days_apart(row.day, booked_day), Match::InSaldo(recorded)))
    })
}

/// A pending row of an older export with a card finds the one entry a newer export booked on
/// that card and day at another amount, with no hold Saldo knows of. Only the row's own day: the
/// days after it hold the card's other bookings, which an older export does not list yet.
fn entries_a_newer_export_booked_at_another_amount<'a>(claims: &Claims<'a>) -> Vec<Pair<'a, ()>> {
    claims.pairs(|row, ledger_item| {
        let recorded = &ledger_item.recorded;
        let (booked_day, booked_amount_minor) = recorded.bank_booked_key()?;
        let fits = row.pending
            && row.card.is_some()
            && recorded.state() == Some(BankState::Booked)
            && recorded.card() == row.card.as_deref()
            && booked_day == row.day
            && booked_amount_minor.signum() == row.amount_minor.signum()
            && booked_amount_minor != row.amount_minor;
        fits.then_some(((), Match::InSaldo(recorded)))
    })
}

/// A pending row finds a hold an import removed as cancelled by its bank day, amount and card.
/// An export older than the one that removed it lists it from before, and the row writes
/// nothing; one as new or newer lists it again.
fn claim_removed_holds_listed_again<'a>(
    rows: &[IncomingRow],
    removed_holds: &'a [Recorded],
    matches: &mut [Match<'a>],
) {
    let Some(newest) = rows.iter().map(|row| row.day).max() else {
        return;
    };
    let mut pairs: Vec<(u32, i64, usize, usize)> = Vec::new();
    for (position, row) in rows.iter().enumerate() {
        if !row.pending || !matches!(matches[position], Match::New) {
            continue;
        }
        for (hold, recorded) in removed_holds.iter().enumerate() {
            if recorded.held_key() == Some(row.key()) && cards_fit(row, recorded) {
                pairs.push((row.index, recorded.bank_entry_event_id, position, hold));
            }
        }
    }
    pairs.sort_unstable();
    let mut used = vec![false; removed_holds.len()];
    for (_, _, position, hold) in pairs {
        if used[hold] || !matches!(matches[position], Match::New) {
            continue;
        }
        used[hold] = true;
        let recorded = &removed_holds[hold];
        matches[position] = match recorded.state() {
            Some(BankState::Cancelled { by_day }) if newest < by_day => Match::InSaldo(recorded),
            _ => Match::RemovedHoldListedAgain(recorded),
        };
    }
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
            ["in Saldo 1", "in Saldo 2"],
            "either way, both rows are in Saldo"
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
                "two candidates",
                github(),
                vec![
                    booked_on("2026-04-30", -3_299),
                    booked_entry(2, "2026-04-30", -1_000, DEBIT_CARD),
                ],
            ),
        ] {
            assert_eq!(verdicts(&[row], &items), ["new"], "{case}");
        }
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
