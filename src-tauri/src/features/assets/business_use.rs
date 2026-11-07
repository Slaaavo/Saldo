use serde::{Deserialize, Serialize};

use super::purchase::PurchaseVat;

pub const FULL_BUSINESS_USE_BPS: i64 = 10000;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessUseChange {
    /// `YYYY-MM`, so months compare as strings.
    pub from_month: String,
    pub business_use_pct_bps: i64,
}

/// The default is a plain asset: no VAT split, used only for business.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetUseTerms {
    pub purchase_vat: Option<PurchaseVat>,
    pub business_use_pct_bps: Option<i64>,
    #[serde(default)]
    pub business_use_changes: Vec<BusinessUseChange>,
}

impl AssetUseTerms {
    pub fn pct_at_purchase(&self) -> i64 {
        self.business_use_pct_bps.unwrap_or(FULL_BUSINESS_USE_BPS)
    }

    pub fn pct_for(&self, year: i32, month: u32) -> i64 {
        let month_key = format!("{year:04}-{month:02}");
        self.business_use_changes
            .iter()
            .filter(|change| change.from_month <= month_key)
            .max_by(|a, b| a.from_month.cmp(&b.from_month))
            .map_or(self.pct_at_purchase(), |change| change.business_use_pct_bps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peugeot() -> AssetUseTerms {
        AssetUseTerms {
            purchase_vat: None,
            business_use_pct_bps: None,
            business_use_changes: vec![
                BusinessUseChange {
                    from_month: "2027-03".into(),
                    business_use_pct_bps: 6000,
                },
                BusinessUseChange {
                    from_month: "2026-07".into(),
                    business_use_pct_bps: 8000,
                },
            ],
        }
    }

    #[test]
    fn a_month_before_any_change_takes_the_share_at_purchase() {
        assert_eq!(peugeot().pct_for(2026, 6), 10000);
    }

    #[test]
    fn a_change_starts_in_its_own_month() {
        assert_eq!(peugeot().pct_for(2026, 7), 8000);
    }

    #[test]
    fn a_later_month_takes_the_latest_change_before_it() {
        assert_eq!(peugeot().pct_for(2027, 2), 8000);
        assert_eq!(peugeot().pct_for(2027, 3), 6000);
        assert_eq!(peugeot().pct_for(2030, 1), 6000);
    }
}
