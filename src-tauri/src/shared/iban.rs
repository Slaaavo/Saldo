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
                // SQLite names the column, not the index, for a unique index on a plain column.
                return m.contains("account.iban");
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::partner_accounts::repository::{
        create_partner_account, CreatePartnerAccountParams,
    };

    const IBAN: &str = "SK3112000000198742637541";

    fn new_account(iban: &str) -> CreateAccountParams {
        CreateAccountParams {
            name: "Account".to_owned(),
            currency_id: 1,
            account_type: "account".to_owned(),
            iban: Some(iban.to_owned()),
            ..Default::default()
        }
    }

    fn new_partner(iban: &str) -> CreatePartnerAccountParams {
        CreatePartnerAccountParams {
            name: "Partner".to_owned(),
            iban: iban.to_owned(),
            currency_id: 1,
        }
    }

    #[test]
    fn second_account_with_same_iban_is_duplicate() {
        let conn = initialize_in_memory().expect("DB init failed");
        create_account(&conn, new_account(IBAN)).unwrap();

        let err = create_account(&conn, new_account(IBAN)).unwrap_err();

        assert_eq!(err.code, "DUPLICATE_IBAN");
    }

    #[test]
    fn partner_with_iban_of_existing_account_is_duplicate() {
        let conn = initialize_in_memory().expect("DB init failed");
        create_account(&conn, new_account(IBAN)).unwrap();

        let err = create_partner_account(&conn, new_partner(IBAN)).unwrap_err();

        assert_eq!(err.code, "DUPLICATE_IBAN");
    }

    #[test]
    fn second_partner_with_same_iban_is_duplicate() {
        let conn = initialize_in_memory().expect("DB init failed");
        create_partner_account(&conn, new_partner(IBAN)).unwrap();

        let err = create_partner_account(&conn, new_partner(IBAN)).unwrap_err();

        assert_eq!(err.code, "DUPLICATE_IBAN");
    }

    #[test]
    fn unrelated_unique_violation_is_not_a_duplicate_iban() {
        let conn = initialize_in_memory().expect("DB init failed");
        conn.execute_batch(
            "CREATE TABLE unrelated (code TEXT UNIQUE); INSERT INTO unrelated VALUES ('x');",
        )
        .unwrap();

        let err = conn
            .execute("INSERT INTO unrelated VALUES ('x')", [])
            .unwrap_err();

        assert!(!super::is_duplicate_iban_error(&err));
    }
}
