use serde::{Deserialize, Serialize};

use super::business_use::{check_bps, FULL_BUSINESS_USE_BPS};
use crate::error::AppError;
use crate::features::tax_models::calculation::{
    calculate_net_amount, calculate_reclaimable_vat, calculate_vat_amount,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseVat {
    pub gross_minor: i64,
    pub vat_rate_bps: i64,
    pub vat_reclaimable_pct_bps: i64,
}

impl PurchaseVat {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.gross_minor <= 0 {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "The price with VAT must be above zero".into(),
            });
        }
        check_bps("The VAT rate", self.vat_rate_bps)?;
        check_bps("VAT reclaimed", self.vat_reclaimable_pct_bps)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseSplit {
    pub depreciating_minor: i64,
    pub reclaimed_vat_minor: i64,
    pub lost_vat_entry_minor: i64,
    pub start_value_minor: i64,
}

pub fn split_purchase(purchase: &PurchaseVat, business_use_pct_bps: i64) -> PurchaseSplit {
    let net_minor = calculate_net_amount(purchase.gross_minor, purchase.vat_rate_bps);
    let vat_minor = calculate_vat_amount(purchase.gross_minor, net_minor);
    let reclaimed_vat_minor =
        calculate_reclaimable_vat(vat_minor, purchase.vat_reclaimable_pct_bps, true);
    let some_vat_comes_back = reclaimed_vat_minor > 0;
    let mixed_use = business_use_pct_bps < FULL_BUSINESS_USE_BPS;

    if some_vat_comes_back && mixed_use {
        return PurchaseSplit {
            depreciating_minor: net_minor,
            reclaimed_vat_minor,
            lost_vat_entry_minor: vat_minor - reclaimed_vat_minor,
            start_value_minor: purchase.gross_minor,
        };
    }

    let cost_minor = purchase.gross_minor - reclaimed_vat_minor;
    PurchaseSplit {
        depreciating_minor: cost_minor,
        reclaimed_vat_minor,
        lost_vat_entry_minor: 0,
        start_value_minor: cost_minor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn purchase(gross_minor: i64, vat_rate_bps: i64, reclaimable: i64) -> PurchaseVat {
        PurchaseVat {
            gross_minor,
            vat_rate_bps,
            vat_reclaimable_pct_bps: reclaimable,
        }
    }

    #[test]
    fn no_reclaim_keeps_the_full_price_at_any_business_use() {
        for business_use in [10000, 8000, 0] {
            assert_eq!(
                split_purchase(&purchase(1_560_000, 2300, 0), business_use),
                PurchaseSplit {
                    depreciating_minor: 1_560_000,
                    reclaimed_vat_minor: 0,
                    lost_vat_entry_minor: 0,
                    start_value_minor: 1_560_000,
                }
            );
        }
    }

    #[test]
    fn full_business_use_keeps_lost_vat_in_the_price() {
        assert_eq!(
            split_purchase(&purchase(1_560_000, 2300, 5000), 10000),
            PurchaseSplit {
                depreciating_minor: 1_414_146,
                reclaimed_vat_minor: 145_854,
                lost_vat_entry_minor: 0,
                start_value_minor: 1_414_146,
            }
        );
    }

    #[test]
    fn full_reclaim_at_full_business_use_starts_at_the_net_price() {
        assert_eq!(
            split_purchase(&purchase(1_230_000, 2300, 10000), 10000),
            PurchaseSplit {
                depreciating_minor: 1_000_000,
                reclaimed_vat_minor: 230_000,
                lost_vat_entry_minor: 0,
                start_value_minor: 1_000_000,
            }
        );
    }

    #[test]
    fn the_kona_splits_into_net_reclaimed_and_lost_vat() {
        assert_eq!(
            split_purchase(&purchase(1_560_000, 2300, 5000), 8000),
            PurchaseSplit {
                depreciating_minor: 1_268_292,
                reclaimed_vat_minor: 145_854,
                lost_vat_entry_minor: 145_854,
                start_value_minor: 1_560_000,
            }
        );
    }

    #[test]
    fn the_honda_splits_into_net_reclaimed_and_lost_vat() {
        assert_eq!(
            split_purchase(&purchase(830_000, 2300, 5000), 8000),
            PurchaseSplit {
                depreciating_minor: 674_796,
                reclaimed_vat_minor: 77_602,
                lost_vat_entry_minor: 77_602,
                start_value_minor: 830_000,
            }
        );
    }

    #[test]
    fn mixed_use_with_full_reclaim_has_no_lost_vat_entry() {
        assert_eq!(
            split_purchase(&purchase(1_230_000, 2300, 10000), 8000),
            PurchaseSplit {
                depreciating_minor: 1_000_000,
                reclaimed_vat_minor: 230_000,
                lost_vat_entry_minor: 0,
                start_value_minor: 1_230_000,
            }
        );
    }

    #[test]
    fn the_parts_always_add_up_to_the_gross() {
        for reclaimable in [0, 2500, 5000, 10000] {
            for business_use in [0, 8000, 10000] {
                let split = split_purchase(&purchase(999_999, 2300, reclaimable), business_use);
                assert_eq!(
                    split.depreciating_minor
                        + split.reclaimed_vat_minor
                        + split.lost_vat_entry_minor,
                    999_999
                );
            }
        }
    }
}
