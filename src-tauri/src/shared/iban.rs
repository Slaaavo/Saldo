use crate::error::AppError;

/// Validate and normalise an IBAN string.
/// Returns the uppercase, space-stripped IBAN on success.
pub fn validate_iban(raw: &str) -> Result<String, AppError> {
    let normalised: String = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if normalised.len() < 15 || normalised.len() > 34 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "IBAN must be 15–34 alphanumeric characters.".into(),
        });
    }
    if !normalised.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "IBAN must be 15–34 alphanumeric characters.".into(),
        });
    }
    Ok(normalised)
}

pub fn is_duplicate_iban_error(err: &rusqlite::Error) -> bool {
    if let rusqlite::Error::SqliteFailure(ref sqlite_err, ref msg) = err {
        // SQLITE_CONSTRAINT_UNIQUE = 2067
        if sqlite_err.extended_code == 2067 {
            if let Some(m) = msg {
                return m.contains("idx_account_iban");
            }
        }
    }
    false
}
