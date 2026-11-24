//! What the random export harnesses share: the seeds they replay, their random numbers and their
//! calendar.

use std::ops::RangeInclusive;

use chrono::{Days, NaiveDate};

/// Seeds 1 to 399, or the ones `SALDO_HARNESS_SEEDS` names: `2400` for 1 to 2400, `887-887` for
/// one seed.
pub(super) fn harness_seeds() -> RangeInclusive<u64> {
    let named = std::env::var("SALDO_HARNESS_SEEDS").unwrap_or_default();
    let (first, last) = named.split_once('-').unwrap_or(("1", &named));
    match (first.parse(), last.parse()) {
        (Ok(first), Ok(last)) => first..=last,
        _ => 1..=399,
    }
}

pub(super) struct Xorshift(u64);

impl Xorshift {
    pub(super) fn seeded(seed: u64, salt: u64) -> Self {
        Xorshift(seed.wrapping_mul(salt) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub(super) fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    pub(super) fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// The scenario's `day`, counted from 1 Oct 2026.
pub(super) fn iso_day(day: u64) -> String {
    let first = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    (first + Days::new(day)).format("%Y-%m-%d").to_string()
}
