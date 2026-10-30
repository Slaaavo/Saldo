use super::models::{
    AssetPart, DateSource, DocumentContent, DocumentItem, ExpenseLine, PriceBasis, ReceiptItem,
    ReceiptLine, ShareBy, ShareSource,
};
use crate::error::AppError;
use crate::features::ekasa::reading::{check_receipt, LineFacts};
use crate::features::ekasa::rules::{ShareRules, Shares};
use crate::features::tax_models::calculation::calculate_net_amount;
use crate::features::transactions::models::TaxableKind;
use crate::features::transactions::SourceRef;
use crate::shared::{major_units, validate_iso_date};

/// The Slovak threshold for depreciable assets: a purchase price over 1,700.00 € and a useful
/// life over one year.
const ASSET_MIN_PRICE_EXCLUSIVE_MINOR: i64 = 170_000;
const ASSET_MIN_MONTHS_EXCLUSIVE: i64 = 12;

/// Everything one item writes, whether it was an invoice or a receipt. The writer reads only
/// this. It is part of the review token, so what Saldo took from the database (eKasa rule
/// shares, the VAT-payer default) is part of what the user approved.
#[derive(Debug, Hash)]
pub(super) struct Plan {
    pub source_ref: SourceRef,
    pub file: String,
    pub person_id: i64,
    pub kind: TaxableKind,
    pub entry_date: String,
    pub date_source: DateSource,
    /// Written only on a split group.
    pub group_note: Option<String>,
    pub legs: Vec<Leg>,
    pub prepaid_until: Option<String>,
    /// For expense kinds: whether the VAT is reclaimed. None for revenue.
    pub reclaimed_vat: Option<bool>,
    pub assets: Vec<AssetPlan>,
    pub payment_ids: Vec<i64>,
}

#[derive(Debug, Hash)]
pub(super) struct Leg {
    pub gross_minor: i64,
    pub vat_rate_bps: i64,
    pub note: Option<String>,
    /// None for revenue.
    pub shares: Option<Shares>,
    /// For a receipt line: where its shares came from.
    pub source: Option<ShareSource>,
}

#[derive(Debug, Hash)]
pub(super) struct AssetPlan {
    pub name: String,
    pub gross_minor: i64,
    pub purchase_price_minor: i64,
    pub price_basis: PriceBasis,
    pub depreciation_months: i64,
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

impl Plan {
    pub fn entries(&self) -> EntryShape {
        entry_shape(self.legs.len())
    }

    pub fn legs_total_minor(&self) -> i64 {
        self.legs.iter().map(|leg| leg.gross_minor).sum()
    }

    /// Legs and assets, VAT included.
    pub fn total_gross_minor(&self) -> i64 {
        self.legs_total_minor() + self.assets.iter().map(|a| a.gross_minor).sum::<i64>()
    }

    fn check_entries(self) -> Result<Self, AppError> {
        if self.entries() == EntryShape::None && self.assets.is_empty() {
            return Err(invalid(
                "The document has nothing to record: give it at least one line or asset",
            ));
        }
        if self.entries() == EntryShape::None && !self.payment_ids.is_empty() {
            return Err(invalid(
                "A document with only assets cannot link payments: Saldo links payments to \
                 entries, not to assets. Leave paymentIds empty.",
            ));
        }
        Ok(self)
    }
}

/// An invoice or credit note: every leg and the split group carry the document's note.
pub(super) fn document(item: &DocumentItem, default_reclaims_vat: bool) -> Result<Plan, AppError> {
    let source_ref = SourceRef::invoice(&item.document.issuer, &item.document.number)?;
    let (entry_date, date_source) = entry_date(&item.issue_date, item.delivery_date.as_deref())?;
    let leg = |gross_minor: i64, vat_rate_bps: i64, shares: Option<Shares>| Leg {
        gross_minor,
        vat_rate_bps,
        note: item.note.clone(),
        shares,
        source: None,
    };
    let expense_leg = |line: &ExpenseLine| {
        leg(
            line.gross_minor,
            line.vat_rate_bps.get(),
            Some(Shares {
                deductible_pct_bps: line.deductible_pct_bps.get(),
                vat_reclaimable_pct_bps: line.vat_reclaimable_pct_bps.get(),
            }),
        )
    };
    let (kind, legs, prepaid_until, reclaimed_vat, assets) = match &item.content {
        DocumentContent::Revenue(revenue) => (
            TaxableKind::Revenue,
            revenue
                .lines
                .iter()
                .map(|line| leg(line.gross_minor, line.vat_rate_bps.get(), None))
                .collect(),
            None,
            None,
            Vec::new(),
        ),
        DocumentContent::Expense(expense) => {
            let reclaims = expense.reclaimed_vat.unwrap_or(default_reclaims_vat);
            let assets = expense
                .assets
                .iter()
                .map(|part| asset_plan(part, reclaims))
                .collect::<Result<Vec<_>, _>>()?;
            (
                TaxableKind::Expense,
                expense.lines.iter().map(expense_leg).collect(),
                None,
                Some(reclaims),
                assets,
            )
        }
        DocumentContent::PrepaidExpense(prepaid) => (
            TaxableKind::PrepaidExpense,
            vec![expense_leg(&prepaid.line)],
            Some(prepaid.prepaid_until.clone()),
            Some(prepaid.reclaimed_vat.unwrap_or(default_reclaims_vat)),
            Vec::new(),
        ),
    };
    Plan {
        source_ref,
        file: item.document.file.clone(),
        person_id: item.person_id,
        kind,
        entry_date,
        date_source,
        group_note: item.note.clone(),
        legs,
        prepaid_until,
        reclaimed_vat,
        assets,
        payment_ids: item.payment_ids.clone(),
    }
    .check_entries()
}

/// An eKasa receipt: one expense dated by the receipt, a leg per line named after it, the shop
/// as the split group's note, and an asset for each line marked as one. `rules` is the person's
/// eKasa profile, None when the person has none.
pub(super) fn receipt(
    item: &ReceiptItem,
    default_reclaims_vat: bool,
    rules: Option<&ShareRules>,
) -> Result<Plan, AppError> {
    if item.lines.is_empty() {
        return Err(invalid("A receipt needs its lines"));
    }
    if item.vendor.trim().is_empty() {
        return Err(invalid(
            "A receipt needs its vendor: read_receipt's vendor, or the shop printed on an \
             offline receipt",
        ));
    }
    if let Some(index) = item.lines.iter().position(|l| l.name.trim().is_empty()) {
        return Err(invalid(&format!(
            "lines[{index}] needs the item's name as printed"
        )));
    }
    let facts: Vec<LineFacts> = item
        .lines
        .iter()
        .map(|line| LineFacts {
            name: &line.name,
            gross_minor: line.gross_minor,
            vat_rate_bps: line.vat_rate_bps.get(),
        })
        .collect();
    let code = check_receipt(&item.receipt, &item.date, &facts)?;
    let reclaims = item.reclaimed_vat.unwrap_or(default_reclaims_vat);

    let mut legs = Vec::new();
    let mut assets = Vec::new();
    let mut missing = Vec::new();
    for (index, line) in item.lines.iter().enumerate() {
        if let Some(asset) = &line.asset {
            if line.shares.is_some() {
                return Err(invalid(&format!(
                    "lines[{index}] is an asset, so it takes no shares"
                )));
            }
            assets.push(asset_plan(
                &AssetPart {
                    name: line.name.clone(),
                    gross_minor: line.gross_minor,
                    vat_rate_bps: line.vat_rate_bps,
                    depreciation_months: asset.depreciation_months,
                },
                reclaims,
            )?);
            continue;
        }
        match line_shares(line, rules) {
            Some((shares, source)) => legs.push(Leg {
                gross_minor: line.gross_minor,
                vat_rate_bps: line.vat_rate_bps.get(),
                note: Some(line.name.clone()),
                shares: Some(shares),
                source: Some(source),
            }),
            None => missing.push(format!("lines[{index}] '{}'", line.name)),
        }
    }
    if !missing.is_empty() {
        return Err(AppError {
            code: "SHARES_NEEDED".into(),
            message: format!(
                "No share was given and no eKasa rule of person {} matches {}. Give your own \
                 judgment as shares with by \"claude\".",
                item.person_id,
                missing.join(", ")
            ),
        });
    }
    Plan {
        source_ref: code.source_ref(),
        file: item.receipt.file.clone(),
        person_id: item.person_id,
        kind: TaxableKind::Expense,
        entry_date: item.date.clone(),
        date_source: DateSource::ReceiptDate,
        group_note: Some(item.vendor.trim().to_owned()),
        legs,
        prepaid_until: None,
        reclaimed_vat: Some(reclaims),
        assets,
        payment_ids: item.payment_ids.clone(),
    }
    .check_entries()
}

/// A share the user dictated wins, then the matching rule, then Claude's judgment.
fn line_shares(line: &ReceiptLine, rules: Option<&ShareRules>) -> Option<(Shares, ShareSource)> {
    let rule = rules
        .and_then(|rules| rules.shares_for(&line.name))
        .cloned();
    let given = line.shares.map(|given| {
        let shares = Shares {
            deductible_pct_bps: given.deductible_pct_bps.get(),
            vat_reclaimable_pct_bps: given.vat_reclaimable_pct_bps.get(),
        };
        (shares, given.by)
    });
    match (given, rule) {
        (Some((shares, ShareBy::User)), rule) => Some((shares, ShareSource::User { rule })),
        (claude, Some(rule)) => Some((
            rule.shares,
            ShareSource::Rule {
                claude: claude.map(|(shares, _)| shares),
                rule,
            },
        )),
        (Some((shares, ShareBy::Claude)), None) => Some((shares, ShareSource::Claude)),
        (None, None) => None,
    }
}

fn asset_plan(part: &AssetPart, reclaims_vat: bool) -> Result<AssetPlan, AppError> {
    let price = asset_price(part, reclaims_vat)?;
    Ok(AssetPlan {
        name: part.name.trim().to_owned(),
        gross_minor: part.gross_minor,
        purchase_price_minor: price.purchase_price_minor,
        price_basis: price.price_basis,
        depreciation_months: part.depreciation_months,
    })
}

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

pub(super) fn entry_shape(line_count: usize) -> EntryShape {
    match line_count {
        0 => EntryShape::None,
        1 => EntryShape::Single,
        _ => EntryShape::Split,
    }
}

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

fn invalid(message: &str) -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::ekasa::reading::fake::FakeEkasaApi;
    use crate::features::ekasa::rules::{RuleMatch, RuleSource};
    use crate::features::entry_batches::models::{Bps, DocumentId, ExpenseContent, LineShares};

    const GROSS_NETTING_TO_THE_THRESHOLD_MINOR: i64 = 209_100;

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
        assert!(asset_price(&laptop(GROSS_NETTING_TO_THE_THRESHOLD_MINOR, 48), true).is_err());
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
        assert_eq!(derived.entries(), EntryShape::Single);
    }

    #[test]
    fn every_leg_of_an_invoice_carries_its_note() {
        let mut item = expense(vec![line(3690), line(1200)], vec![], vec![]);
        item.note = Some("Alza FA20261187".to_owned());

        let plan = document(&item, true).unwrap();

        assert_eq!(plan.group_note.as_deref(), Some("Alza FA20261187"));
        assert!(plan
            .legs
            .iter()
            .all(|leg| leg.note.as_deref() == Some("Alza FA20261187") && leg.source.is_none()));
    }

    const BILLA: &str = "O-7A3F0C1E9B2D4C58A6E1F0D3B5C7A9E2";

    fn shares(by: ShareBy, deductible: i64, vat_reclaimable: i64) -> LineShares {
        LineShares {
            deductible_pct_bps: bps(deductible),
            vat_reclaimable_pct_bps: bps(vat_reclaimable),
            by,
        }
    }

    /// A receipt read online with these lines (name, gross, VAT bps), each sent with its shares.
    fn receipt_item(lines: &[(&'static str, i64, i64, Option<LineShares>)]) -> ReceiptItem {
        let reading = FakeEkasaApi {
            receipt_id: BILLA,
            lines: lines.iter().map(|&(n, g, v, _)| (n, g, v)).collect(),
        }
        .read();
        ReceiptItem {
            receipt: reading.receipt,
            person_id: 1,
            date: reading.date,
            vendor: reading.vendor.unwrap(),
            payment_ids: vec![],
            reclaimed_vat: None,
            lines: lines
                .iter()
                .map(|&(name, gross_minor, vat_rate_bps, shares)| ReceiptLine {
                    name: name.to_owned(),
                    gross_minor,
                    vat_rate_bps: bps(vat_rate_bps),
                    shares,
                    asset: None,
                })
                .collect(),
        }
    }

    fn rules(patterns: &[(&str, i64)]) -> ShareRules {
        ShareRules::compile(patterns.iter().map(|&(pattern, bps)| RuleSource {
            pattern,
            shares: Shares {
                deductible_pct_bps: bps,
                vat_reclaimable_pct_bps: bps,
            },
        }))
    }

    fn sources(plan: &Plan) -> Vec<(i64, &ShareSource)> {
        plan.legs
            .iter()
            .map(|leg| {
                (
                    leg.shares.unwrap().deductible_pct_bps,
                    leg.source.as_ref().unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn a_share_the_user_dictated_wins_then_the_rule_then_claude() {
        let item = receipt_item(&[
            ("KAVA", 1599, 2300, Some(shares(ShareBy::User, 5000, 5000))),
            (
                "MLIEKO",
                129,
                500,
                Some(shares(ShareBy::Claude, 10000, 10000)),
            ),
            ("ROZOK", 30, 500, None),
            (
                "PAPIER",
                662,
                2300,
                Some(shares(ShareBy::Claude, 8000, 8000)),
            ),
        ]);
        let rules = rules(&[("kava", 10000), ("mlieko|rozok", 0)]);

        let plan = receipt(&item, true, Some(&rules)).unwrap();

        let rule = |position: usize, pattern: &str, bps: i64| RuleMatch {
            position,
            pattern: pattern.to_owned(),
            shares: Shares {
                deductible_pct_bps: bps,
                vat_reclaimable_pct_bps: bps,
            },
        };
        assert_eq!(
            sources(&plan),
            [
                (
                    5000,
                    &ShareSource::User {
                        rule: Some(rule(1, "kava", 10000))
                    }
                ),
                (
                    0,
                    &ShareSource::Rule {
                        rule: rule(2, "mlieko|rozok", 0),
                        claude: Some(Shares {
                            deductible_pct_bps: 10000,
                            vat_reclaimable_pct_bps: 10000
                        }),
                    }
                ),
                (
                    0,
                    &ShareSource::Rule {
                        rule: rule(2, "mlieko|rozok", 0),
                        claude: None,
                    }
                ),
                (8000, &ShareSource::Claude),
            ]
        );
    }

    #[test]
    fn without_a_profile_only_given_shares_count() {
        let item = receipt_item(&[
            ("KAVA", 1599, 2300, Some(shares(ShareBy::User, 5000, 5000))),
            ("MLIEKO", 129, 500, None),
        ]);

        let err = receipt(&item, true, None).unwrap_err();

        assert_eq!(err.code, "SHARES_NEEDED");
        assert!(err.message.contains("lines[1] 'MLIEKO'"), "{}", err.message);
    }

    #[test]
    fn a_receipt_is_dated_by_itself_with_its_lines_as_notes_and_the_shop_as_group_note() {
        let item = receipt_item(&[
            (
                "KAVA",
                1599,
                2300,
                Some(shares(ShareBy::Claude, 10000, 10000)),
            ),
            ("MLIEKO", 129, 500, Some(shares(ShareBy::Claude, 0, 0))),
        ]);

        let plan = receipt(&item, false, None).unwrap();

        assert_eq!(plan.source_ref.as_str(), format!("online:{BILLA}"));
        assert_eq!(plan.kind, TaxableKind::Expense);
        assert_eq!(plan.entry_date, "2026-10-12");
        assert_eq!(plan.date_source, DateSource::ReceiptDate);
        assert_eq!(plan.group_note.as_deref(), Some("BILLA s.r.o."));
        let notes: Vec<Option<&str>> = plan.legs.iter().map(|l| l.note.as_deref()).collect();
        assert_eq!(notes, [Some("KAVA"), Some("MLIEKO")]);
        assert_eq!(plan.reclaimed_vat, Some(false));
        assert_eq!(plan.total_gross_minor(), 1728);
    }

    #[test]
    fn a_receipt_needs_a_vendor_and_named_lines() {
        let mut no_vendor = receipt_item(&[("KAVA", 1599, 2300, None)]);
        no_vendor.vendor = " ".to_owned();
        let mut no_name = receipt_item(&[("KAVA", 1599, 2300, None)]);
        no_name.lines[0].name = String::new();

        for item in [no_vendor, no_name] {
            assert_eq!(receipt(&item, true, None).unwrap_err().code, "VALIDATION");
        }
    }
}
