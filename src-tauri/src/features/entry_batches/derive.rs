use super::models::{AssetPart, DateSource, DocumentContent, DocumentItem, PriceBasis};
use crate::error::AppError;
use crate::features::tax_models::calculation::calculate_net_amount;
use crate::features::transactions::SourceRef;
use crate::shared::validate_iso_date;

/// The Slovak threshold for depreciable assets: a purchase price over 1,700.00 € and a useful
/// life over one year.
const ASSET_MIN_PRICE_EXCLUSIVE_MINOR: i64 = 170_000;
const ASSET_MIN_MONTHS_EXCLUSIVE: i64 = 12;

/// The facts Saldo derives from a document, by the rules the user settled.
pub(super) struct Derived {
    pub source_ref: SourceRef,
    pub entry_date: String,
    pub date_source: DateSource,
    pub entries: EntryShape,
    /// For expense kinds: whether the document's VAT is reclaimed. None for revenue.
    pub reclaimed_vat: Option<bool>,
    pub assets: Vec<DerivedAsset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EntryShape {
    None,
    Single,
    Split,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DerivedAsset {
    pub purchase_price_minor: i64,
    pub price_basis: PriceBasis,
}

/// `vat_payer` is the person's flag, the default for a document without `reclaimedVat`.
pub(super) fn document(item: &DocumentItem, vat_payer: bool) -> Result<Derived, AppError> {
    let source_ref = SourceRef::invoice(&item.document.issuer, &item.document.number)?;
    let (entry_date, date_source) = entry_date(&item.issue_date, item.delivery_date.as_deref())?;
    let (line_count, reclaimed_vat, assets) = match &item.content {
        DocumentContent::Revenue(revenue) => (revenue.lines.len(), None, Vec::new()),
        DocumentContent::Expense(expense) => {
            let reclaims = expense.reclaimed_vat.unwrap_or(vat_payer);
            let assets = expense
                .assets
                .iter()
                .map(|part| asset_price(part, reclaims))
                .collect::<Result<Vec<_>, _>>()?;
            (expense.lines.len(), Some(reclaims), assets)
        }
        DocumentContent::PrepaidExpense(prepaid) => (
            1,
            Some(prepaid.reclaimed_vat.unwrap_or(vat_payer)),
            Vec::new(),
        ),
    };
    let entries = entry_shape(line_count);
    if entries == EntryShape::None && assets.is_empty() {
        return Err(invalid(
            "The document has nothing to record: give it at least one line or asset",
        ));
    }
    if entries == EntryShape::None && !item.payment_ids.is_empty() {
        return Err(invalid(
            "A document with only assets cannot link payments: Saldo links payments to entries, \
             not to assets. Leave paymentIds empty.",
        ));
    }
    Ok(Derived {
        source_ref,
        entry_date,
        date_source,
        entries,
        reclaimed_vat,
        assets,
    })
}

/// The delivery date (dátum dodania) when the document has one, the issue date otherwise.
pub(super) fn entry_date(
    issue_date: &str,
    delivery_date: Option<&str>,
) -> Result<(String, DateSource), AppError> {
    validate_iso_date(issue_date, "issueDate")?;
    match delivery_date {
        Some(delivery) => {
            validate_iso_date(delivery, "deliveryDate")?;
            Ok((delivery.to_owned(), DateSource::DeliveryDate))
        }
        None => Ok((issue_date.to_owned(), DateSource::IssueDate)),
    }
}

/// One line is one entry, two or more lines are a split group.
pub(super) fn entry_shape(line_count: usize) -> EntryShape {
    match line_count {
        0 => EntryShape::None,
        1 => EntryShape::Single,
        _ => EntryShape::Split,
    }
}

/// The price depreciation spreads: without VAT when the VAT is reclaimed, with it otherwise.
pub(super) fn asset_price(part: &AssetPart, reclaims_vat: bool) -> Result<DerivedAsset, AppError> {
    if part.name.trim().is_empty() {
        return Err(invalid("An asset needs a name"));
    }
    let (purchase_price_minor, price_basis) = if reclaims_vat {
        (
            calculate_net_amount(part.gross_minor, part.vat_rate_bps.get()),
            PriceBasis::NetVatReclaimed,
        )
    } else {
        (part.gross_minor, PriceBasis::GrossVatNotReclaimed)
    };
    if purchase_price_minor <= ASSET_MIN_PRICE_EXCLUSIVE_MINOR
        || part.depreciation_months <= ASSET_MIN_MONTHS_EXCLUSIVE
    {
        return Err(invalid(&format!(
            "'{}' costs {} over {} months. An asset needs a price over 1700.00 and a useful \
             life over 12 months; record it as an expense line instead.",
            part.name,
            major_units(purchase_price_minor),
            part.depreciation_months
        )));
    }
    Ok(DerivedAsset {
        purchase_price_minor,
        price_basis,
    })
}

fn major_units(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", minor.abs() / 100, minor.abs() % 100)
}

fn invalid(message: &str) -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::entry_batches::models::{Bps, DocumentId, ExpenseContent, ExpenseLine};

    fn bps(value: i64) -> Bps {
        Bps::try_from(value).unwrap()
    }

    fn laptop(gross_minor: i64, depreciation_months: i64) -> AssetPart {
        AssetPart {
            name: "MacBook Pro 16".to_owned(),
            gross_minor,
            vat_rate_bps: bps(2300),
            depreciation_months,
        }
    }

    fn line(gross_minor: i64) -> ExpenseLine {
        ExpenseLine {
            gross_minor,
            vat_rate_bps: bps(2300),
            deductible_pct_bps: bps(10000),
            vat_reclaimable_pct_bps: bps(10000),
        }
    }

    fn expense(
        lines: Vec<ExpenseLine>,
        assets: Vec<AssetPart>,
        payment_ids: Vec<i64>,
    ) -> DocumentItem {
        DocumentItem {
            document: DocumentId {
                issuer: "36562939".to_owned(),
                number: "FA20261187".to_owned(),
                file: "alza.pdf".to_owned(),
            },
            person_id: 1,
            issue_date: "2026-10-12".to_owned(),
            delivery_date: None,
            note: None,
            payment_ids,
            content: DocumentContent::Expense(ExpenseContent {
                lines,
                assets,
                reclaimed_vat: None,
            }),
        }
    }

    #[test]
    fn the_delivery_date_wins_over_the_issue_date() {
        assert_eq!(
            entry_date("2026-10-06", Some("2026-09-30")).unwrap(),
            ("2026-09-30".to_owned(), DateSource::DeliveryDate)
        );
    }

    #[test]
    fn without_a_delivery_date_the_issue_date_counts() {
        assert_eq!(
            entry_date("2026-10-06", None).unwrap(),
            ("2026-10-06".to_owned(), DateSource::IssueDate)
        );
    }

    #[test]
    fn both_dates_must_be_dates() {
        assert!(entry_date("06.10.2026", None).is_err());
        assert!(entry_date("2026-10-06", Some("2026-9-30")).is_err());
        assert!(entry_date("2026-13-01", Some("2026-09-30")).is_err());
    }

    #[test]
    fn one_line_is_one_entry_and_more_lines_are_a_split_group() {
        assert_eq!(entry_shape(0), EntryShape::None);
        assert_eq!(entry_shape(1), EntryShape::Single);
        assert_eq!(entry_shape(2), EntryShape::Split);
        assert_eq!(entry_shape(5), EntryShape::Split);
    }

    #[test]
    fn an_asset_costs_its_price_without_vat_when_the_vat_is_reclaimed() {
        // 3,599.00 with 23 % VAT is 2,926.01 without it.
        assert_eq!(
            asset_price(&laptop(359_900, 48), true).unwrap(),
            DerivedAsset {
                purchase_price_minor: 292_601,
                price_basis: PriceBasis::NetVatReclaimed,
            }
        );
    }

    #[test]
    fn an_asset_costs_its_full_price_when_the_vat_is_not_reclaimed() {
        assert_eq!(
            asset_price(&laptop(359_900, 48), false).unwrap(),
            DerivedAsset {
                purchase_price_minor: 359_900,
                price_basis: PriceBasis::GrossVatNotReclaimed,
            }
        );
    }

    #[test]
    fn an_asset_must_cost_over_1700_euros() {
        assert!(asset_price(&laptop(170_000, 48), false).is_err());
        assert!(asset_price(&laptop(170_001, 48), false).is_ok());
        // 2,091.00 with VAT is exactly 1,700.00 without it.
        assert!(asset_price(&laptop(209_100, 48), true).is_err());
    }

    #[test]
    fn an_asset_must_last_over_twelve_months() {
        assert!(asset_price(&laptop(359_900, 12), false).is_err());
        assert!(asset_price(&laptop(359_900, 13), false).is_ok());
    }

    #[test]
    fn an_expense_reclaims_vat_by_the_persons_flag_unless_it_says_otherwise() {
        let item = expense(vec![], vec![laptop(359_900, 48)], vec![]);

        let payer = document(&item, true).unwrap();
        let non_payer = document(&item, false).unwrap();

        assert_eq!(payer.reclaimed_vat, Some(true));
        assert_eq!(payer.assets[0].purchase_price_minor, 292_601);
        assert_eq!(non_payer.assets[0].purchase_price_minor, 359_900);
    }

    #[test]
    fn a_document_needs_something_to_record() {
        assert!(document(&expense(vec![], vec![], vec![]), true).is_err());
    }

    #[test]
    fn a_document_with_only_assets_cannot_link_a_payment() {
        let with_payment = expense(vec![], vec![laptop(359_900, 48)], vec![5233]);
        let with_a_line = expense(vec![line(3690)], vec![laptop(359_900, 48)], vec![5233]);

        assert_eq!(
            document(&with_payment, true).err().unwrap().code,
            "VALIDATION"
        );
        assert!(document(&with_a_line, true).is_ok());
    }

    #[test]
    fn the_reference_is_built_from_issuer_and_number() {
        let derived = document(&expense(vec![line(3690)], vec![], vec![]), true).unwrap();

        assert_eq!(derived.source_ref.as_str(), "invoice:36562939/FA20261187");
        assert_eq!(derived.entries, EntryShape::Single);
    }
}
