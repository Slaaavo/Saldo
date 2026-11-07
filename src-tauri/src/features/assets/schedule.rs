use super::business_use::AssetUseTerms;
use super::purchase::split_purchase;
use crate::error::AppError;
use crate::shared::{div_round, format_end_of_month};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetEntryKind {
    Depreciation,
    LostVat,
}

impl AssetEntryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AssetEntryKind::Depreciation => "depreciation",
            AssetEntryKind::LostVat => "lost_vat",
        }
    }
}

pub struct AssetFacts {
    pub name: String,
    pub purchase_price_minor: i64,
    pub purchase_date: String,
    pub depreciation_period_months: i64,
    pub use_terms: AssetUseTerms,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PlannedEntry {
    pub kind: AssetEntryKind,
    pub event_date: String,
    pub amount_minor: i64,
    pub expense_deductible_pct_bps: i64,
    pub note: String,
}

pub fn parse_ymd(date: &str) -> Result<(i32, i32, i32), AppError> {
    let invalid = |what: &str| AppError {
        code: "VALIDATION".into(),
        message: format!("Invalid {what} in date: {date}"),
    };
    let s = date.get(..10).ok_or_else(|| invalid("length"))?;
    let parts: Vec<&str> = s.splitn(3, '-').collect();
    if parts.len() != 3 {
        return Err(invalid("shape"));
    }
    let year: i32 = parts[0].parse().map_err(|_| invalid("year"))?;
    let month: i32 = parts[1].parse().map_err(|_| invalid("month"))?;
    let day: i32 = parts[2].parse().map_err(|_| invalid("day"))?;
    Ok((year, month, day))
}

fn add_months(year: i32, month: i32, offset: i64) -> (i32, u32) {
    let total_months = (year as i64) * 12 + (month as i64 - 1) + offset;
    let new_year = (total_months / 12) as i32;
    let new_month = (total_months % 12 + 1) as u32;
    (new_year, new_month)
}

fn purchase_datetime(purchase_date: &str) -> String {
    if purchase_date.len() == 10 {
        format!("{purchase_date}T00:00:00")
    } else {
        purchase_date.to_owned()
    }
}

fn monthly_depreciation(facts: &AssetFacts) -> Result<Vec<PlannedEntry>, AppError> {
    let (purchase_year, purchase_month, _) = parse_ymd(&facts.purchase_date)?;
    let last_eligible_offset = facts.depreciation_period_months - 1;
    let monthly_base = div_round(facts.purchase_price_minor, facts.depreciation_period_months);

    let mut entries = Vec::new();
    let mut cumulative_total: i64 = 0;
    let mut yearly_target: i64 = 0;
    let mut year_accumulated: i64 = 0;

    for offset in 0..=last_eligible_offset {
        let (event_year, event_month) = add_months(purchase_year, purchase_month, offset);

        if offset == 0 || event_month == 1 {
            let months_remaining_in_schedule = last_eligible_offset - offset + 1;
            let months_until_december = 13 - event_month as i64;
            let months_in_this_year_segment =
                months_remaining_in_schedule.min(months_until_december);
            let segment_contains_last =
                offset + months_in_this_year_segment - 1 == last_eligible_offset;
            yearly_target = if segment_contains_last {
                facts.purchase_price_minor - cumulative_total
            } else {
                div_round(
                    facts.purchase_price_minor * months_in_this_year_segment,
                    facts.depreciation_period_months,
                )
            };
            year_accumulated = 0;
        }

        let is_last_of_segment = event_month == 12 || offset == last_eligible_offset;
        let amount_minor = if is_last_of_segment {
            yearly_target - year_accumulated
        } else {
            monthly_base
        };

        year_accumulated += amount_minor;
        cumulative_total += amount_minor;

        entries.push(PlannedEntry {
            kind: AssetEntryKind::Depreciation,
            event_date: format_end_of_month(event_year, event_month),
            amount_minor,
            expense_deductible_pct_bps: facts.use_terms.pct_for(event_year, event_month),
            note: format!(
                "Depreciation: {} ({:02}/{})",
                facts.name, event_month, event_year
            ),
        });
    }

    Ok(entries)
}

pub fn asset_entries(facts: &AssetFacts) -> Result<Vec<PlannedEntry>, AppError> {
    let mut entries = if facts.depreciation_period_months > 0 {
        monthly_depreciation(facts)?
    } else {
        vec![PlannedEntry {
            kind: AssetEntryKind::Depreciation,
            event_date: purchase_datetime(&facts.purchase_date),
            amount_minor: facts.purchase_price_minor,
            expense_deductible_pct_bps: facts.use_terms.pct_at_purchase(),
            note: format!("Depreciation: {} (instant)", facts.name),
        }]
    };

    let lost_vat_minor = facts.use_terms.purchase_vat.map_or(0, |purchase_vat| {
        split_purchase(&purchase_vat, facts.use_terms.pct_at_purchase()).lost_vat_entry_minor
    });
    if lost_vat_minor > 0 {
        entries.push(PlannedEntry {
            kind: AssetEntryKind::LostVat,
            event_date: purchase_datetime(&facts.purchase_date),
            amount_minor: lost_vat_minor,
            expense_deductible_pct_bps: 0,
            note: format!("Lost VAT: {}", facts.name),
        });
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::assets::business_use::BusinessUseChange;
    use crate::features::assets::purchase::PurchaseVat;

    fn vehicle(gross_minor: i64, price_minor: i64, months: i64) -> AssetFacts {
        AssetFacts {
            name: "Vehicle".to_owned(),
            purchase_price_minor: price_minor,
            purchase_date: "2026-07-12".to_owned(),
            depreciation_period_months: months,
            use_terms: AssetUseTerms {
                purchase_vat: Some(PurchaseVat {
                    gross_minor,
                    vat_rate_bps: 2300,
                    vat_reclaimable_pct_bps: 5000,
                }),
                business_use_pct_bps: Some(8000),
                business_use_changes: Vec::new(),
            },
        }
    }

    fn depreciation_deductible_rest_in_2026(entries: &[PlannedEntry]) -> (i64, i64, i64) {
        let (depreciation, deductible) = entries
            .iter()
            .filter(|entry| entry.kind == AssetEntryKind::Depreciation)
            .filter(|entry| entry.event_date.starts_with("2026"))
            .fold((0, 0), |(total, deductible), entry| {
                (
                    total + entry.amount_minor,
                    deductible + entry.amount_minor * entry.expense_deductible_pct_bps / 10000,
                )
            });
        (depreciation, deductible, depreciation - deductible)
    }

    #[test]
    fn the_kona_matches_the_worked_example() {
        let entries = asset_entries(&vehicle(1_560_000, 1_268_292, 24)).unwrap();

        assert_eq!(entries[0].amount_minor, 52_846);
        assert_eq!(
            depreciation_deductible_rest_in_2026(&entries),
            (317_073, 253_654, 63_419)
        );
        assert_eq!(
            entries.last().unwrap(),
            &PlannedEntry {
                kind: AssetEntryKind::LostVat,
                event_date: "2026-07-12T00:00:00".to_owned(),
                amount_minor: 145_854,
                expense_deductible_pct_bps: 0,
                note: "Lost VAT: Vehicle".to_owned(),
            }
        );
    }

    #[test]
    fn the_honda_matches_the_worked_example() {
        let entries = asset_entries(&vehicle(830_000, 674_796, 72)).unwrap();

        assert_eq!(entries[0].amount_minor, 9_372);
        assert_eq!(
            depreciation_deductible_rest_in_2026(&entries),
            (56_233, 44_983, 11_250)
        );
        assert_eq!(entries.last().unwrap().amount_minor, 77_602);
    }

    #[test]
    fn the_peugeot_moves_425_03_to_non_deductible_costs() {
        let peugeot = AssetFacts {
            name: "Peugeot 5008".to_owned(),
            purchase_price_minor: 1_700_000,
            purchase_date: "2024-07-01".to_owned(),
            depreciation_period_months: 48,
            use_terms: AssetUseTerms {
                business_use_changes: vec![BusinessUseChange {
                    from_month: "2026-07".to_owned(),
                    business_use_pct_bps: 8000,
                }],
                ..Default::default()
            },
        };

        let entries = asset_entries(&peugeot).unwrap();

        assert_eq!(
            depreciation_deductible_rest_in_2026(&entries),
            (425_000, 382_497, 42_503)
        );
    }

    #[test]
    fn an_asset_with_no_lost_vat_gets_no_zero_entry() {
        let mut facts = vehicle(1_230_000, 1_000_000, 24);
        facts.use_terms.purchase_vat = Some(PurchaseVat {
            gross_minor: 1_230_000,
            vat_rate_bps: 2300,
            vat_reclaimable_pct_bps: 10000,
        });

        let entries = asset_entries(&facts).unwrap();

        assert!(entries
            .iter()
            .all(|entry| entry.kind == AssetEntryKind::Depreciation));
    }
}
