use crate::error::AppError;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct OxrResponse {
    #[allow(dead_code)] // Part of OXR API contract; kept for documentation.
    pub base: String,
    pub rates: HashMap<String, serde_json::Value>,
}

const OXR_BASE_URL: &str = "https://openexchangerates.org/api";

/// The API key travels in the URL query and reqwest prints the URL in its error text, so the
/// message that reaches the FX Rates page, toasts and the log must not carry it.
fn network_error(e: reqwest::Error) -> AppError {
    AppError {
        code: "NETWORK_ERROR".into(),
        message: e.without_url().to_string(),
    }
}

pub async fn fetch_rates(api_key: &str, date: Option<&str>) -> Result<OxrResponse, AppError> {
    let url = match date {
        None => format!("{}/latest.json?app_id={}", OXR_BASE_URL, api_key),
        Some(d) => format!("{}/historical/{}.json?app_id={}", OXR_BASE_URL, d, api_key),
    };

    let client = reqwest::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .https_only(true)
        .build()
        .map_err(|e| AppError::from(e.without_url().to_string()))?;

    let response = client.get(&url).send().await.map_err(network_error)?;

    let status = response.status();
    if !status.is_success() {
        let code = match status.as_u16() {
            401 | 403 | 429 => "OXR_ACCESS_ERROR",
            _ => "OXR_ERROR",
        };
        return Err(AppError {
            code: code.into(),
            message: format!("OXR API returned status {}", status),
        });
    }

    let oxr_response = response.json::<OxrResponse>().await.map_err(|e| AppError {
        code: "PARSE_ERROR".into(),
        message: format!("Failed to parse OXR response: {}", e.without_url()),
    })?;

    Ok(oxr_response)
}

/// Parse a JSON number Value into (mantissa, exponent) without f64 intermediate for integers.
/// For decimal/float values, uses Rust's Ryu shortest-decimal algorithm which guarantees
/// a round-trip accurate string (OXR rates have at most ~10 significant digits).
pub fn parse_rate_to_mantissa_exponent(value: &serde_json::Value) -> Result<(i64, i64), AppError> {
    let n = match value {
        serde_json::Value::Number(n) => n,
        _ => {
            return Err(AppError {
                code: "PARSE_ERROR".into(),
                message: "Expected a JSON number for FX rate".into(),
            })
        }
    };

    // Prefer exact integer paths; fall back to float-string for decimals.
    let s = if let Some(i) = n.as_i64() {
        i.to_string()
    } else if let Some(u) = n.as_u64() {
        u.to_string()
    } else if let Some(f) = n.as_f64() {
        // format! uses Ryu algorithm: shortest decimal that round-trips through f64.
        format!("{}", f)
    } else {
        return Err(AppError {
            code: "PARSE_ERROR".into(),
            message: "Cannot extract number from JSON value".into(),
        });
    };

    parse_decimal_str(&s)
}

pub(crate) fn parse_decimal_str(s: &str) -> Result<(i64, i64), AppError> {
    let s = s.trim();

    // Handle optional leading minus sign.
    let (negative, s) = if let Some(rest) = s.strip_prefix('-') {
        (true, rest)
    } else {
        (false, s)
    };

    // Split on 'e'/'E' for scientific notation.
    let (coeff_str, sci_exp) = if let Some(pos) = s.find(['e', 'E']) {
        let exp: i64 = s[pos + 1..].parse().map_err(|_| AppError {
            code: "PARSE_ERROR".into(),
            message: format!("Invalid scientific exponent in '{}'", s),
        })?;
        (&s[..pos], exp)
    } else {
        (s, 0i64)
    };

    // Split on decimal point.
    let (int_part, frac_part, frac_len) = if let Some(dot_pos) = coeff_str.find('.') {
        let frac = &coeff_str[dot_pos + 1..];
        (&coeff_str[..dot_pos], frac, frac.len() as i64)
    } else {
        (coeff_str, "", 0i64)
    };

    // Concatenate all digit characters.
    let combined = format!("{}{}", int_part, frac_part);

    // Strip leading zeros.
    let trimmed = combined.trim_start_matches('0');
    if trimmed.is_empty() {
        return Ok((0, 0));
    }

    // Strip trailing zeros and count them.
    let mantissa_str = trimmed.trim_end_matches('0');
    let trailing_zeros = (trimmed.len() - mantissa_str.len()) as i64;

    let abs_mantissa: i64 = mantissa_str.parse().map_err(|_| AppError {
        code: "PARSE_ERROR".into(),
        message: format!("Mantissa overflows i64 in '{}'", s),
    })?;

    let mantissa = if negative {
        -abs_mantissa
    } else {
        abs_mantissa
    };
    // exponent accounts for: decimal position, stripped trailing zeros, and sci notation.
    let exponent = trailing_zeros - frac_len + sci_exp;

    Ok((mantissa, exponent))
}

/// Significant digits kept by `compute_cross_rate` before the final quotient is rounded. The
/// stored mantissa ends up with this many digits or one more, so it stays below 10^15 and is
/// exactly representable as a JS number on the frontend.
const CROSS_RATE_DIGITS: i64 = 14;

/// Compute cross rate: how many `target` units equal 1 `consolidation` unit.
///
/// Formula (corrected): `cross_rate = oxr_target / oxr_consolidation`
///
/// If consolidation is the OXR base currency (not present in rates map), its rate is 1.
/// Uses integer arithmetic scaled so the quotient keeps `CROSS_RATE_DIGITS` significant digits
/// whatever the digit counts of the two mantissas, rounded half up. The returned mantissa is
/// never 0 (`convert_balance` divides by it); a zero rate is an error.
pub fn compute_cross_rate(
    oxr_rates: &HashMap<String, serde_json::Value>,
    consolidation_code: &str,
    target_code: &str,
) -> Result<(i64, i64), AppError> {
    let target_val = oxr_rates.get(target_code).ok_or_else(|| AppError {
        code: "MISSING_RATE".into(),
        message: format!("No OXR rate found for currency '{}'", target_code),
    })?;

    let (m_t, e_t) = parse_rate_to_mantissa_exponent(target_val)?;

    // If consolidation not in rates, it's the OXR base (USD), rate = 1 exactly.
    let (m_c, e_c) = if let Some(consol_val) = oxr_rates.get(consolidation_code) {
        parse_rate_to_mantissa_exponent(consol_val)?
    } else {
        (1i64, 0i64)
    };

    if m_c == 0 {
        return Err(AppError {
            code: "COMPUTE_ERROR".into(),
            message: "Consolidation currency rate is zero".into(),
        });
    }
    if m_t == 0 {
        return Err(AppError {
            code: "COMPUTE_ERROR".into(),
            message: format!("OXR rate for currency '{}' is zero", target_code),
        });
    }

    // cross_rate = (m_t * 10^e_t) / (m_c * 10^e_c)
    //            = (m_t / m_c) * 10^(e_t - e_c)
    //
    // Scale the numerator (or, when it is already much longer, the denominator) by 10^shift so
    // the quotient has CROSS_RATE_DIGITS or one more digits, whatever the two mantissas' lengths.
    let negative = (m_t < 0) != (m_c < 0);
    let numerator = m_t.unsigned_abs() as u128;
    let denominator = m_c.unsigned_abs() as u128;
    let shift = CROSS_RATE_DIGITS - (numerator.ilog10() as i64 - denominator.ilog10() as i64);
    let (numerator, denominator) = if shift >= 0 {
        (numerator * 10_u128.pow(shift as u32), denominator)
    } else {
        (numerator, denominator * 10_u128.pow(-shift as u32))
    };

    // `div_round` in shared/helpers.rs is i64-only; the scaled numerator needs u128.
    let mut mantissa = (numerator + denominator / 2) / denominator;
    let mut exponent = e_t - e_c - shift;

    // Normalize: strip trailing zeros.
    while mantissa % 10 == 0 {
        mantissa /= 10;
        exponent += 1;
    }

    let mantissa = i64::try_from(mantissa).map_err(|_| AppError {
        code: "OVERFLOW".into(),
        message: "Computed cross rate mantissa overflows i64".into(),
    })?;

    Ok((if negative { -mantissa } else { mantissa }, exponent))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- parse_rate_to_mantissa_exponent / parse_decimal_str tests ---

    #[test]
    fn parse_integer_one() {
        assert_eq!(parse_decimal_str("1").unwrap(), (1, 0));
    }

    #[test]
    fn parse_one_point_zero() {
        // Trailing zero should be stripped; exponent compensates.
        assert_eq!(parse_decimal_str("1.0").unwrap(), (1, 0));
    }

    #[test]
    fn parse_1_0842() {
        assert_eq!(parse_decimal_str("1.0842").unwrap(), (10842, -4));
    }

    #[test]
    fn parse_157_23() {
        assert_eq!(parse_decimal_str("157.23").unwrap(), (15723, -2));
    }

    #[test]
    fn parse_small_decimal() {
        assert_eq!(parse_decimal_str("0.0000149").unwrap(), (149, -7));
    }

    #[test]
    fn parse_scientific_notation() {
        // 1.5e-3 = 0.0015 = 15 * 10^(-4)
        assert_eq!(parse_decimal_str("1.5e-3").unwrap(), (15, -4));
    }

    #[test]
    fn parse_trailing_zeros() {
        // "0.9000" -> mantissa=9, exponent=-1 (trailing zeros stripped, exponent adjusted)
        assert_eq!(parse_decimal_str("0.9000").unwrap(), (9, -1));
    }

    #[test]
    fn parse_json_number_integer() {
        let v = json!(1);
        assert_eq!(parse_rate_to_mantissa_exponent(&v).unwrap(), (1, 0));
    }

    #[test]
    fn parse_json_number_decimal() {
        let v = json!(0.9223);
        let (m, e) = parse_rate_to_mantissa_exponent(&v).unwrap();
        // Reconstruct and compare within float precision.
        let result = (m as f64) * 10_f64.powi(e as i32);
        assert!(
            (result - 0.9223).abs() < 1e-10,
            "got {} (m={}, e={})",
            result,
            m,
            e
        );
    }

    // --- compute_cross_rate tests ---

    #[test]
    fn cross_rate_eur_consolidation_btc_target() {
        let mut rates = HashMap::new();
        rates.insert("EUR".to_string(), json!(0.9223));
        rates.insert("BTC".to_string(), json!(0.0000149));

        let (m, e) = compute_cross_rate(&rates, "EUR", "BTC").unwrap();
        // Expected: 0.0000149 / 0.9223 ≈ 1.615e-5
        let result = (m as f64) * 10_f64.powi(e as i32);
        let expected = 0.0000149_f64 / 0.9223_f64;
        let rel_err = ((result - expected) / expected).abs();
        assert!(rel_err < 1e-9, "relative error too large: {:.2e}", rel_err);
    }

    #[test]
    fn cross_rate_usd_base_not_in_rates() {
        // Consolidation = USD (OXR base): treat as rate=1, so cross_rate = oxr_target.
        let mut rates = HashMap::new();
        rates.insert("EUR".to_string(), json!(0.9223));

        let (m, e) = compute_cross_rate(&rates, "USD", "EUR").unwrap();
        let result = (m as f64) * 10_f64.powi(e as i32);
        let rel_err = ((result - 0.9223) / 0.9223).abs();
        assert!(rel_err < 1e-9, "relative error too large: {:.2e}", rel_err);
    }

    #[test]
    fn cross_rate_same_currency_returns_one() {
        // EUR→EUR cross rate = 1/1 = 1 (consolidation=EUR in rates with value 0.9223/0.9223=1)
        let mut rates = HashMap::new();
        rates.insert("EUR".to_string(), json!(0.9223));

        let (m, e) = compute_cross_rate(&rates, "EUR", "EUR").unwrap();
        let result = (m as f64) * 10_f64.powi(e as i32);
        assert!((result - 1.0).abs() < 1e-9, "expected 1.0, got {}", result);
    }

    #[test]
    fn network_error_message_omits_the_api_key() {
        // https_only rejects the plain-http URL before any connection is attempted.
        let client = reqwest::ClientBuilder::new()
            .https_only(true)
            .build()
            .unwrap();
        let err = tauri::async_runtime::block_on(
            client
                .get("http://localhost/latest.json?app_id=SECRET_KEY")
                .send(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("SECRET_KEY"));

        let mapped = network_error(err);
        assert_eq!(mapped.code, "NETWORK_ERROR");
        assert!(!mapped.message.contains("SECRET_KEY"), "{}", mapped.message);
    }

    #[test]
    fn cross_rate_missing_target_returns_error() {
        let rates: HashMap<String, serde_json::Value> = HashMap::new();
        let result = compute_cross_rate(&rates, "EUR", "GBP");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "MISSING_RATE");
    }

    #[test]
    fn cross_rate_keeps_precision_when_numerator_is_short() {
        // 1 / 0.921453 = 1.08524254628288...; a fixed numerator scale kept only 1.0852.
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), json!(1));
        rates.insert("EUR".to_string(), json!(0.921453));

        let (m, e) = compute_cross_rate(&rates, "EUR", "USD").unwrap();
        assert_eq!((m, e), (10852425462829, -13));
    }

    #[test]
    fn cross_rate_keeps_precision_for_small_rate() {
        // 0.00000123 / 0.921453 = 1.3348483319279...e-6
        let mut rates = HashMap::new();
        rates.insert("EUR".to_string(), json!(0.921453));
        rates.insert("XYZ".to_string(), json!(0.00000123));

        let (m, e) = compute_cross_rate(&rates, "EUR", "XYZ").unwrap();
        assert_eq!((m, e), (13348483319279, -19));
    }

    #[test]
    fn cross_rate_converts_without_overflow_at_small_scale() {
        let mut rates = HashMap::new();
        rates.insert("EUR".to_string(), json!(0.921453));
        rates.insert("XYZ".to_string(), json!(0.00000123));

        let (m, e) = compute_cross_rate(&rates, "EUR", "XYZ").unwrap();
        // 10^9 XYZ cents at 1 EUR = 1.3348483319279e-6 XYZ.
        assert_eq!(
            crate::shared::convert_balance(1_000_000_000, m, e, 2, 2, false),
            749_148_780_487_830
        );
    }

    #[test]
    fn cross_rate_keeps_precision_for_large_rate() {
        // 123456789.123456 / 0.0000149 = 8285690545198.4...
        let mut rates = HashMap::new();
        rates.insert("BTC".to_string(), json!(0.0000149));
        rates.insert("XYZ".to_string(), json!(123456789.123456));

        let (m, e) = compute_cross_rate(&rates, "BTC", "XYZ").unwrap();
        assert_eq!((m, e), (82856905451984, -1));
    }

    #[test]
    fn cross_rate_rounds_half_up() {
        // 1 / 3 rounds down and 2 / 3 rounds up at the last kept digit.
        let mut rates = HashMap::new();
        rates.insert("AAA".to_string(), json!(3));
        rates.insert("BBB".to_string(), json!(1));
        rates.insert("CCC".to_string(), json!(2));

        assert_eq!(
            compute_cross_rate(&rates, "AAA", "BBB").unwrap(),
            (33333333333333, -14)
        );
        assert_eq!(
            compute_cross_rate(&rates, "AAA", "CCC").unwrap(),
            (66666666666667, -14)
        );
    }

    #[test]
    fn cross_rate_mantissa_stays_exact_for_js_numbers() {
        let mut rates = HashMap::new();
        rates.insert("EUR".to_string(), json!(0.9223));
        rates.insert("BTC".to_string(), json!(0.0000149));
        rates.insert("JPY".to_string(), json!(157.23));

        for target in ["BTC", "JPY", "EUR"] {
            let (m, _) = compute_cross_rate(&rates, "BTC", target).unwrap();
            assert!(m.unsigned_abs() < 1 << 53, "mantissa {} not JS-safe", m);
        }
    }

    #[test]
    fn cross_rate_zero_target_returns_error() {
        let mut rates = HashMap::new();
        rates.insert("EUR".to_string(), json!(0.9223));
        rates.insert("XYZ".to_string(), json!(0));

        let err = compute_cross_rate(&rates, "EUR", "XYZ").unwrap_err();
        assert_eq!(err.code, "COMPUTE_ERROR");
    }

    #[test]
    fn cross_rate_zero_consolidation_returns_error() {
        let mut rates = HashMap::new();
        rates.insert("EUR".to_string(), json!(0));
        rates.insert("USD".to_string(), json!(1));

        let err = compute_cross_rate(&rates, "EUR", "USD").unwrap_err();
        assert_eq!(err.code, "COMPUTE_ERROR");
    }
}
