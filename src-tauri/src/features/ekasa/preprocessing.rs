use chrono::NaiveDateTime;

use super::models::{EkasaReceiptItem, OfflineReceiptData, ReceiptReadLine};
use crate::features::transactions::SourceRef;

pub fn preprocess_receipt_items(items: &[EkasaReceiptItem]) -> Vec<ReceiptReadLine> {
    let mut groups: Vec<(&str, i64, f64)> = Vec::new();
    for item in items {
        // vat_rate is a percent: 20.0 is 2000 bps.
        let vat_rate_bps = (item.vat_rate * 100.0_f64).round() as i64;
        match groups
            .iter_mut()
            .find(|(name, rate, _)| *name == item.name && *rate == vat_rate_bps)
        {
            Some((_, _, total_eur)) => *total_eur += item.price,
            None => groups.push((&item.name, vat_rate_bps, item.price)),
        }
    }
    groups
        .into_iter()
        .filter_map(|(name, vat_rate_bps, total_eur)| {
            let gross_minor = (total_eur * 100.0_f64).round() as i64;
            (gross_minor != 0).then(|| ReceiptReadLine {
                name: name.to_owned(),
                gross_minor,
                vat_rate_bps,
            })
        })
        .collect()
}

/// An eKasa receipt code. Only `parse` builds one, so its text always has an eKasa code's shape
/// before it reaches a reference, a stored row or a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptCode {
    text: String,
    offline: Option<OfflineReceiptData>,
}

impl ReceiptCode {
    /// An online receipt id (bare, or in a `receiptId=` URL) or an offline receipt code; None
    /// for any other text. An online id is kept in upper case, so the QR code's id and the same
    /// id typed in lower case give one reference.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if let Some(receipt_id) = receipt_id_in_url(text) {
            return Self::online(receipt_id);
        }
        Self::online(text).or_else(|| {
            parse_offline_qr(text).map(|receipt| Self {
                text: text.to_owned(),
                offline: Some(receipt),
            })
        })
    }

    fn online(receipt_id: &str) -> Option<Self> {
        let receipt_id = receipt_id.to_ascii_uppercase();
        is_online_receipt_id(&receipt_id).then_some(Self {
            text: receipt_id,
            offline: None,
        })
    }

    /// The online receipt id, or the offline receipt's whole code.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The date and total an offline code holds; None for an online receipt id.
    pub fn offline(&self) -> Option<&OfflineReceiptData> {
        self.offline.as_ref()
    }

    pub fn source_ref(&self) -> SourceRef {
        match self.offline {
            None => SourceRef::ekasa_online(&self.text),
            Some(_) => SourceRef::ekasa_offline(&self.text),
        }
    }
}

/// The value of the `receiptId=` query parameter of a QR code URL.
fn receipt_id_in_url(qr_content: &str) -> Option<&str> {
    let (_, after) = qr_content.split_once("receiptId=")?;
    let end = after.find('&').unwrap_or(after.len());
    Some(after[..end].trim())
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
/// Expected format (spec section 2.9): `{OKP}:{KP}:{YYMMDDHHMISS}:{sequence}:{total}`. The OKP
/// is hex digits and dashes, the cash register code (KP) letters and digits, and the sequence
/// number digits.
///
/// Returns `None` if the format is unrecognised or any field fails to parse.
pub fn parse_offline_qr(qr_content: &str) -> Option<OfflineReceiptData> {
    let parts: Vec<&str> = qr_content.split(':').collect();
    let [okp, cash_register, issued_at, sequence, total] = parts[..] else {
        return None;
    };
    let made_of =
        |part: &str, allowed: fn(u8) -> bool| !part.is_empty() && part.bytes().all(allowed);
    if !made_of(okp, |b| b.is_ascii_hexdigit() || b == b'-')
        || !made_of(cash_register, |b| b.is_ascii_alphanumeric())
        || !made_of(sequence, |b| b.is_ascii_digit())
    {
        return None;
    }

    let event_date = parse_dt_yymmddhhmiss(issued_at)?;
    let total_amount_minor = parse_total_to_minor(total)?;

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
        assert_eq!(result[0].gross_minor, 430);
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
        assert_eq!(result[0].gross_minor, 150);
        assert_eq!(result[1].name, "Banana");
        assert_eq!(result[1].gross_minor, 200);
    }

    #[test]
    fn test_decimal_to_minor_conversion() {
        let items = vec![item("Tea", 3.99, 20.0)];
        let result = preprocess_receipt_items(&items);
        assert_eq!(result[0].gross_minor, 399);
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
    fn test_one_name_at_two_vat_rates_is_two_lines() {
        let items = vec![
            item("Misc", 5.00, 10.0),
            item("Misc", 4.00, 20.0),
            item("Misc", 1.00, 10.0),
        ];
        let result = preprocess_receipt_items(&items);
        let lines: Vec<(&str, i64, i64)> = result
            .iter()
            .map(|l| (l.name.as_str(), l.gross_minor, l.vat_rate_bps))
            .collect();
        assert_eq!(lines, [("Misc", 600, 1000), ("Misc", 400, 2000)]);
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

    const ONLINE_ID: &str = "O-7DBCDA8A56EE426DBCDA8A56EE426D1A";
    const OFFLINE_CODE: &str =
        "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180213093414:23:237.23";

    fn online_id_of(qr: &str) -> Option<String> {
        ReceiptCode::parse(qr)
            .filter(|code| code.offline().is_none())
            .map(|code| code.text().to_owned())
    }

    #[test]
    fn test_parse_bare_online_receipt_id() {
        assert_eq!(online_id_of(ONLINE_ID).as_deref(), Some(ONLINE_ID));
    }

    #[test]
    fn test_an_online_id_in_lower_case_is_the_same_receipt() {
        let lower = ONLINE_ID.to_ascii_lowercase();
        let url = format!("https://ekasa.financnasprava.sk/receipt?receiptId={lower}");

        for qr in [lower.as_str(), url.as_str()] {
            let code = ReceiptCode::parse(qr).unwrap();
            assert_eq!(code.text(), ONLINE_ID, "{qr}");
            assert_eq!(
                code.source_ref(),
                SourceRef::ekasa_online(ONLINE_ID),
                "{qr}"
            );
        }
    }

    #[test]
    fn test_parse_trims_surrounding_whitespace() {
        let qr = format!("  {ONLINE_ID}\r\n");
        assert_eq!(online_id_of(&qr).as_deref(), Some(ONLINE_ID));
    }

    #[test]
    fn test_parse_receipt_id_url() {
        let qr = format!("https://ekasa.financnasprava.sk/receipt?receiptId={ONLINE_ID}&lang=sk");
        assert_eq!(online_id_of(&qr).as_deref(), Some(ONLINE_ID));
    }

    #[test]
    fn test_a_receipt_id_url_must_carry_an_online_receipt_id() {
        for id in [
            "hello",
            "O-7DBCDA8A56EE426DBCDA8A56EE4-TEST",
            "O-7DBCDA8A56EE426DBCDA8A56EE426D1A<script>",
            OFFLINE_CODE,
        ] {
            let qr = format!("https://ekasa.financnasprava.sk/receipt?receiptId={id}&lang=sk");
            assert_eq!(ReceiptCode::parse(&qr), None, "{qr:?}");
        }
    }

    #[test]
    fn test_parse_offline_code() {
        let code = ReceiptCode::parse(&format!(" {OFFLINE_CODE}\n")).unwrap();
        let receipt = code.offline().expect("an offline code");

        assert_eq!(code.text(), OFFLINE_CODE);
        assert_eq!(receipt.event_date, "2018-02-13");
        assert_eq!(receipt.total_amount_minor, 23723);
    }

    #[test]
    fn test_an_offline_code_is_made_of_ekasa_fields_only() {
        for (okp, cash_register, sequence) in [
            ("Free text", "99920045678900001", "23"),
            (
                "C44B3977-0E415CC6-EE663AA1-776C973G",
                "99920045678900001",
                "23",
            ),
            ("", "99920045678900001", "23"),
            ("C44B3977-0E415CC6", "9992 0045", "23"),
            ("C44B3977-0E415CC6", "Kasa-1", "23"),
            ("C44B3977-0E415CC6", "", "23"),
            ("C44B3977-0E415CC6", "99920045678900001", "2a"),
            ("C44B3977-0E415CC6", "99920045678900001", "-1"),
            ("C44B3977-0E415CC6", "99920045678900001", ""),
        ] {
            let qr = format!("{okp}:{cash_register}:180213093414:{sequence}:237.23");
            assert_eq!(ReceiptCode::parse(&qr), None, "{qr:?}");
        }
        let lower_okp = "c44b3977-0e415cc6:Kasa01:180213093414:23:237.23";
        assert!(ReceiptCode::parse(lower_okp).is_some());
    }

    #[test]
    fn test_a_code_parses_back_from_its_text() {
        let url = format!("https://ekasa.financnasprava.sk/receipt?receiptId={ONLINE_ID}");
        for qr in [url.as_str(), OFFLINE_CODE] {
            let code = ReceiptCode::parse(qr).unwrap();
            assert_eq!(ReceiptCode::parse(code.text()), Some(code));
        }
    }

    #[test]
    fn test_online_reference_is_the_same_however_the_qr_carries_the_id() {
        let url = format!("https://ekasa.financnasprava.sk/receipt?receiptId={ONLINE_ID}&lang=sk");
        let padded = format!("  {ONLINE_ID}\r\n");
        let references: Vec<SourceRef> = [ONLINE_ID, url.as_str(), padded.as_str()]
            .iter()
            .map(|qr| ReceiptCode::parse(qr).unwrap().source_ref())
            .collect();

        assert_eq!(references[0].as_str(), format!("online:{ONLINE_ID}"));
        assert_eq!(references[1], references[0]);
        assert_eq!(references[2], references[0]);
    }

    #[test]
    fn test_offline_reference_is_the_trimmed_code() {
        assert_eq!(
            ReceiptCode::parse(&format!("  {OFFLINE_CODE}\n"))
                .unwrap()
                .source_ref()
                .as_str(),
            format!("offline:{OFFLINE_CODE}")
        );
        assert_eq!(
            SourceRef::ekasa_offline(&format!("  {OFFLINE_CODE}\n")).as_str(),
            format!("offline:{OFFLINE_CODE}")
        );
    }

    #[test]
    fn test_offline_reference_differs_for_another_receipt_number_or_total() {
        let other_number = OFFLINE_CODE.replace(":23:", ":24:");
        let other_total = OFFLINE_CODE.replace(":237.23", ":237.24");

        let reference = SourceRef::ekasa_offline(OFFLINE_CODE);

        assert_ne!(reference, SourceRef::ekasa_offline(&other_number));
        assert_ne!(reference, SourceRef::ekasa_offline(&other_total));
    }

    #[test]
    fn test_online_and_offline_references_never_collide() {
        assert_ne!(
            SourceRef::ekasa_online(ONLINE_ID),
            SourceRef::ekasa_offline(ONLINE_ID)
        );
    }

    #[test]
    fn test_other_text_is_not_an_ekasa_code() {
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
            assert_eq!(ReceiptCode::parse(qr), None, "{qr:?}");
        }
    }

    #[test]
    fn test_a_malformed_offline_code_is_not_an_ekasa_code() {
        for qr in [
            "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180230093414:23:237.23",
            "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180213093414:23:23723",
            "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180213093414:23",
        ] {
            assert_eq!(ReceiptCode::parse(qr), None, "{qr:?}");
        }
    }
}
