use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::models::{EkasaReceiptData, OfflineReceiptData, ReceiptReadLine};
use super::preprocessing::{preprocess_receipt_items, ReceiptCode};
use super::qr_decoder::decode_qr_from_file;
use crate::error::AppError;
use crate::features::transactions::SourceRef;

/// The receipt as read_receipt read it. Put it into record_batch unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceiptId {
    /// The receipt's file. Shown on the result row, not stored.
    pub file: String,
    /// The receipt's eKasa code: an online receipt id, or an offline receipt's code.
    pub code: String,
    /// read_receipt's seal over what it read.
    pub read_token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptKind {
    Online,
    Offline,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "code",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ReadingWarning {
    /// The eKasa service's own lines do not add up to its total. The receipt is recorded as
    /// listed.
    LinesDifferFromTotal {
        lines_total_minor: i64,
        total_minor: i64,
    },
}

/// What Saldo read from a receipt. The app's wizard and `saldo mcp`'s read_receipt both start
/// from it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptReading {
    pub receipt: ReceiptId,
    pub kind: ReceiptKind,
    pub source_ref: SourceRef,
    /// YYYY-MM-DD.
    pub date: String,
    /// The shop, from the eKasa service. None for an offline receipt.
    pub vendor: Option<String>,
    pub vendor_ico: Option<String>,
    pub total_minor: i64,
    /// None for an offline receipt, whose code holds only its date and total.
    pub lines: Option<Vec<ReceiptReadLine>>,
    pub warnings: Vec<ReadingWarning>,
    /// The eKasa service's answer as it came, which the wizard still receives.
    #[serde(skip)]
    pub api_receipt: Option<EkasaReceiptData>,
}

/// The eKasa service of Finančná správa, behind a trait so tests never reach the network.
pub trait EkasaApi {
    /// The receipt, or None when the service does not know the id.
    fn fetch(&self, receipt_id: &str) -> Result<Option<EkasaReceiptData>, AppError>;
}

/// File to receipt, for the app and for `saldo mcp`. Synchronous: QR decoding is CPU heavy and
/// an online receipt waits for the eKasa service, so an async caller runs it on a blocking
/// thread. It touches no database.
///
/// `code` is the online receipt id typed from the printed receipt when the QR code does not
/// decode; the file is then not decoded. An offline code must come from the QR code itself.
pub fn read_receipt_file(
    file: &str,
    code: Option<&str>,
    resource_dir: Option<&Path>,
    api: &dyn EkasaApi,
) -> Result<ReceiptReading, AppError> {
    let code = match code {
        Some(typed) => match ReceiptCode::parse(typed) {
            Some(online @ ReceiptCode::Online { .. }) => online,
            _ => return Err(not_an_online_id()),
        },
        None => {
            let text = decode_qr_from_file(file, resource_dir)?;
            ReceiptCode::parse(&text).ok_or_else(not_ekasa)?
        }
    };
    match &code {
        ReceiptCode::Online { receipt_id } => {
            let data = api
                .fetch(receipt_id)?
                .ok_or_else(|| receipt_not_found(receipt_id))?;
            online_reading(file, &code, data)
        }
        ReceiptCode::Offline { receipt, .. } => Ok(offline_reading(file, &code, receipt)),
    }
}

fn online_reading(
    file: &str,
    code: &ReceiptCode,
    data: EkasaReceiptData,
) -> Result<ReceiptReading, AppError> {
    let date = api_date(&data.issue_date)?;
    let lines = preprocess_receipt_items(&data.items);
    let total_minor = (data.total_price * 100.0).round() as i64;
    let lines_total_minor: i64 = lines.iter().map(|line| line.gross_minor).sum();
    let warnings = if lines_total_minor == total_minor {
        Vec::new()
    } else {
        vec![ReadingWarning::LinesDifferFromTotal {
            lines_total_minor,
            total_minor,
        }]
    };
    let read_token = seal(code, &date, lines.iter().map(LineFacts::from));
    Ok(ReceiptReading {
        receipt: ReceiptId {
            file: file.to_owned(),
            code: code.text().to_owned(),
            read_token,
        },
        kind: ReceiptKind::Online,
        source_ref: code.source_ref(),
        date,
        vendor: Some(data.organization.name.clone()),
        vendor_ico: Some(data.organization.ico.clone()),
        total_minor,
        lines: Some(lines),
        warnings,
        api_receipt: Some(data),
    })
}

fn offline_reading(file: &str, code: &ReceiptCode, receipt: &OfflineReceiptData) -> ReceiptReading {
    ReceiptReading {
        receipt: ReceiptId {
            file: file.to_owned(),
            code: code.text().to_owned(),
            read_token: seal(code, &receipt.event_date, std::iter::empty()),
        },
        kind: ReceiptKind::Offline,
        source_ref: code.source_ref(),
        date: receipt.event_date.clone(),
        vendor: None,
        vendor_ico: None,
        total_minor: receipt.total_amount_minor,
        lines: None,
        warnings: Vec::new(),
        api_receipt: None,
    }
}

/// One receipt line's facts, from a reading or from a batch line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineFacts<'a> {
    pub name: &'a str,
    pub gross_minor: i64,
    pub vat_rate_bps: i64,
}

impl<'a> From<&'a ReceiptReadLine> for LineFacts<'a> {
    fn from(line: &'a ReceiptReadLine) -> Self {
        Self {
            name: &line.name,
            gross_minor: line.gross_minor,
            vat_rate_bps: line.vat_rate_bps,
        }
    }
}

/// 16 hex digits over the code and, for an online receipt, its date and its lines in any
/// order. An offline code already holds its date and total. The vendor is not sealed: it is
/// only a note. `DefaultHasher::new` gives the same digits in every run of one build, as the
/// review token relies on too.
fn seal<'a>(code: &ReceiptCode, date: &str, lines: impl Iterator<Item = LineFacts<'a>>) -> String {
    let mut hasher = DefaultHasher::new();
    ("ekasa-receipt-v1", code.text()).hash(&mut hasher);
    if let ReceiptCode::Online { .. } = code {
        let mut lines: Vec<LineFacts> = lines.collect();
        lines.sort_unstable();
        (date, lines).hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

/// The service's `DD.MM.YYYY HH:mm:ss` as `YYYY-MM-DD`.
fn api_date(issue_date: &str) -> Result<String, AppError> {
    let day = issue_date.split(' ').next().unwrap_or_default();
    NaiveDate::parse_from_str(day, "%d.%m.%Y")
        .map(|date| date.format("%Y-%m-%d").to_string())
        .map_err(|_| AppError {
            code: "EKASA_API_ERROR".into(),
            message: format!(
                "The eKasa service sent the receipt date '{issue_date}', which is not \
                 DD.MM.YYYY HH:mm:ss"
            ),
        })
}

fn not_ekasa() -> AppError {
    AppError {
        code: "QR_NOT_EKASA".into(),
        message: "The QR code in this file is not an eKasa receipt code. Choose a file that \
                  shows the QR code printed on the receipt."
            .into(),
    }
}

fn not_an_online_id() -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message: "A typed code must be the online receipt id printed on the receipt: O- or V- \
                  and 32 hex digits. An offline receipt's code can only come from its QR code."
            .into(),
    }
}

fn receipt_not_found(receipt_id: &str) -> AppError {
    AppError {
        code: "RECEIPT_NOT_FOUND".into(),
        message: format!(
            "Finančná správa's eKasa service does not know the receipt {receipt_id}: the id is \
             mistyped, or the receipt is too new to be there yet."
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::features::ekasa::models::{EkasaReceiptItem, EkasaReceiptOrganization};

    const ONLINE_ID: &str = "O-7DBCDA8A56EE426DBCDA8A56EE426D1A";
    const OFFLINE_FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ekasa/offline-receipt.png"
    );
    const OFFLINE_CODE: &str =
        "C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180213093414:23:237.23";

    fn item(name: &str, price: f64, vat_rate: f64) -> EkasaReceiptItem {
        EkasaReceiptItem {
            name: name.to_owned(),
            quantity: 1.0,
            price,
            vat_rate,
        }
    }

    fn billa(total_price: f64) -> EkasaReceiptData {
        EkasaReceiptData {
            receipt_id: ONLINE_ID.to_owned(),
            issue_date: "12.10.2026 14:03:11".to_owned(),
            total_price,
            organization: EkasaReceiptOrganization {
                name: "BILLA s.r.o.".to_owned(),
                ico: "31347037".to_owned(),
            },
            items: vec![
                item("KAVA LAVAZZA", 15.99, 23.0),
                item("MLIEKO 1L", 1.29, 5.0),
                item("KAVA LAVAZZA", -2.00, 23.0),
            ],
            vat_summary: vec![],
        }
    }

    /// Answers every id with `receipt` and remembers the ids it was asked for.
    struct FakeApi {
        receipt: fn() -> Option<EkasaReceiptData>,
        asked: RefCell<Vec<String>>,
    }

    impl FakeApi {
        fn new(receipt: fn() -> Option<EkasaReceiptData>) -> Self {
            Self {
                receipt,
                asked: RefCell::new(Vec::new()),
            }
        }
    }

    impl EkasaApi for FakeApi {
        fn fetch(&self, receipt_id: &str) -> Result<Option<EkasaReceiptData>, AppError> {
            self.asked.borrow_mut().push(receipt_id.to_owned());
            Ok((self.receipt)())
        }
    }

    fn read_online(api: &FakeApi) -> ReceiptReading {
        read_receipt_file("/r/billa.jpg", Some(ONLINE_ID), None, api).unwrap()
    }

    fn facts(reading: &ReceiptReading) -> Vec<LineFacts<'_>> {
        reading
            .lines
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(LineFacts::from)
            .collect()
    }

    #[test]
    fn an_online_receipt_is_read_from_the_ekasa_service() {
        let api = FakeApi::new(|| Some(billa(15.28)));

        let reading = read_online(&api);

        assert_eq!(*api.asked.borrow(), [ONLINE_ID]);
        assert_eq!(reading.kind, ReceiptKind::Online);
        assert_eq!(reading.source_ref.as_str(), format!("online:{ONLINE_ID}"));
        assert_eq!(reading.receipt.code, ONLINE_ID);
        assert_eq!(reading.receipt.file, "/r/billa.jpg");
        assert_eq!(reading.date, "2026-10-12");
        assert_eq!(reading.vendor.as_deref(), Some("BILLA s.r.o."));
        assert_eq!(reading.total_minor, 1528);
        let lines: Vec<(&str, i64, i64)> = facts(&reading)
            .iter()
            .map(|l| (l.name, l.gross_minor, l.vat_rate_bps))
            .collect();
        assert_eq!(
            lines,
            [("KAVA LAVAZZA", 1399, 2300), ("MLIEKO 1L", 129, 500)]
        );
        assert!(reading.warnings.is_empty());
    }

    #[test]
    fn lines_that_do_not_add_up_to_the_total_are_a_warning() {
        let api = FakeApi::new(|| Some(billa(20.00)));

        let reading = read_online(&api);

        assert_eq!(
            reading.warnings,
            [ReadingWarning::LinesDifferFromTotal {
                lines_total_minor: 1528,
                total_minor: 2000,
            }]
        );
        assert_eq!(reading.lines.unwrap().len(), 2);
    }

    #[test]
    fn an_id_the_service_does_not_know_is_not_found() {
        let api = FakeApi::new(|| None);

        let err = read_receipt_file("/r/billa.jpg", Some(ONLINE_ID), None, &api).unwrap_err();

        assert_eq!(err.code, "RECEIPT_NOT_FOUND");
        assert!(err.message.contains(ONLINE_ID), "{}", err.message);
    }

    #[test]
    fn a_typed_code_must_be_an_online_receipt_id() {
        let api = FakeApi::new(|| Some(billa(15.28)));

        for typed in [OFFLINE_CODE, "hello", ""] {
            let err = read_receipt_file("/r/a.jpg", Some(typed), None, &api).unwrap_err();
            assert_eq!(err.code, "VALIDATION", "{typed:?}");
        }
        assert!(api.asked.borrow().is_empty());
    }

    #[test]
    fn a_receipt_date_from_the_service_must_be_a_date() {
        assert_eq!(api_date("03.05.2025 14:20:00").unwrap(), "2025-05-03");
        assert_eq!(api_date("31.12.2026").unwrap(), "2026-12-31");
        for bad in ["2025-05-03 14:20:00", "32.05.2025 10:00:00", ""] {
            assert_eq!(
                api_date(bad).unwrap_err().code,
                "EKASA_API_ERROR",
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_offline_fixture_decodes_to_its_code_without_the_service() {
        let api = FakeApi::new(|| panic!("an offline code never goes to the eKasa service"));

        let reading = read_receipt_file(OFFLINE_FIXTURE, None, None, &api).unwrap();

        assert_eq!(reading.kind, ReceiptKind::Offline);
        assert_eq!(reading.receipt.code, OFFLINE_CODE);
        assert_eq!(
            reading.source_ref.as_str(),
            format!("offline:{OFFLINE_CODE}")
        );
        assert_eq!(reading.date, "2018-02-13");
        assert_eq!(reading.total_minor, 23723);
        assert_eq!(reading.lines, None);
        assert_eq!(reading.vendor, None);
    }

    #[test]
    fn the_live_fetch_can_wait_on_the_runtime_inside_the_apps_blocking_task() {
        struct BlockingApi;
        impl EkasaApi for BlockingApi {
            fn fetch(&self, _: &str) -> Result<Option<EkasaReceiptData>, AppError> {
                tauri::async_runtime::block_on(async { Ok(Some(billa(15.28))) })
            }
        }

        let reading = tauri::async_runtime::block_on(async {
            tauri::async_runtime::spawn_blocking(|| {
                read_receipt_file("/r/billa.jpg", Some(ONLINE_ID), None, &BlockingApi)
            })
            .await
        })
        .unwrap()
        .unwrap();

        assert_eq!(reading.total_minor, 1528);
    }
}
