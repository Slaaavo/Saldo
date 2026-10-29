use serde::{Deserialize, Serialize};

const ONLINE: &str = "online:";
const OFFLINE: &str = "offline:";
const INVOICE: &str = "invoice:";

/// The document an entry or an asset was recorded from, in the one canonical form that is
/// stored on `event.source_ref` and `account.source_ref`:
///
/// - `online:<receipt id>` and `offline:<offline code>` for eKasa receipts;
/// - `invoice:<ISSUER>/<NUMBER>` for an invoice or credit note.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceRef(String);

impl SourceRef {
    /// The id alone identifies an online receipt, however its QR code carried it.
    pub fn ekasa_online(receipt_id: &str) -> Self {
        Self(format!("{ONLINE}{receipt_id}"))
    }

    /// The whole offline code: it holds the OKP, cash register code, date and time, receipt
    /// number and total.
    pub fn ekasa_offline(offline_code: &str) -> Self {
        Self(format!("{OFFLINE}{}", offline_code.trim()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
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
