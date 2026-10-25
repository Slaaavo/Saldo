use chrono::NaiveDate;

const FETCHED_RATE_STALE_AFTER_DAYS: i64 = 7;
const TYPED_PRICE_STALE_AFTER_DAYS: i64 = 31;

pub fn stale_after_days(is_custom: bool) -> i64 {
    if is_custom {
        TYPED_PRICE_STALE_AFTER_DAYS
    } else {
        FETCHED_RATE_STALE_AFTER_DAYS
    }
}

pub fn rate_age_days(rate_date: NaiveDate, reference: NaiveDate) -> i64 {
    (reference - rate_date).num_days().max(0)
}

pub fn is_stale(age_days: i64, is_custom: bool) -> bool {
    age_days > stale_after_days(is_custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: &str = "2026-10-25";

    fn day(date: &str) -> NaiveDate {
        NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()
    }

    fn stale_today(rate_date: &str, is_custom: bool) -> bool {
        is_stale(rate_age_days(day(rate_date), day(TODAY)), is_custom)
    }

    #[test]
    fn a_standard_rate_is_fresh_at_7_days_and_stale_at_8() {
        assert!(!stale_today("2026-10-18", false));
        assert!(stale_today("2026-10-17", false));
    }

    #[test]
    fn a_custom_unit_price_is_fresh_at_31_days_and_stale_at_32() {
        assert!(!stale_today("2026-09-24", true));
        assert!(stale_today("2026-09-23", true));
    }

    #[test]
    fn age_counts_whole_days_back_from_the_reference_day() {
        assert_eq!(rate_age_days(day("2026-10-10"), day(TODAY)), 15);
        assert_eq!(rate_age_days(day(TODAY), day(TODAY)), 0);
    }

    #[test]
    fn a_rate_dated_after_the_reference_day_is_0_days_old() {
        assert_eq!(rate_age_days(day("2026-10-30"), day(TODAY)), 0);
    }

    #[test]
    fn the_limit_is_7_days_for_standard_rates_and_31_for_custom_units() {
        assert_eq!(stale_after_days(false), 7);
        assert_eq!(stale_after_days(true), 31);
    }
}
