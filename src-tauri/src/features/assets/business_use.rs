use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::purchase::PurchaseVat;
use crate::error::AppError;
use crate::shared::is_iso_date;

pub const FULL_BUSINESS_USE_BPS: i64 = 10000;

fn invalid(message: String) -> AppError {
    AppError {
        code: "VALIDATION".into(),
        message,
    }
}

pub(crate) fn check_bps(what: &str, bps: i64) -> Result<(), AppError> {
    if (0..=10000).contains(&bps) {
        Ok(())
    } else {
        Err(invalid(format!("{what} must be between 0 % and 100 %")))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessUseChange {
    pub from_month: String,
    pub business_use_pct_bps: i64,
}

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

    pub fn is_plain(&self) -> bool {
        self.purchase_vat.is_none()
            && self.pct_at_purchase() == FULL_BUSINESS_USE_BPS
            && self.business_use_changes.is_empty()
    }

    pub fn validate(
        &self,
        purchase_month: Option<&str>,
        depreciation_period_months: Option<i64>,
    ) -> Result<(), AppError> {
        if let Some(purchase_vat) = &self.purchase_vat {
            purchase_vat.validate()?;
        }
        check_bps("Business use", self.pct_at_purchase())?;
        if self.is_plain() {
            return Ok(());
        }
        let Some(purchase_month) = purchase_month else {
            return Err(invalid(
                "A business-use share needs a purchase price and date".into(),
            ));
        };
        if !self.business_use_changes.is_empty() && depreciation_period_months.is_none() {
            return Err(invalid(
                "A business-use change needs a depreciation period".into(),
            ));
        }
        let mut months = HashSet::new();
        for change in &self.business_use_changes {
            let month = change.from_month.as_str();
            if !is_iso_date(&format!("{month}-01")) {
                return Err(invalid(format!(
                    "'{month}' is not a month in YYYY-MM format"
                )));
            }
            check_bps("Business use", change.business_use_pct_bps)?;
            if month <= purchase_month {
                return Err(invalid(format!(
                    "The business-use change from {month} must start after the purchase month"
                )));
            }
            if !months.insert(month) {
                return Err(invalid(format!(
                    "There are two business-use changes from {month}"
                )));
            }
        }
        Ok(())
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

    fn message_for(
        terms: &AssetUseTerms,
        purchase_month: Option<&str>,
        months: Option<i64>,
    ) -> String {
        terms.validate(purchase_month, months).unwrap_err().message
    }

    fn with_change(from_month: &str, business_use_pct_bps: i64) -> AssetUseTerms {
        AssetUseTerms {
            business_use_changes: vec![BusinessUseChange {
                from_month: from_month.to_owned(),
                business_use_pct_bps,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn valid_terms_pass() {
        assert!(peugeot().validate(Some("2024-07"), Some(48)).is_ok());
    }

    #[test]
    fn plain_terms_need_no_purchase_month() {
        let terms = AssetUseTerms {
            business_use_pct_bps: Some(10000),
            ..Default::default()
        };
        assert!(terms.validate(None, None).is_ok());
    }

    #[test]
    fn a_share_needs_a_purchase_month() {
        let terms = AssetUseTerms {
            business_use_pct_bps: Some(8000),
            ..Default::default()
        };
        assert!(message_for(&terms, None, Some(48)).contains("purchase price and date"));
    }

    #[test]
    fn a_share_outside_0_to_100_percent_is_refused() {
        let terms = AssetUseTerms {
            business_use_pct_bps: Some(10001),
            ..Default::default()
        };
        assert!(message_for(&terms, Some("2026-07"), Some(48)).contains("between"));
        assert!(
            message_for(&with_change("2026-08", -1), Some("2026-07"), Some(48)).contains("between")
        );
    }

    #[test]
    fn a_change_needs_a_depreciation_period() {
        let message = message_for(&with_change("2026-08", 8000), Some("2026-07"), None);
        assert!(message.contains("depreciation period"));
    }

    #[test]
    fn a_change_month_must_be_well_formed() {
        for month in ["2026-7", "2026-13", "26-07", "2026-07-01"] {
            let message = message_for(&with_change(month, 8000), Some("2020-01"), Some(48));
            assert!(message.contains("YYYY-MM"), "{month}: {message}");
        }
    }

    #[test]
    fn a_change_must_start_after_the_purchase_month() {
        for month in ["2026-06", "2026-07"] {
            let message = message_for(&with_change(month, 8000), Some("2026-07"), Some(48));
            assert!(message.contains(month), "{message}");
        }
    }

    #[test]
    fn two_changes_from_one_month_are_refused() {
        let mut terms = with_change("2026-08", 8000);
        terms.business_use_changes.push(BusinessUseChange {
            from_month: "2026-08".to_owned(),
            business_use_pct_bps: 6000,
        });
        assert!(message_for(&terms, Some("2026-07"), Some(48)).contains("two"));
    }
}
