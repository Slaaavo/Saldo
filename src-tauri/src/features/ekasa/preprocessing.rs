use std::collections::HashSet;

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

/// Parses an offline eKasa QR code string.
///
/// Expected format: `{OKP}:{KP}:{YYMMDDHHmmss}:{sequence}:{total}`
///
/// Returns `None` if the format is unrecognised or any field fails to parse.
pub fn parse_offline_qr(qr_content: &str) -> Option<OfflineReceiptData> {
    let parts: Vec<&str> = qr_content.split(':').collect();
    if parts.len() < 5 {
        return None;
    }

    let dt_str = parts[2];
    if dt_str.len() != 12 {
        return None;
    }

    // Try YYMMDDHHmmss first; fall back to DDMMYYHHmmss if that produces an invalid date.
    let event_date = parse_dt_yymmdd(dt_str).or_else(|| parse_dt_ddmmyy(dt_str))?;
    let total_amount_minor = parse_total_to_minor(parts[4])?;

    Some(OfflineReceiptData {
        event_date,
        total_amount_minor,
    })
}

/// Interprets the first 6 chars of `dt_str` as `YY MM DD`.
/// Returns `"20YY-MM-DD"` or `None` if the date fields are out of range.
fn parse_dt_yymmdd(dt_str: &str) -> Option<String> {
    let yy = &dt_str[0..2];
    let mm = &dt_str[2..4];
    let dd = &dt_str[4..6];

    let mm_n: u32 = mm.parse().ok()?;
    let dd_n: u32 = dd.parse().ok()?;

    if !(1..=12).contains(&mm_n) || !(1..=31).contains(&dd_n) {
        return None;
    }

    Some(format!("20{}-{}-{}", yy, mm, dd))
}

/// Interprets the first 6 chars of `dt_str` as `DD MM YY`.
/// Returns `"20YY-MM-DD"` or `None` if the date fields are out of range.
fn parse_dt_ddmmyy(dt_str: &str) -> Option<String> {
    let dd = &dt_str[0..2];
    let mm = &dt_str[2..4];
    let yy = &dt_str[4..6];

    let mm_n: u32 = mm.parse().ok()?;
    let dd_n: u32 = dd.parse().ok()?;

    if !(1..=12).contains(&mm_n) || !(1..=31).contains(&dd_n) {
        return None;
    }

    Some(format!("20{}-{}-{}", yy, mm, dd))
}

/// Converts a total amount string to minor units (cents).
/// Strings containing `'.'` are treated as decimal EUR (e.g. `"12.50"` → 1250).
/// Strings without `'.'` are treated as already-integer minor units (e.g. `"1250"` → 1250).
fn parse_total_to_minor(total_str: &str) -> Option<i64> {
    if total_str.contains('.') {
        let val: f64 = total_str.parse().ok()?;
        Some((val * 100.0_f64).round() as i64)
    } else {
        total_str.parse().ok()
    }
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
    fn test_parse_offline_qr_integer_total() {
        let qr = "ABCD1234:XYZ9876:260412153045:001:1250";
        let result = parse_offline_qr(qr);
        assert!(result.is_some());
        assert_eq!(result.unwrap().total_amount_minor, 1250);
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
    fn test_parse_offline_qr_fallback_to_ddmmyy_on_invalid_yymmdd_day() {
        // "260432121200": YYMMDD → DD=32 (invalid) → fallback to DDMMYY
        // DDMMYY: DD=26, MM=04, YY=32 → "2032-04-26"
        let qr = "ABCD1234:XYZ9876:260432121200:001:25.00";
        let result = parse_offline_qr(qr);
        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.event_date, "2032-04-26");
        assert_eq!(data.total_amount_minor, 2500);
    }

    #[test]
    fn test_parse_offline_qr_both_formats_invalid_returns_none() {
        // Month field (chars 2..4) = "99" → invalid for both YYMMDD and DDMMYY
        let qr = "ABCD1234:XYZ9876:269912121200:001:10.00";
        assert!(parse_offline_qr(qr).is_none());
    }
}
