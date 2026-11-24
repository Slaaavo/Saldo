//! Random card purchases, each held and then booked (a day or two later, sometimes at another
//! amount) or cancelled, exported by the bank at random days, newest row first or last. Each
//! harness imports the exports with the review's defaults and compares Saldo with the bank.

use super::harness::{harness_seeds, iso_day, Xorshift};
use super::*;

const SCENARIO_DAYS: u64 = 8;

#[derive(Debug, Clone)]
struct Purchase {
    card: &'static str,
    held_on: u64,
    held_minor: i64,
    /// The booked day and amount, or `None` for a hold the bank cancels on `settles_on`.
    booked: Option<(u64, i64)>,
    settles_on: u64,
    text: &'static str,
}

impl Purchase {
    /// One booked purchase a day, so every export's newest row is the export's own day.
    fn daily_filler(day: u64) -> Self {
        let amount_minor = -900 - day as i64;
        Purchase {
            card: CARD,
            held_on: day,
            held_minor: amount_minor,
            booked: Some((day, amount_minor)),
            settles_on: day,
            text: "LIDL",
        }
    }

    fn is_filler(&self) -> bool {
        self.text == "LIDL"
    }

    /// What an export made on `day` lists for this purchase: its hold while pending, its booked
    /// row once booked, nothing once cancelled or before it was made.
    fn listed_on(&self, day: u64) -> Option<(u64, i64, CardRow)> {
        if self.held_on > day {
            None
        } else if day < self.settles_on {
            Some((self.held_on, self.held_minor, CardRow::Pending))
        } else {
            let (booked_on, booked_minor) = self.booked?;
            Some((booked_on, booked_minor, CardRow::Booked(self.text)))
        }
    }
}

struct Scenario {
    seed: u64,
    purchases: Vec<Purchase>,
    export_days: Vec<u64>,
    newest_first: bool,
}

impl Scenario {
    fn of(seed: u64) -> Self {
        let mut random = Xorshift::seeded(seed, 0x9E37_79B9_7F4A_7C15);
        let mut purchases = Vec::new();
        for day in 0..SCENARIO_DAYS + 3 {
            purchases.push(Purchase::daily_filler(day));
            if day >= SCENARIO_DAYS {
                continue;
            }
            for _ in 0..random.below(3) {
                let card = if random.chance(50) { CARD } else { SECOND_CARD };
                let held_minor = [-50, -50, -50, -1_999, -3_300][random.below(5) as usize];
                let cancelled = random.chance(15);
                let booked_on = day + random.below(3);
                let settles_on = booked_on.max(day + random.below(3));
                let booked_minor = if booked_on == day && held_minor == -3_300 && random.chance(50)
                {
                    -3_299
                } else {
                    held_minor
                };
                purchases.push(Purchase {
                    card,
                    held_on: day,
                    held_minor,
                    booked: (!cancelled).then_some((booked_on, booked_minor)),
                    settles_on: if cancelled {
                        day + 1 + random.below(3)
                    } else {
                        settles_on
                    },
                    text: if held_minor == -50 { "ZSSK" } else { "SHOP" },
                });
            }
        }
        let mut export_days: Vec<u64> = (0..SCENARIO_DAYS + 3)
            .filter(|_| random.chance(60))
            .collect();
        if export_days.is_empty() {
            export_days.push(SCENARIO_DAYS + 2);
        }
        let newest_first = random.chance(50);
        Scenario {
            seed,
            purchases,
            export_days,
            newest_first,
        }
    }

    /// A hold booked at another amount is told apart from the other holds of its card and day
    /// only when it is their only one; with two, the bank's data cannot say which was repriced.
    fn reprices_only_lone_holds(&self) -> bool {
        self.purchases.iter().all(|purchase| {
            let repriced = purchase
                .booked
                .is_some_and(|(_, booked_minor)| booked_minor != purchase.held_minor);
            !repriced
                || self
                    .purchases
                    .iter()
                    .filter(|other| {
                        !other.is_filler()
                            && other.card == purchase.card
                            && other.held_on == purchase.held_on
                    })
                    .count()
                    == 1
        })
    }

    /// The export made on `day` and the balance the bank showed with it.
    fn export(&self, day: u64) -> (Vec<IncomingRowInput>, i64) {
        let mut listed: Vec<_> = self
            .purchases
            .iter()
            .filter_map(|purchase| {
                let (row_day, amount_minor, state) = purchase.listed_on(day)?;
                Some((row_day, amount_minor, purchase.card, state))
            })
            .collect();
        if self.newest_first {
            listed.reverse();
        }
        let balance_minor = listed
            .iter()
            .map(|&(_, amount_minor, ..)| amount_minor)
            .sum();
        let rows = listed
            .into_iter()
            .enumerate()
            .map(|(index, (row_day, amount_minor, card, state))| {
                card_row(index as u32, &iso_day(row_day), amount_minor, card, state)
            })
            .collect();
        (rows, balance_minor)
    }

    /// The purchases an export made on `day` lists, by position.
    fn listed_by(&self, day: u64) -> Vec<usize> {
        (0..self.purchases.len())
            .filter(|&position| self.purchases[position].listed_on(day).is_some())
            .collect()
    }

    /// The export days in a shuffled order of their own.
    fn shuffled_export_days(&self) -> Vec<u64> {
        let mut order = self.export_days.clone();
        let mut random = Xorshift::seeded(self.seed, 0xD1B5_4A32_D192_ED03);
        for i in (1..order.len()).rev() {
            let j = random.below(i as u64 + 1) as usize;
            order.swap(i, j);
        }
        order
    }

    fn failure(&self, what: String, export_days: &[u64]) -> Option<String> {
        let purchases: Vec<String> = self
            .purchases
            .iter()
            .filter(|purchase| !purchase.is_filler())
            .map(|purchase| format!("{purchase:?}"))
            .collect();
        Some(format!(
            "seed {}: {what}\n  exports on {export_days:?}, newest first: {}\n    {}",
            self.seed,
            self.newest_first,
            purchases.join("\n    ")
        ))
    }
}

/// Runs `failure_of` on every scenario of the harness seeds whose repriced holds are their card
/// and day's only ones, `ambiguous` seeds left out, and asserts that none failed.
fn assert_every_scenario(
    ambiguous: &[(u64, &str)],
    failure_of: impl Fn(&Scenario) -> Option<String>,
) {
    let failures: Vec<String> = harness_seeds()
        .filter(|seed| ambiguous.iter().all(|&(left_out, _)| left_out != *seed))
        .map(Scenario::of)
        .filter(Scenario::reprices_only_lone_holds)
        .filter_map(|scenario| failure_of(&scenario))
        .collect();
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn random_export_sequences_match_the_bank_and_older_exports_add_nothing() {
    assert_every_scenario(&[], |scenario| {
        let conn = initialize_in_memory().unwrap();
        let card_account = own_without_iban(&conn, "Card");
        let mut imported_days = Vec::new();
        for &day in &scenario.export_days {
            let (rows, bank_balance_minor) = scenario.export(day);
            import(&conn, card_account, &rows);
            let saldo_minor = account_total(&conn, card_account);
            if saldo_minor != bank_balance_minor {
                return scenario.failure(
                    format!("export of day {day}: Saldo {saldo_minor}, bank {bank_balance_minor}"),
                    &scenario.export_days,
                );
            }
            imported_days.push(day);
            for &earlier in &imported_days {
                let (again, _) = scenario.export(earlier);
                let before = everything(&conn);
                let added = import(&conn, card_account, &again);
                if added != 0 || everything(&conn) != before {
                    return scenario.failure(
                        format!(
                            "after the export of day {day}, the export of day {earlier} again added {added}"
                        ),
                        &scenario.export_days,
                    );
                }
            }
        }
        None
    });
}

#[test]
fn random_export_sequences_keep_a_check_made_between_imports_verified() {
    assert_every_scenario(&[], |scenario| {
        let conn = initialize_in_memory().unwrap();
        let card_account = own_without_iban(&conn, "Card");
        let mut checked = false;
        for &day in &scenario.export_days {
            let (rows, bank_balance_minor) = scenario.export(day);
            if rows.is_empty() {
                continue;
            }
            import(&conn, card_account, &rows);
            let after_the_import = verification_now(&conn, card_account);
            if checked && !matches!(after_the_import, Some(Verification::Verified { .. })) {
                return scenario.failure(
                    format!("export of day {day}: {after_the_import:?}"),
                    &scenario.export_days,
                );
            }
            check_now(&conn, card_account, &iso_day(day + 1), bank_balance_minor);
            checked = true;
            let fresh = verification_now(&conn, card_account);
            if !matches!(fresh, Some(Verification::Verified { .. })) {
                return scenario.failure(
                    format!("export of day {day}: a fresh check {fresh:?}"),
                    &scenario.export_days,
                );
            }
        }
        None
    });
}

#[test]
fn random_export_orders_end_at_the_newest_exports_balance() {
    assert_every_scenario(&[], |scenario| {
        let order = scenario.shuffled_export_days();
        let newest = *order.iter().max().unwrap();
        let conn = initialize_in_memory().unwrap();
        let card_account = own_without_iban(&conn, "Card");
        let mut newest_so_far: Option<u64> = None;
        for &day in order.iter().chain([&newest]) {
            let (rows, _) = scenario.export(day);
            if rows.is_empty() {
                continue;
            }
            let added = import(&conn, card_account, &rows);
            if let Some(newer) = newest_so_far.filter(|&newer| newer > day) {
                let in_newer = scenario.listed_by(newer);
                let gone_since = scenario
                    .listed_by(day)
                    .into_iter()
                    .filter(|position| !in_newer.contains(position))
                    .count() as u32;
                if added > gone_since {
                    return scenario.failure(
                        format!(
                            "the export of day {day} after the one of day {newer} added {added}, only {gone_since} gone since"
                        ),
                        &order,
                    );
                }
            }
            newest_so_far = Some(newest_so_far.map_or(day, |newer| newer.max(day)));
        }
        let (_, bank_balance_minor) = scenario.export(newest);
        let saldo_minor = account_total(&conn, card_account);
        (saldo_minor != bank_balance_minor).then(|| {
            scenario
                .failure(
                    format!("Saldo {saldo_minor}, bank {bank_balance_minor}"),
                    &order,
                )
                .unwrap()
        })
    });
}
