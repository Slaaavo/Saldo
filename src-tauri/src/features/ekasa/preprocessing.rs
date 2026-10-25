use std::collections::HashSet;

use chrono::NaiveDateTime;

use super::models::{EkasaReceiptItem, OfflineReceiptData, ProcessedItem};

/// Groups receipt items by name, sums their prices, converts to minor units,
/// and drops zero-amount groups. Order matches first occurrence of each name.
pub fn preprocess_receipt_items(items: &[EkasaReceiptItem]) -> Vec<ProcessedItem> {
    // Collect unique names in first-occurrence order
    let mut seen: HashSet<String> = HashSet::new();
    let mut names: Vec<String> = Vec::new();
    for item in items {
        if seen.insert(item.name.clone()) {
            names.push(item.name.clone());
        }
    }

    names
        .into_iter()
        .filter_map(|name| {
            let total_eur: f64 = items
                .iter()
                .filter(|i| i.name == name)
                .map(|i| i.price)
                .sum();

            let amount_minor = (total_eur * 100.0_f64).round() as i64;
            if amount_minor == 0 {
                return None;
            }

            // vat_rate comes from the first item in the group (string like "20" → 2000 bps)
            let first = items.iter().find(|i| i.name == name)?;
            // unwrap_or(0.0): an unparseable VAT rate is treated as 0 bps (no VAT).
            // This is intentional — an unknown rate should not block import; the user
            // can correct it manually when reviewing the generated expense event.
            let vat_rate_bps = (first.vat_rate * 100.0_f64).round() as i64;

            Some(ProcessedItem {
                name,
                amount_minor,
                vat_rate_bps,
            })
        })
        .collect()
}

/// What a decoded QR text is. Only `OnlineReceiptId` may be sent to the eKasa API.
#[derive(Debug)]
pub enum QrContent {
    OnlineReceiptId(String),
    Offline(OfflineReceiptData),
    NotEkasa,
}

/// Sorts decoded QR text into an online receipt id (bare, or in a `receiptId=` URL),
/// an offline receipt code, or something that is not an eKasa code at all.
pub fn classify_qr_content(qr_content: &str) -> QrContent {
    let text = qr_content.trim();
    if let Some(id) = extract_receipt_id_from_url(text) {
        return QrContent::OnlineReceiptId(id);
    }
    if is_online_receipt_id(text) {
        return QrContent::OnlineReceiptId(text.to_string());
    }
    match parse_offline_qr(text) {
        Some(offline) => QrContent::Offline(offline),
        None => QrContent::NotEkasa,
    }
}

/// Extracts the id from the `receiptId=` query parameter of a QR code URL.
fn extract_receipt_id_from_url(qr_content: &str) -> Option<String> {
    // URL format: ...?receiptId=XXXX[&...]
    let pos = qr_content.find("receiptId=")?;
    let after = &qr_content[pos + "receiptId=".len()..];
    let end = after.find('&').unwrap_or(after.len());
    let id = after[..end].trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// Online receipt id per spec section 3.4.4.4: `[VO]-` followed by 32 hex digits.
/// The `-TEST` ids of the integration environment are not served by the production API.
fn is_online_receipt_id(text: &str) -> bool {
    match text.as_bytes() {
        [b'V' | b'O', b'-', hex @ ..] => hex.len() == 32 && hex.iter().all(u8::is_ascii_hexdigit),
        _ => false,
    }
}

/// Parses an offline eKasa QR code string.
///
/// Expected format (spec section 2.9): `{OKP}:{KP}:{YYMMDDHHMISS}:{sequence}:{total}`
///
/// Returns `None` if the format is unrecognised or any field fails to parse.
pub fn parse_offline_qr(qr_content: &str) -> Option<OfflineReceiptData> {
    let parts: Vec<&str> = qr_content.split(':').collect();
    if parts.len() != 5 {
        return None;
    }

    let event_date = parse_dt_yymmddhhmiss(parts[2])?;
    let total_amount_minor = parse_total_to_minor(parts[4])?;

    Some(OfflineReceiptData {
        event_date,
        total_amount_minor,
    })
}

/// Parses `YYMMDDHHMISS` and returns the date as `YYYY-MM-DD`.
/// Returns `None` for anything that is not a real calendar date and time.
fn parse_dt_yymmddhhmiss(dt_str: &str) -> Option<String> {
    // chrono accepts space-padded fields such as "2612121212 0".
    if dt_str.len() != 12 || !dt_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let dt = NaiveDateTime::parse_from_str(dt_str, "%y%m%d%H%M%S").ok()?;
    Some(dt.date().format("%Y-%m-%d").to_string())
}

/// Largest absolute total the XSD Amount type allows, in minor units.
const MAX_TOTAL_MINOR: i64 = 10_000_000 * 100;

/// Converts a QR total to minor units (cents).
///
/// The total has the format of the XSD `Amount` attribute (spec section 3.3.12.17):
/// 1 to 12 characters, a decimal part rounded to 2 decimal places, range
/// -10000000 to 10000000. A dot and exactly two decimals are required, so `"1250"`
/// is rejected instead of being read as 12.50 or 1250.00.
fn parse_total_to_minor(total_str: &str) -> Option<i64> {
    if total_str.len() > 12 {
        return None;
    }
    let (negative, unsigned) = match total_str.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, total_str),
    };
    let (whole, decimals) = unsigned.split_once('.')?;
    let is_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !is_digits(whole) || decimals.len() != 2 || !is_digits(decimals) {
        return None;
    }

    let minor = whole.parse::<i64>().ok()? * 100 + decimals.parse::<i64>().ok()?;
    if minor > MAX_TOTAL_MINOR {
        return None;
    }
    Some(if negative { -minor } else { minor })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::ekasa::models::EkasaReceiptItem;

    fn item(name: &str, price: f64, vat_rate: f64) -> EkasaReceiptItem {
        EkasaReceiptItem {
            name: name.to_string(),
            quantity: 1.0,
            price,
            vat_rate,
        }
    }

    // -----------------------------------------------------------------------
    // preprocess_receipt_items
    // -----------------------------------------------------------------------

    #[test]
    fn test_merges_items_with_same_name() {
        let items = vec![item("Coffee", 2.50, 20.0), item("Coffee", 1.80, 20.0)];
        let result = preprocess_receipt_items(&items);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Coffee");
        assert_eq!(result[0].amount_minor, 430); // 4.30 EUR = 430 cents
        assert_eq!(result[0].vat_rate_bps, 2000);
    }

    #[test]
    fn test_drops_zero_price_group() {
        let items = vec![item("Discount", 0.0, 20.0), item("Coffee", 2.00, 20.0)];
        let result = preprocess_receipt_items(&items);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Coffee");
    }

    #[test]
    fn test_drops_group_when_prices_cancel_out() {
        // K (positive) + Z (negative discount) summing to zero should be dropped
        let items = vec![item("Item", 5.00, 20.0), item("Item", -5.00, 20.0)];
        let result = preprocess_receipt_items(&items);
        assert!(result.is_empty());
    }

    #[test]
    fn test_multi_group_preserves_first_occurrence_order() {
        let items = vec![
            item("Apple", 1.00, 10.0),
            item("Banana", 2.00, 20.0),
            item("Apple", 0.50, 10.0),
        ];
        let result = preprocess_receipt_items(&items);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "Apple");
        assert_eq!(result[0].amount_minor, 150); // 1.50 EUR
        assert_eq!(result[1].name, "Banana");
        assert_eq!(result[1].amount_minor, 200); // 2.00 EUR
    }

    #[test]
    fn test_decimal_to_minor_conversion() {
        let items = vec![item("Tea", 3.99, 20.0)];
        let result = preprocess_receipt_items(&items);
        assert_eq!(result[0].amount_minor, 399);
    }

    #[test]
    fn test_different_vat_rates_per_group() {
        let items = vec![item("Book", 10.00, 10.0), item("Shirt", 25.00, 20.0)];
        let result = preprocess_receipt_items(&items);
        let book = result.iter().find(|i| i.name == "Book").unwrap();
        let shirt = result.iter().find(|i| i.name == "Shirt").unwrap();
        assert_eq!(book.vat_rate_bps, 1000);
        assert_eq!(shirt.vat_rate_bps, 2000);
    }

    #[test]
    fn test_vat_rate_taken_from_first_item_in_group() {
        // If a data anomaly causes mixed rates in the same group, the first item wins
        let items = vec![item("Misc", 5.00, 10.0), item("Misc", 5.00, 20.0)];
        let result = preprocess_receipt_items(&items);
        assert_eq!(result[0].vat_rate_bps, 1000);
    }

    #[test]
    fn test_empty_input_returns_empty() {
        let result = preprocess_receipt_items(&[]);
        assert!(result.is_empty());
    }

    // -----------------------------------------------------------------------
    // parse_offline_qr
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_offline_qr_valid_decimal_total() {
        // YYMMDDHHmmss: YY=26, MM=04, DD=12
        let qr = "ABCD1234:XYZ9876:260412153045:001:12.50";
        let result = parse_offline_qr(qr);
        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.event_date, "2026-04-12");
        assert_eq!(data.total_amount_minor, 1250);
    }

    #[test]
    fn test_parse_offline_qr_spec_example() {
        let qr =
            "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180213093414:23:237.23";
        let data = parse_offline_qr(qr).unwrap();
        assert_eq!(data.event_date, "2018-02-13");
        assert_eq!(data.total_amount_minor, 23723);
    }

    #[test]
    fn test_parse_offline_qr_total_without_decimal_point_returns_none() {
        // "1250" is not an Amount: the spec requires a decimal part, so it must not
        // be guessed as 1250 minor units (or 1250.00).
        let qr = "ABCD1234:XYZ9876:260412153045:001:1250";
        assert!(parse_offline_qr(qr).is_none());
    }

    #[test]
    fn test_parse_offline_qr_total_must_have_exactly_two_decimals() {
        for total in ["12.5", "12.500", "12.", ".50"] {
            let qr = format!("ABCD1234:XYZ9876:260412153045:001:{total}");
            assert!(parse_offline_qr(&qr).is_none(), "{total:?}");
        }
    }

    #[test]
    fn test_parse_offline_qr_total_rejects_non_amount_text() {
        for total in ["+12.50", "1e2.00", "12,50", "1 2.50", "--1.00", "-.50", ""] {
            let qr = format!("ABCD1234:XYZ9876:260412153045:001:{total}");
            assert!(parse_offline_qr(&qr).is_none(), "{total:?}");
        }
    }

    #[test]
    fn test_parse_offline_qr_total_range_and_sign() {
        let total = |t: &str| {
            parse_offline_qr(&format!("ABCD1234:XYZ9876:260412153045:001:{t}"))
                .map(|d| d.total_amount_minor)
        };
        assert_eq!(total("0.00"), Some(0));
        assert_eq!(total("0.29"), Some(29));
        assert_eq!(total("19.99"), Some(1999));
        assert_eq!(total("-12.50"), Some(-1250));
        assert_eq!(total("10000000.00"), Some(1_000_000_000));
        assert_eq!(total("-10000000.00"), Some(-1_000_000_000));
        assert_eq!(total("10000000.01"), None);
        assert_eq!(total("-10000000.01"), None);
    }

    #[test]
    fn test_parse_offline_qr_total_longer_than_12_chars_returns_none() {
        // 13 characters, although the value itself is in range
        let qr = "ABCD1234:XYZ9876:260412153045:001:0000000012.50";
        assert!(parse_offline_qr(qr).is_none());
    }

    #[test]
    fn test_parse_offline_qr_too_few_parts_returns_none() {
        // Only 3 colon-separated parts — not enough
        let qr = "ABCD1234:XYZ9876:260412153045";
        assert!(parse_offline_qr(qr).is_none());
    }

    #[test]
    fn test_parse_offline_qr_datetime_wrong_length_returns_none() {
        // dt_str must be exactly 12 chars
        let qr = "ABCD1234:XYZ9876:260412:001:12.50";
        assert!(parse_offline_qr(qr).is_none());
    }

    #[test]
    fn test_parse_offline_qr_invalid_total_returns_none() {
        let qr = "ABCD1234:XYZ9876:260412153045:001:notanumber";
        assert!(parse_offline_qr(qr).is_none());
    }

    #[test]
    fn test_parse_offline_qr_empty_string_returns_none() {
        assert!(parse_offline_qr("").is_none());
    }

    #[test]
    fn test_parse_offline_qr_does_not_fall_back_to_ddmmyy() {
        // "260432121200" is not YYMMDD (day 32). Reading it as DDMMYY would give
        // 2032-04-26, which is a guess the spec does not allow.
        let qr = "ABCD1234:XYZ9876:260432121200:001:25.00";
        assert!(parse_offline_qr(qr).is_none());
    }

    #[test]
    fn test_parse_offline_qr_invalid_month_returns_none() {
        let qr = "ABCD1234:XYZ9876:269912121200:001:10.00";
        assert!(parse_offline_qr(qr).is_none());
    }

    #[test]
    fn test_parse_offline_qr_impossible_calendar_dates_return_none() {
        for dt in [
            "260230120000", // Feb 30
            "250229120000", // Feb 29 in a non-leap year
            "260431120000", // Apr 31
            "260400120000", // day 0
            "260013120000", // month 0
            "261301120000", // month 13
        ] {
            let qr = format!("ABCD1234:XYZ9876:{dt}:001:10.00");
            assert!(parse_offline_qr(&qr).is_none(), "{dt:?}");
        }
    }

    #[test]
    fn test_parse_offline_qr_leap_day_is_valid() {
        let qr = "ABCD1234:XYZ9876:240229120000:001:10.00";
        assert_eq!(parse_offline_qr(qr).unwrap().event_date, "2024-02-29");
    }

    #[test]
    fn test_parse_offline_qr_impossible_time_returns_none() {
        for dt in [
            "260412250000", // hour 25
            "260412240000", // hour 24
            "260412126000", // minute 60
            "260412996099",
        ] {
            let qr = format!("ABCD1234:XYZ9876:{dt}:001:10.00");
            assert!(parse_offline_qr(&qr).is_none(), "{dt:?}");
        }
    }

    #[test]
    fn test_parse_offline_qr_end_of_day_time_is_valid() {
        let qr = "ABCD1234:XYZ9876:261231235959:001:10.00";
        assert_eq!(parse_offline_qr(qr).unwrap().event_date, "2026-12-31");
    }

    #[test]
    fn test_parse_offline_qr_non_ascii_datetime_returns_none() {
        // "1é345678901" is 12 bytes, so a byte-length check passes but slicing [0..2] would
        // cut the multi-byte 'é' in half.
        let qr = "ABCD1234:XYZ9876:1é345678901:001:10.00";
        assert!(parse_offline_qr(qr).is_none());
    }

    #[test]
    fn test_parse_offline_qr_non_digit_datetime_returns_none() {
        for dt in ["26121212120a", "2612121212 0", "-26121212120"] {
            let qr = format!("ABCD1234:XYZ9876:{dt}:001:10.00");
            assert!(parse_offline_qr(&qr).is_none(), "{dt:?}");
        }
    }

    #[test]
    fn test_parse_offline_qr_requires_exactly_five_parts() {
        let qr = "ABCD1234:XYZ9876:260412153045:001:12.50:extra";
        assert!(parse_offline_qr(qr).is_none());
    }

    // -----------------------------------------------------------------------
    // classify_qr_content
    // -----------------------------------------------------------------------

    const ONLINE_ID: &str = "O-7DBCDA8A56EE426DBCDA8A56EE426D1A";
    const OFFLINE_CODE: &str =
        "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180213093414:23:237.23";

    fn online_id_of(qr: &str) -> Option<String> {
        match classify_qr_content(qr) {
            QrContent::OnlineReceiptId(id) => Some(id),
            _ => None,
        }
    }

    #[test]
    fn test_classify_bare_online_receipt_id() {
        assert_eq!(online_id_of(ONLINE_ID).as_deref(), Some(ONLINE_ID));
        let lower = "V-7dbcda8a56ee426dbcda8a56ee426d1a";
        assert_eq!(online_id_of(lower).as_deref(), Some(lower));
    }

    #[test]
    fn test_classify_trims_surrounding_whitespace() {
        let qr = format!("  {ONLINE_ID}\r\n");
        assert_eq!(online_id_of(&qr).as_deref(), Some(ONLINE_ID));
    }

    #[test]
    fn test_classify_receipt_id_url() {
        let qr = format!("https://ekasa.financnasprava.sk/receipt?receiptId={ONLINE_ID}&lang=sk");
        assert_eq!(online_id_of(&qr).as_deref(), Some(ONLINE_ID));
    }

    #[test]
    fn test_classify_offline_code() {
        match classify_qr_content(OFFLINE_CODE) {
            QrContent::Offline(data) => {
                assert_eq!(data.event_date, "2018-02-13");
                assert_eq!(data.total_amount_minor, 23723);
            }
            other => panic!("expected an offline code, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_other_text_is_not_ekasa() {
        for qr in [
            "",
            "   ",
            "hello",
            "https://example.com/menu",
            "https://example.com/?receiptId=",
            "WIFI:T:WPA;S:cafe;P:secret;;",
            "8590123456789",
            "O-7DBCDA8A56EE426DBCDA8A56EE4-TEST", // integration environment id
            "O-7DBCDA8A56EE426DBCDA8A56EE426D1",  // 31 hex digits
            "O-7DBCDA8A56EE426DBCDA8A56EE426D1AB", // 33 hex digits
            "O-7DBCDA8A56EE426DBCDA8A56EE426D1G", // not hex
            "X-7DBCDA8A56EE426DBCDA8A56EE426D1A", // unknown prefix
            "7DBCDA8A56EE426DBCDA8A56EE426D1A",   // no prefix
        ] {
            assert!(
                matches!(classify_qr_content(qr), QrContent::NotEkasa),
                "{qr:?}"
            );
        }
    }

    #[test]
    fn test_classify_malformed_offline_code_is_not_ekasa() {
        for qr in [
            "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180230093414:23:237.23",
            "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180213093414:23:23723",
            "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180213093414:23",
        ] {
            assert!(
                matches!(classify_qr_content(qr), QrContent::NotEkasa),
                "{qr:?}"
            );
        }
    }
}
