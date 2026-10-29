use serde::{Deserialize, Serialize};

use crate::error::AppError;

const ONLINE: &str = "online:";
const OFFLINE: &str = "offline:";
const INVOICE: &str = "invoice:";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceRef(String);

impl SourceRef {
    pub fn ekasa_online(receipt_id: &str) -> Self {
        Self(format!("{ONLINE}{receipt_id}"))
    }

    pub fn ekasa_offline(offline_code: &str) -> Self {
        Self(format!("{OFFLINE}{}", offline_code.trim()))
    }

    pub fn invoice(issuer: &str, number: &str) -> Result<Self, AppError> {
        let number = canonical(number);
        if number.is_empty() {
            return Err(invalid("The document number is required"));
        }
        Ok(Self(format!("{}{number}", invoice_issuer_prefix(issuer)?)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn invoice_issuer_prefix(issuer: &str) -> Result<String, AppError> {
    let issuer = canonical(issuer);
    if issuer.is_empty() {
        return Err(invalid("The issuer is required"));
    }
    if issuer.contains('/') {
        return Err(invalid("The issuer cannot contain '/'"));
    }
    Ok(format!("{INVOICE}{issuer}/"))
}

fn canonical(part: &str) -> String {
    part.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase()
}

fn invalid(message: &str) -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message: message.into(),
    }
}

impl TryFrom<String> for SourceRef {
    type Error = String;

    fn try_from(value: String) -> Result<Self, String> {
        let body = [ONLINE, OFFLINE, INVOICE]
            .iter()
            .find_map(|prefix| value.strip_prefix(prefix));
        match body {
            Some(body) if !body.trim().is_empty() => Ok(Self(value)),
            _ => Err(format!(
                "'{value}' is not a source reference (online:, offline: or invoice:)"
            )),
        }
    }
}

impl From<SourceRef> for String {
    fn from(source_ref: SourceRef) -> String {
        source_ref.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_invoice_reference_names_the_issuer_and_the_number() {
        let reference = SourceRef::invoice("31347037", "2026/10/551").unwrap();
        assert_eq!(reference.as_str(), "invoice:31347037/2026/10/551");
    }

    #[test]
    fn an_invoice_reference_ignores_case_and_whitespace() {
        assert_eq!(
            SourceRef::invoice(" sk 2020 ", "fa 2026\t001").unwrap(),
            SourceRef::invoice("SK2020", "FA2026001").unwrap()
        );
    }

    #[test]
    fn the_same_number_from_two_issuers_is_two_documents() {
        assert_ne!(
            SourceRef::invoice("31347037", "FA1").unwrap(),
            SourceRef::invoice("36562939", "FA1").unwrap()
        );
    }

    #[test]
    fn an_invoice_reference_needs_an_issuer_and_a_number() {
        for (issuer, number) in [
            ("", "FA1"),
            ("  ", "FA1"),
            ("31347037", " "),
            ("a/b", "FA1"),
        ] {
            let err = SourceRef::invoice(issuer, number).unwrap_err();
            assert_eq!(err.code, "VALIDATION", "{issuer:?} {number:?}");
        }
    }

    #[test]
    fn every_invoice_from_an_issuer_starts_with_its_prefix() {
        let prefix = invoice_issuer_prefix("sk 2020").unwrap();
        let reference = SourceRef::invoice("SK2020", "FA1").unwrap();
        assert!(reference.as_str().starts_with(&prefix));
        assert!(!SourceRef::invoice("SK20201", "FA1")
            .unwrap()
            .as_str()
            .starts_with(&prefix));
    }

    #[test]
    fn an_invoice_never_collides_with_an_ekasa_receipt() {
        assert_ne!(
            SourceRef::invoice("online", "X").unwrap().as_str(),
            SourceRef::ekasa_online("X").as_str()
        );
    }

    #[test]
    fn a_reference_from_the_frontend_must_carry_a_known_prefix() {
        let parse = |s: &str| serde_json::from_value::<SourceRef>(serde_json::json!(s));
        assert_eq!(
            parse("online:O-ABC").unwrap(),
            SourceRef::ekasa_online("O-ABC")
        );
        for bad in ["O-ABC", "online:", "invoice: ", "ekasa:O-ABC"] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }
}
