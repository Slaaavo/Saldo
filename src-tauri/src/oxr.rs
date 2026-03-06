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

pub async fn fetch_rates(api_key: &str, date: Option<&str>) -> Result<OxrResponse, AppError> {
    let url = match date {
        None => format!("{}/latest.json?app_id={}", OXR_BASE_URL, api_key),
        Some(d) => format!("{}/historical/{}.json?app_id={}", OXR_BASE_URL, d, api_key),
    };

    let client = reqwest::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .https_only(true)
        .build()
        .map_err(|e| AppError::from(e.to_string()))?;

    let response = client.get(&url).send().await.map_err(|e| AppError {
        code: "NETWORK_ERROR".into(),
        message: e.to_string(),
    })?;

    if !response.status().is_success() {
        return Err(AppError {
            code: "OXR_ERROR".into(),
            message: format!("OXR API returned status {}", response.status()),
        });
    }

    let oxr_response = response.json::<OxrResponse>().await.map_err(|e| AppError {
        code: "PARSE_ERROR".into(),
        message: format!("Failed to parse OXR response: {}", e),
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

fn parse_decimal_str(s: &str) -> Result<(i64, i64), AppError> {
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

/// Compute cross rate: how many `target` units equal 1 `consolidation` unit.
///
/// Formula (corrected): `cross_rate = oxr_target / oxr_consolidation`
///
/// If consolidation is the OXR base currency (not present in rates map), its rate is 1.
/// Uses i128 arithmetic with 10 digits of extra precision to avoid float division.
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

    // cross_rate = (m_t * 10^e_t) / (m_c * 10^e_c)
    //            = (m_t / m_c) * 10^(e_t - e_c)
    //
    // Multiply numerator by 10^PRECISION to keep 10 significant digits in the quotient.
    const PRECISION: i64 = 10;
    let precision_factor = 10_i128.pow(PRECISION as u32);

    let numerator = (m_t as i128) * precision_factor;
    let raw_mantissa = numerator / (m_c as i128);
    let raw_exponent = e_t - e_c - PRECISION;

    if raw_mantissa == 0 {
        return Ok((0, 0));
    }

    // Normalize: strip trailing zeros.
    let mut mantissa = raw_mantissa;
    let mut exponent = raw_exponent;
    while mantissa % 10 == 0 {
        mantissa /= 10;
        exponent += 1;
    }

    if mantissa > i64::MAX as i128 || mantissa < i64::MIN as i128 {
        return Err(AppError {
            code: "OVERFLOW".into(),
            message: "Computed cross rate mantissa overflows i64".into(),
        });
    }

    Ok((mantissa as i64, exponent))
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
    fn cross_rate_missing_target_returns_error() {
        let rates: HashMap<String, serde_json::Value> = HashMap::new();
        let result = compute_cross_rate(&rates, "EUR", "GBP");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "MISSING_RATE");
    }
}
