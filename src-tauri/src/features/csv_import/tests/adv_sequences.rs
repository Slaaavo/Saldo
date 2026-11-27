//! The export sequences of `export_sequences.rs`, extended: purchases made after the day's export,
//! repriced holds that share their card and day, every amount repriced, splits chosen in the
//! review, the user's note and date edits between imports, holds booked up to 5 days after their
//! day, holds the bank drops from its exports for a few days and then books, and a check that
//! every posted entry pairs a hold and a booking of one purchase.

use super::harness::{harness_seeds, iso_day, Xorshift};
use super::*;

const SCENARIO_DAYS: u64 = 8;

#[derive(Clone, Copy)]
struct Knobs {
    /// Up to one less purchase a day.
    per_day: u64,
    /// The chance a purchase is made after the day's export, so the next day's export lists it.
    after_export_pct: u64,
    /// The chance a purchase booked on its own day is booked 0.01 to 0.03 above its held amount.
    reprice_pct: u64,
    /// The chance the review splits in two a new pending row of 10.00 or more, or a row of 10.00
    /// or more that replaces a split hold.
    split_pct: u64,
    /// The chance the user rewrites a note after an import.
    edit_pct: u64,
    /// The chance the user moves an entry a day later after an import.
    date_edit_pct: u64,
    /// The most days after its hold the bank books a purchase.
    max_delay: u64,
    /// The chance the bank lists a booked purchase's hold on its day, then nowhere until it lists
    /// the booking, 2 to 5 days after the hold.
    drop_out_pct: u64,
}

impl Knobs {
    const DEFAULT: Knobs = Knobs {
        per_day: 4,
        after_export_pct: 25,
        reprice_pct: 30,
        split_pct: 30,
        edit_pct: 20,
        date_edit_pct: 20,
        max_delay: 2,
        drop_out_pct: 0,
    };

    const WITHOUT_REPRICES: Knobs = Knobs {
        reprice_pct: 0,
        ..Knobs::DEFAULT
    };

    const BOOKED_LATE: Knobs = Knobs {
        max_delay: NEAR_DAYS,
        ..Knobs::DEFAULT
    };

    const DROPPED_OUT: Knobs = Knobs {
        drop_out_pct: 40,
        ..Knobs::DEFAULT
    };
}

const NEAR_DAYS: u64 = 5;

#[derive(Debug, Clone)]
struct Purchase {
    card: &'static str,
    held_on: u64,
    held_minor: i64,
    booked: Option<(u64, i64)>,
    settles_on: u64,
    /// The first export day that lists it: the next day when it was made after that day's export.
    listed_from: u64,
    text: &'static str,
    /// The export days from the first to before the second that list it nowhere.
    absent: Option<(u64, u64)>,
}

impl Purchase {
    fn daily_filler(day: u64) -> Self {
        let amount_minor = -900 - day as i64;
        Purchase {
            card: CARD,
            held_on: day,
            held_minor: amount_minor,
            booked: Some((day, amount_minor)),
            settles_on: day,
            listed_from: day,
            text: "LIDL",
            absent: None,
        }
    }

    fn is_filler(&self) -> bool {
        self.text == "LIDL"
    }

    fn is_repriced(&self) -> bool {
        self.booked
            .is_some_and(|(_, booked_minor)| booked_minor != self.held_minor)
    }

    fn listed_on(&self, day: u64) -> Option<(u64, i64, CardRow)> {
        if self.held_on > day
            || day < self.listed_from
            || self
                .absent
                .is_some_and(|(from, to)| (from..to).contains(&day))
        {
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
    knobs: Knobs,
    purchases: Vec<Purchase>,
    export_days: Vec<u64>,
    newest_first: bool,
}

impl Scenario {
    fn of(seed: u64, knobs: Knobs) -> Self {
        let mut random = Xorshift::seeded(seed, 0x9E37_79B9_7F4A_7C15);
        let mut purchases = Vec::new();
        let days = SCENARIO_DAYS + knobs.max_delay + 1;
        for day in 0..days {
            purchases.push(Purchase::daily_filler(day));
            if day >= SCENARIO_DAYS {
                continue;
            }
            for _ in 0..random.below(knobs.per_day) {
                let card = if random.chance(50) { CARD } else { SECOND_CARD };
                let held_minor = [-50, -50, -50, -1_999, -3_300, -500][random.below(6) as usize];
                let cancelled = random.chance(15);
                let booked_on = day + random.below(knobs.max_delay + 1);
                let settles_on = booked_on.max(day + random.below(knobs.max_delay + 2));
                let booked_minor = if booked_on == day && random.chance(knobs.reprice_pct) {
                    held_minor + 1 + random.below(3) as i64
                } else {
                    held_minor
                };
                let settles_on = if cancelled {
                    day + 1 + random.below(3)
                } else {
                    settles_on
                };
                purchases.push(Purchase {
                    card,
                    held_on: day,
                    held_minor,
                    booked: (!cancelled).then_some((booked_on, booked_minor)),
                    settles_on,
                    listed_from: day + u64::from(random.chance(knobs.after_export_pct)),
                    text: if held_minor == -50 { "ZSSK" } else { "SHOP" },
                    absent: None,
                });
            }
        }
        let mut export_days: Vec<u64> = (0..days).filter(|_| random.chance(60)).collect();
        if export_days.is_empty() {
            export_days.push(days - 1);
        }
        let mut dropping = Xorshift::seeded(seed, 0x5851_F42D_4C95_7F2D);
        for purchase in &mut purchases {
            if purchase.is_filler()
                || purchase.booked.is_none()
                || !dropping.chance(knobs.drop_out_pct)
            {
                continue;
            }
            let listed_booked_from = purchase.held_on + 2 + dropping.below(4);
            purchase.listed_from = purchase.held_on;
            purchase.settles_on = listed_booked_from;
            purchase.absent = Some((purchase.held_on + 1, listed_booked_from));
        }
        Scenario {
            seed,
            knobs,
            purchases,
            export_days,
            newest_first: random.chance(50),
        }
    }

    /// Every repriced hold can be told apart from the other holds of its card and day: each of
    /// them is booked at its own amount on a later day than the hold's day.
    fn reprices_are_unambiguous(&self) -> bool {
        self.purchases.iter().all(|purchase| {
            !purchase.is_repriced()
                || self
                    .purchases
                    .iter()
                    .filter(|other| {
                        !std::ptr::eq(*other, purchase)
                            && !other.is_filler()
                            && other.card == purchase.card
                            && other.held_on == purchase.held_on
                    })
                    .all(|other| {
                        other.booked.is_some_and(|(booked_on, booked_minor)| {
                            booked_minor == other.held_minor && booked_on > other.held_on
                        })
                    })
        })
    }

    fn export(&self, day: u64) -> (Vec<IncomingRowInput>, i64) {
        let mut listed: Vec<_> = self
            .purchases
            .iter()
            .filter_map(|purchase| {
                let (row_day, amount_minor, state) = purchase.listed_on(day)?;
                Some((row_day, amount_minor, purchase.card, state))
            })
            .collect();
        listed.sort_by_key(|&(row_day, ..)| row_day);
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

    fn shuffled_export_days(&self) -> Vec<u64> {
        let mut order = self.export_days.clone();
        let mut random = Xorshift::seeded(self.seed, 0xD1B5_4A32_D192_ED03);
        for i in (1..order.len()).rev() {
            let j = random.below(i as u64 + 1) as usize;
            order.swap(i, j);
        }
        order
    }

    fn failure(&self, what: String) -> String {
        let purchases: Vec<String> = self
            .purchases
            .iter()
            .filter(|purchase| !purchase.is_filler())
            .map(|purchase| format!("{purchase:?}"))
            .collect();
        format!(
            "seed {}: {what}\n  exports on {:?}, newest first: {}\n    {}",
            self.seed,
            self.export_days,
            self.newest_first,
            purchases.join("\n    ")
        )
    }
}

fn state_of(conn: &Connection) -> Vec<String> {
    let mut all = everything(conn);
    let mut stmt = conn
        .prepare(
            "SELECT 'replaces ' || event_id || ' ' || IFNULL(replaces_event_id, '-') FROM bank_entry
             UNION ALL SELECT 'check ' || id || ' ' || bank_balance_minor FROM balance_check
             ORDER BY 1",
        )
        .unwrap();
    let more: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    all.extend(more);
    all
}

/// Imports with the review's defaults, except that a new pending row of 10.00 or more, or a row of
/// 10.00 or more that replaces a split hold, is split in two with `split_pct` chance.
fn import_choosing(
    conn: &Connection,
    account_id: i64,
    rows: &[IncomingRowInput],
    random: &mut Xorshift,
    split_pct: u64,
) -> CsvImportResult {
    let reviewed = preview(conn, account_id, rows);
    let choices = default_choices(&reviewed)
        .into_iter()
        .zip(rows)
        .zip(&reviewed.rows)
        .map(|((choice, row), previewed)| {
            let splittable = match previewed.verdict {
                Verdict::New => row.pending,
                Verdict::NewReplacingSplitHold { .. } => true,
                _ => false,
            };
            if splittable && row.amount_minor <= -1_000 && random.chance(split_pct) {
                split_choice(
                    choice.index,
                    &[(row.amount_minor + 300, None), (-300, None)],
                )
            } else {
                choice
            }
        })
        .collect();
    apply_reviewed(conn, account_id, rows, &reviewed, choices, true).unwrap()
}

/// The posted entries that do not pair a hold and a booking of one purchase, by card, held amount
/// and booked amount.
fn misposted(conn: &Connection, scenario: &Scenario) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT be.event_id, be.card, be.held_day, be.held_amount_minor, be.amount_minor
             FROM bank_entry be
             JOIN event e ON e.id = be.event_id
             WHERE be.posted_at IS NOT NULL AND e.deleted_at IS NULL",
        )
        .unwrap();
    let posted: Vec<(i64, Option<String>, String, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    posted
        .into_iter()
        .filter(|(_, card, _, held_amount, booked_amount)| {
            !scenario.purchases.iter().any(|purchase| {
                Some(purchase.card) == card.as_deref()
                    && purchase.held_minor == *held_amount
                    && purchase.booked.map(|(_, amount)| amount) == Some(*booked_amount)
            })
        })
        .map(|(event_id, card, held_day, held_amount, booked_amount)| {
            format!(
                "event {event_id}: {card:?} held {held_day} {held_amount}, booked {booked_amount}"
            )
        })
        .collect()
}

/// Holds an import removed as cancelled, per card and held amount, beyond the purchases of that
/// card and amount the bank cancelled.
fn wrongly_removed(conn: &Connection, scenario: &Scenario) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT be.card, be.amount_minor, COUNT(*) FROM bank_entry be
             WHERE be.cancelled_at IS NOT NULL GROUP BY be.card, be.amount_minor",
        )
        .unwrap();
    let removed: Vec<(Option<String>, i64, usize)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    removed
        .into_iter()
        .filter_map(|(card, amount_minor, count)| {
            let cancelled = scenario
                .purchases
                .iter()
                .filter(|purchase| {
                    Some(purchase.card) == card.as_deref()
                        && purchase.held_minor == amount_minor
                        && purchase.booked.is_none()
                })
                .count();
            (count > cancelled).then(|| {
                format!(
                    "{count} holds of {card:?} {amount_minor} removed, {cancelled} cancelled at the bank"
                )
            })
        })
        .collect()
}

/// Imports the exports in their order, checks Saldo against the bank after each, imports every
/// export so far again, edits a note now and then, and checks the bank app's balance.
fn replay_in_order(scenario: &Scenario) -> Option<String> {
    let Knobs {
        split_pct,
        edit_pct,
        date_edit_pct,
        after_export_pct,
        drop_out_pct,
        ..
    } = scenario.knobs;
    // A hold the bank drops is removed by the next export, and a dropped hold's booking can post
    // another hold of its card and amount, so a check made in between can break: the data of that
    // day fits both stories with the same balance. A dropped-out run asserts only fresh checks.
    let holds_drop_out = drop_out_pct > 0;
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let mut random = Xorshift::seeded(scenario.seed, 0xA24B_AED4_963E_E407);
    let mut imported_days = Vec::new();
    let mut checked = false;
    for &day in &scenario.export_days {
        let (rows, bank_balance_minor) = scenario.export(day);
        if rows.is_empty() {
            continue;
        }
        import_choosing(&conn, card_account, &rows, &mut random, split_pct);
        let saldo_minor = account_total(&conn, card_account);
        if saldo_minor != bank_balance_minor {
            return Some(format!(
                "export of day {day}: Saldo {saldo_minor}, bank {bank_balance_minor}"
            ));
        }
        if checked && !holds_drop_out && !is_verified(&conn, card_account) {
            return Some(format!(
                "export of day {day}: {:?}",
                verification_now(&conn, card_account)
            ));
        }
        let wrong = misposted(&conn, scenario);
        if !wrong.is_empty() {
            return Some(format!("export of day {day} misposted {wrong:?}"));
        }
        let removed = wrongly_removed(&conn, scenario);
        if !holds_drop_out && !removed.is_empty() {
            return Some(format!("export of day {day}: {removed:?}"));
        }
        imported_days.push(day);
        for &earlier in &imported_days {
            let (again, _) = scenario.export(earlier);
            let before = state_of(&conn);
            let result = import_choosing(&conn, card_account, &again, &mut random, 0);
            if result.imported != 0 || state_of(&conn) != before {
                return Some(format!(
                    "after day {day}, the export of day {earlier} again: {result:?}"
                ));
            }
        }
        if checked && !holds_drop_out && !is_verified(&conn, card_account) {
            return Some(format!(
                "day {day}, after the replays: {:?}",
                verification_now(&conn, card_account)
            ));
        }
        if random.chance(edit_pct) {
            edit_an_entry(&conn, &mut random, |current| UpdateEventParams {
                note: Some("the user's own words".to_owned()),
                ..current
            });
        }
        if random.chance(date_edit_pct) {
            edit_an_entry(&conn, &mut random, |current| UpdateEventParams {
                event_date: next_day(&current.event_date),
                ..current
            });
        }
        let after_the_export = u64::from(after_export_pct == 0);
        check_now(
            &conn,
            card_account,
            &iso_day(day + after_the_export),
            bank_balance_minor,
        );
        checked = true;
        if !is_verified(&conn, card_account) {
            return Some(format!(
                "day {day}: a fresh check {:?}",
                verification_now(&conn, card_account)
            ));
        }
    }
    None
}

/// The user edits a random live entry that is not part of a split, as `edit` says.
fn edit_an_entry(
    conn: &Connection,
    random: &mut Xorshift,
    edit: impl FnOnce(UpdateEventParams) -> UpdateEventParams,
) {
    let ids = event_ids(conn);
    if ids.is_empty() {
        return;
    }
    let id = ids[random.below(ids.len() as u64) as usize];
    let current = entry(conn, id);
    let in_split: bool = conn
        .query_row(
            "SELECT split_group_id IS NOT NULL FROM event WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .unwrap();
    if !in_split {
        let edited = edit(UpdateEventParams {
            event_id: id,
            amount_minor: current.amount_minor,
            event_date: current.event_date,
            note: current.note,
        });
        update_event(conn, edited).unwrap();
    }
}

fn next_day(event_date: &str) -> String {
    let day = chrono::NaiveDate::parse_from_str(&event_date[..10], "%Y-%m-%d").unwrap();
    format!("{}{}", day.succ_opt().unwrap(), &event_date[10..])
}

/// The live entries created after `since`, a split group counted once.
fn entries_created_after(conn: &Connection, since: i64) -> usize {
    conn.query_row(
        "SELECT COUNT(DISTINCT IFNULL('s' || split_group_id, 'e' || id)) FROM event
         WHERE deleted_at IS NULL AND id > ?1",
        params![since],
        |row| row.get(0),
    )
    .unwrap()
}

fn newest_event_id(conn: &Connection) -> i64 {
    conn.query_row("SELECT IFNULL(MAX(id), 0) FROM event", [], |row| row.get(0))
        .unwrap()
}

/// Imports the exports in a shuffled order, then the newest again, compares the balance, and
/// imports every export again in the same order, which adds and changes nothing. An export older
/// than one imported before it adds at most the purchases it lists that the newer one does not.
fn replay_shuffled(scenario: &Scenario) -> Option<String> {
    let order = scenario.shuffled_export_days();
    let newest = *order.iter().max().unwrap();
    let conn = initialize_in_memory().unwrap();
    let card_account = own_without_iban(&conn, "Card");
    let mut random = Xorshift::seeded(scenario.seed, 0xA24B_AED4_963E_E407);
    let mut newest_so_far: Option<u64> = None;
    for &day in order.iter().chain([&newest]) {
        let (rows, _) = scenario.export(day);
        if rows.is_empty() {
            continue;
        }
        let since = newest_event_id(&conn);
        import_choosing(
            &conn,
            card_account,
            &rows,
            &mut random,
            scenario.knobs.split_pct,
        );
        if let Some(newer) = newest_so_far.filter(|&newer| newer > day) {
            let gone_since = scenario
                .purchases
                .iter()
                .filter(|purchase| {
                    purchase.listed_on(day).is_some() && purchase.listed_on(newer).is_none()
                })
                .count();
            let added = entries_created_after(&conn, since);
            if added > gone_since {
                return Some(format!(
                    "order {order:?}: the export of day {day} after the one of day {newer} added \
                     {added}, only {gone_since} gone since"
                ));
            }
        }
        newest_so_far = Some(newest_so_far.map_or(day, |newer| newer.max(day)));
    }
    let (_, bank_balance_minor) = scenario.export(newest);
    let saldo_minor = account_total(&conn, card_account);
    if saldo_minor != bank_balance_minor {
        return Some(format!(
            "order {order:?}: Saldo {saldo_minor}, bank {bank_balance_minor}"
        ));
    }
    for &day in &order {
        let (rows, _) = scenario.export(day);
        let before = state_of(&conn);
        let result = import_choosing(&conn, card_account, &rows, &mut random, 0);
        if result.imported != 0 || state_of(&conn) != before {
            return Some(format!(
                "order {order:?}: the export of day {day} again: {result:?}"
            ));
        }
    }
    None
}

/// Replays every scenario of the harness seeds whose reprices are unambiguous, `ambiguous` seeds
/// left out, and asserts that none failed.
fn assert_every_scenario(
    knobs: Knobs,
    ambiguous: &[(u64, &str)],
    replay: impl Fn(&Scenario) -> Option<String>,
) {
    let mut failed_seeds = Vec::new();
    let failures: Vec<String> = harness_seeds()
        .filter(|seed| ambiguous.iter().all(|&(left_out, _)| left_out != *seed))
        .map(|seed| Scenario::of(seed, knobs))
        .filter(Scenario::reprices_are_unambiguous)
        .filter_map(|scenario| {
            let failure = replay(&scenario)?;
            failed_seeds.push(scenario.seed);
            Some(scenario.failure(failure))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} failures, seeds {failed_seeds:?}:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn random_exports_with_late_purchases_splits_and_edits_follow_the_bank() {
    assert_every_scenario(
        Knobs::DEFAULT,
        &[(
            1901,
            "A -19.99 hold of 2 Oct is booked on 3 Oct, a -33.00 hold of 3 Oct is cancelled, and a \
             -19.99 purchase no export listed pending is booked on 5 Oct, all on one card. The \
             bank's data fits the -33.00 hold booked at -19.99 on its day and the -19.99 hold \
             booked on 5 Oct as well, and both stories end at the bank's balance.",
        )],
        replay_in_order,
    );
}

#[test]
fn random_exports_with_late_purchases_and_no_reprices_follow_the_bank() {
    assert_every_scenario(
        Knobs::WITHOUT_REPRICES,
        &[(
            379,
            "A -33.00 hold of 7 Oct is cancelled, and a -19.99 purchase no export listed pending \
             is booked on 7 Oct on the same card. The bank's data fits the hold booked at -19.99 \
             as well, and both stories end at the bank's balance.",
        )],
        replay_in_order,
    );
}

#[test]
fn random_exports_with_holds_booked_up_to_five_days_later_follow_the_bank() {
    assert_every_scenario(Knobs::BOOKED_LATE, &[], replay_in_order);
}

#[test]
fn random_exports_with_holds_booked_up_to_five_days_later_in_any_order_end_at_the_newest_balance() {
    assert_every_scenario(
        Knobs::BOOKED_LATE,
        &[(
            178,
            "Three -0.50 holds of 6 Oct, one of them cancelled, and a -0.50 purchase of 7 Oct no \
             imported export listed pending yet, all on one card. The 12 Oct export fits the \
             cancelled hold booked as the 7 Oct purchase as well, with the same balance. The \
             older 9 Oct export, imported after it, then lists the 7 Oct hold, which stays until \
             a newer export removes it.",
        )],
        replay_shuffled,
    );
}

#[test]
fn random_exports_with_holds_the_bank_drops_until_it_books_them_follow_the_bank() {
    assert_every_scenario(Knobs::DROPPED_OUT, &[], replay_in_order);
}

#[test]
fn random_exports_with_holds_the_bank_drops_until_it_books_them_in_any_order_end_at_the_newest_balance(
) {
    assert_every_scenario(Knobs::DROPPED_OUT, &[], replay_shuffled);
}

#[test]
fn random_exports_with_late_purchases_in_any_order_end_at_the_newest_balance() {
    assert_every_scenario(Knobs::DEFAULT, &[], replay_shuffled);
}

#[test]
fn random_exports_with_late_purchases_and_no_reprices_in_any_order_end_at_the_newest_balance() {
    assert_every_scenario(Knobs::WITHOUT_REPRICES, &[], replay_shuffled);
}
