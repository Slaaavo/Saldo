//! Random card purchases, each held and then booked (a day or two later, sometimes at another
//! amount) or cancelled, exported by the bank at random days, newest row first or last. Each
//! harness imports the exports with the review's defaults and compares Saldo with the bank.

use std::thread::sleep;
use std::time::Duration;

use chrono::{Days, Local, NaiveDate};

use super::*;
use crate::features::bank::Verification;
use crate::features::transactions::commands::get_accounts_snapshot_inner;
use crate::features::transactions::repository::{GetSnapshotParams, Moment};

const SEEDS: std::ops::Range<u64> = 1..400;
const SCENARIO_DAYS: u64 = 8;
/// Long enough for `created_at` and a check's `checked_at`, both in milliseconds, to order the
/// writes.
const CLOCK_TICK: Duration = Duration::from_millis(1);

struct Xorshift(u64);

impl Xorshift {
    fn seeded(seed: u64, salt: u64) -> Self {
        Xorshift(seed.wrapping_mul(salt) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

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

fn iso_day(day: u64) -> String {
    let first = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    (first + Days::new(day)).format("%Y-%m-%d").to_string()
}

/// Runs `failure_of` on every scenario of `SEEDS` whose repriced holds are their card and day's
/// only ones, and asserts that none failed.
fn assert_every_scenario(failure_of: impl Fn(&Scenario) -> Option<String>) {
    let failures: Vec<String> = SEEDS
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

fn verification_now(conn: &Connection, account_id: i64) -> Option<Verification> {
    get_accounts_snapshot_inner(
        conn,
        GetSnapshotParams {
            moment: Moment::live(Local::now().date_naive()),
            person_id: None,
        },
    )
    .unwrap()
    .rows
    .into_iter()
    .find(|row| row.account_id == account_id)
    .and_then(|row| row.verification)
}

fn check_now(conn: &Connection, account_id: i64, day: &str, bank_balance_minor: i64) {
    conn.execute(
        "INSERT INTO balance_check (account_id, day, bank_balance_minor) VALUES (?1, ?2, ?3)",
        params![account_id, day, bank_balance_minor],
    )
    .unwrap();
}

#[test]
fn random_export_sequences_match_the_bank_and_older_exports_add_nothing() {
    assert_every_scenario(|scenario| {
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
    assert_every_scenario(|scenario| {
        let conn = initialize_in_memory().unwrap();
        let card_account = own_without_iban(&conn, "Card");
        let mut checked = false;
        for &day in &scenario.export_days {
            let (rows, bank_balance_minor) = scenario.export(day);
            if rows.is_empty() {
                continue;
            }
            sleep(CLOCK_TICK);
            import(&conn, card_account, &rows);
            let after_the_import = verification_now(&conn, card_account);
            if checked && !matches!(after_the_import, Some(Verification::Verified { .. })) {
                return scenario.failure(
                    format!("export of day {day}: {after_the_import:?}"),
                    &scenario.export_days,
                );
            }
            sleep(CLOCK_TICK);
            check_now(&conn, card_account, &iso_day(day + 1), bank_balance_minor);
            checked = true;
            sleep(CLOCK_TICK);
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
    assert_every_scenario(|scenario| {
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
