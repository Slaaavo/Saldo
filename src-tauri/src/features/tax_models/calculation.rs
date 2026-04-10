use crate::features::tax_models::models::{
    EventDataForCalc, TaxCalculationResult, TaxEventBreakdown, TaxModelBracketRow, TaxModelDetail,
    TaxTier,
};

pub fn calculate_net_amount(amount_minor: i64, vat_rate_bps: i64) -> i64 {
    if vat_rate_bps == 0 {
        return amount_minor;
    }
    amount_minor * 10000 / (10000 + vat_rate_bps)
}

pub fn calculate_vat_amount(amount_minor: i64, net_amount_minor: i64) -> i64 {
    amount_minor - net_amount_minor
}

/// Returns true only if the person's VAT payer status is active for the event date
/// AND the per-event `reclaimed_vat` flag is true.
pub fn determine_can_reclaim(
    vat_status: &str,
    vat_from_date: Option<&str>,
    event_date: &str,
    reclaimed_vat: Option<bool>,
) -> bool {
    match vat_status {
        "none" => false,
        "all_year" => reclaimed_vat.unwrap_or(false),
        "from_date" => {
            let from_date = match vat_from_date {
                Some(d) => d,
                None => return false,
            };
            // Compare first 10 chars (YYYY-MM-DD) — ISO format is lexicographically sortable.
            let event_day = &event_date[..10.min(event_date.len())];
            let from_day = &from_date[..10.min(from_date.len())];
            if event_day < from_day {
                return false;
            }
            reclaimed_vat.unwrap_or(false)
        }
        _ => false,
    }
}

pub fn calculate_reclaimable_vat(
    vat_amount_minor: i64,
    vat_reclaimable_pct_bps: i64,
    can_reclaim: bool,
) -> i64 {
    if can_reclaim {
        vat_amount_minor * vat_reclaimable_pct_bps / 10000
    } else {
        0
    }
}

pub fn calculate_flat_tax(tax_basis_minor: i64, flat_rate_bps: i64) -> i64 {
    tax_basis_minor * flat_rate_bps / 10000
}

/// Computes progressive tax by iterating tiers in order. Each tier applies its rate to the
/// income band from the previous tier's threshold (or 0) up to the current tier's threshold,
/// capped at `tax_basis_minor`. The last tier has no upper cap.
pub fn calculate_progressive_tax(tax_basis_minor: i64, tiers: &[TaxTier]) -> i64 {
    if tax_basis_minor <= 0 || tiers.is_empty() {
        return 0;
    }
    let mut total_tax = 0i64;
    let mut prev_threshold = 0i64;
    for (i, tier) in tiers.iter().enumerate() {
        let is_last = i == tiers.len() - 1;
        let upper = if is_last {
            tax_basis_minor
        } else {
            tier.threshold_minor.min(tax_basis_minor)
        };
        if upper > prev_threshold {
            let band = upper - prev_threshold;
            total_tax += band * tier.rate_bps / 10000;
        }
        if !is_last {
            prev_threshold = tier.threshold_minor;
        }
        if upper >= tax_basis_minor {
            break;
        }
    }
    total_tax
}

/// Dispatches to flat or progressive tax calculation based on the first bracket's `rate_type`.
pub fn calculate_tax_from_brackets(tax_basis_minor: i64, brackets: &[TaxModelBracketRow]) -> i64 {
    let Some(bracket) = brackets.first() else {
        return 0;
    };
    match bracket.rate_type.as_str() {
        "flat" => {
            let rate = bracket.flat_rate_bps.unwrap_or(0);
            calculate_flat_tax(tax_basis_minor, rate)
        }
        "progressive" => {
            let tiers_str = match bracket.tiers_json.as_deref() {
                Some(s) if !s.is_empty() => s,
                _ => return 0,
            };
            let tiers: Vec<TaxTier> = match serde_json::from_str(tiers_str) {
                Ok(t) => t,
                Err(_) => return 0,
            };
            calculate_progressive_tax(tax_basis_minor, &tiers)
        }
        _ => 0,
    }
}

pub fn calculate_expense_breakdown(
    event: &EventDataForCalc,
    can_reclaim: bool,
) -> TaxEventBreakdown {
    let vat_rate_bps = event.vat_rate_bps.unwrap_or(0);
    let net_amount_minor = calculate_net_amount(event.amount_minor, vat_rate_bps);
    let vat_amount_minor = calculate_vat_amount(event.amount_minor, net_amount_minor);
    let vat_reclaimable_pct_bps = event.vat_reclaimable_pct_bps.unwrap_or(0);
    let reclaimable_vat_minor =
        calculate_reclaimable_vat(vat_amount_minor, vat_reclaimable_pct_bps, can_reclaim);
    let non_reclaimable_vat_minor = vat_amount_minor - reclaimable_vat_minor;
    let expense_deductible_pct_bps = event.expense_deductible_pct_bps.unwrap_or(10000);
    let tax_deductible_cost_minor = net_amount_minor * expense_deductible_pct_bps / 10000;
    let non_tax_deductible_cost_minor = net_amount_minor - tax_deductible_cost_minor;
    TaxEventBreakdown {
        event_id: event.event_id,
        event_type: event.event_type.clone(),
        event_date: event.event_date.clone(),
        note: event.note.clone(),
        amount_minor: event.amount_minor,
        net_amount_minor,
        vat_amount_minor,
        reclaimable_vat_minor,
        non_reclaimable_vat_minor,
        tax_deductible_cost_minor,
        non_tax_deductible_cost_minor,
    }
}

pub fn calculate_revenue_breakdown(event: &EventDataForCalc) -> TaxEventBreakdown {
    let vat_rate_bps = event.vat_rate_bps.unwrap_or(0);
    let net_amount_minor = calculate_net_amount(event.amount_minor, vat_rate_bps);
    let vat_amount_minor = calculate_vat_amount(event.amount_minor, net_amount_minor);
    TaxEventBreakdown {
        event_id: event.event_id,
        event_type: event.event_type.clone(),
        event_date: event.event_date.clone(),
        note: event.note.clone(),
        amount_minor: event.amount_minor,
        net_amount_minor,
        vat_amount_minor,
        reclaimable_vat_minor: 0,
        non_reclaimable_vat_minor: 0,
        tax_deductible_cost_minor: 0,
        non_tax_deductible_cost_minor: 0,
    }
}

pub fn calculate_tax_model_results(
    model: &TaxModelDetail,
    events: &[EventDataForCalc],
) -> TaxCalculationResult {
    let vat_status = model.vat_status.as_str();
    let vat_from_date = model.vat_from_date.as_deref();

    let mut event_breakdowns: Vec<TaxEventBreakdown> = Vec::new();
    let mut total_income_minor = 0i64;
    let mut total_tax_deductible_expenses_minor = 0i64;
    let mut total_non_tax_deductible_expenses_minor = 0i64;

    for event in events {
        let breakdown = match event.event_type.as_str() {
            "revenue" => calculate_revenue_breakdown(event),
            "expense" => {
                let can_reclaim = determine_can_reclaim(
                    vat_status,
                    vat_from_date,
                    &event.event_date,
                    event.reclaimed_vat,
                );
                calculate_expense_breakdown(event, can_reclaim)
            }
            _ => continue,
        };

        match event.event_type.as_str() {
            "revenue" => total_income_minor += breakdown.net_amount_minor,
            "expense" => {
                total_tax_deductible_expenses_minor += breakdown.tax_deductible_cost_minor;
                total_non_tax_deductible_expenses_minor += breakdown.non_tax_deductible_cost_minor;
            }
            _ => {}
        }

        event_breakdowns.push(breakdown);
    }

    let tax_basis_minor = (total_income_minor - total_tax_deductible_expenses_minor).max(0);
    let tax_amount_minor = calculate_tax_from_brackets(tax_basis_minor, &model.brackets);
    let total_profit_minor = total_income_minor
        - total_tax_deductible_expenses_minor
        - total_non_tax_deductible_expenses_minor
        - tax_amount_minor;

    let (reserve_fund_generation_minor, dividend_minor, withholding_tax_minor, net_dividend_minor) =
        if model.person_type == "legal" && total_profit_minor > 0 {
            let reserve_rate = model.reserve_fund_pct_bps.unwrap_or(0);
            let reserve_target = total_profit_minor * reserve_rate / 10000;
            let reserve_current = model.reserve_fund_current_minor.unwrap_or(0);
            let reserve_max = model.reserve_fund_max_minor.unwrap_or(0);
            let reserve_cap = (reserve_max - reserve_current).max(0);
            let reserve_generation = reserve_target.min(reserve_cap);
            let dividend = total_profit_minor - reserve_generation;
            let withholding_rate = model.dividend_tax_rate_bps.unwrap_or(0);
            let withholding_tax = dividend * withholding_rate / 10000;
            let net_dividend = dividend - withholding_tax;
            (reserve_generation, dividend, withholding_tax, net_dividend)
        } else {
            (0, 0, 0, 0)
        };

    let monthly_tax_burden_minor =
        crate::shared::div_round(tax_amount_minor + withholding_tax_minor, 12);

    TaxCalculationResult {
        total_income_minor,
        total_tax_deductible_expenses_minor,
        total_non_tax_deductible_expenses_minor,
        tax_basis_minor,
        tax_amount_minor,
        total_profit_minor,
        reserve_fund_generation_minor,
        dividend_minor,
        withholding_tax_minor,
        net_dividend_minor,
        monthly_tax_burden_minor,
        person_type: model.person_type.clone(),
        event_breakdowns,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::tax_models::models::{
        EventDataForCalc, TaxModelBracketRow, TaxModelDetail,
    };

    fn make_event(
        event_id: i64,
        event_type: &str,
        amount_minor: i64,
        vat_rate_bps: Option<i64>,
        vat_reclaimable_pct_bps: Option<i64>,
        expense_deductible_pct_bps: Option<i64>,
        reclaimed_vat: Option<bool>,
    ) -> EventDataForCalc {
        EventDataForCalc {
            event_id,
            event_type: event_type.to_owned(),
            amount_minor,
            event_date: "2026-01-15T00:00:00".to_owned(),
            note: None,
            vat_rate_bps,
            vat_reclaimable_pct_bps,
            expense_deductible_pct_bps,
            reclaimed_vat,
        }
    }

    fn flat_bracket(rate_bps: i64) -> TaxModelBracketRow {
        TaxModelBracketRow {
            id: 1,
            sort_order: 0,
            lower_bound_minor: 0,
            rate_type: "flat".to_owned(),
            flat_rate_bps: Some(rate_bps),
            tiers_json: None,
        }
    }

    fn make_flat_model(
        person_type: &str,
        flat_rate_bps: i64,
        reserve_fund_pct_bps: Option<i64>,
        reserve_fund_current_minor: Option<i64>,
        reserve_fund_max_minor: Option<i64>,
        dividend_tax_rate_bps: Option<i64>,
    ) -> TaxModelDetail {
        TaxModelDetail {
            id: 1,
            name: "Test Model".to_owned(),
            calendar_year: 2026,
            person_id: 1,
            person_name: "Test Person".to_owned(),
            person_type: person_type.to_owned(),
            vat_status: "none".to_owned(),
            vat_from_date: None,
            reserve_fund_current_minor,
            reserve_fund_pct_bps,
            reserve_fund_max_minor,
            dividend_tax_rate_bps,
            created_at: "2026-01-01T00:00:00".to_owned(),
            updated_at: "2026-01-01T00:00:00".to_owned(),
            brackets: vec![flat_bracket(flat_rate_bps)],
        }
    }

    // --- calculate_net_amount ---

    #[test]
    fn test_net_amount_basic() {
        // 20% VAT: gross 120 → net 100 (in minor units: 12000 → 10000)
        assert_eq!(calculate_net_amount(12000, 2000), 10000);
    }

    #[test]
    fn test_net_amount_zero_vat() {
        assert_eq!(calculate_net_amount(5000, 0), 5000);
    }

    // --- calculate_expense_breakdown ---

    #[test]
    fn test_expense_breakdown_full_reclaim() {
        // 20% VAT (100% reclaimable), 100% tax-deductible, can_reclaim=true
        let event = make_event(
            1,
            "expense",
            12000,
            Some(2000),
            Some(10000),
            Some(10000),
            Some(true),
        );
        let bd = calculate_expense_breakdown(&event, true);
        assert_eq!(bd.net_amount_minor, 10000);
        assert_eq!(bd.vat_amount_minor, 2000);
        assert_eq!(bd.reclaimable_vat_minor, 2000);
        assert_eq!(bd.non_reclaimable_vat_minor, 0);
        assert_eq!(bd.tax_deductible_cost_minor, 10000);
        assert_eq!(bd.non_tax_deductible_cost_minor, 0);
    }

    #[test]
    fn test_expense_breakdown_partial_reclaim() {
        // 20% VAT (50% reclaimable), 100% tax-deductible, can_reclaim=true
        let event = make_event(
            2,
            "expense",
            12000,
            Some(2000),
            Some(5000),
            Some(10000),
            Some(true),
        );
        let bd = calculate_expense_breakdown(&event, true);
        assert_eq!(bd.net_amount_minor, 10000);
        assert_eq!(bd.vat_amount_minor, 2000);
        assert_eq!(bd.reclaimable_vat_minor, 1000);
        assert_eq!(bd.non_reclaimable_vat_minor, 1000);
    }

    #[test]
    fn test_expense_breakdown_no_reclaim() {
        // can_reclaim=false → reclaimable_vat = 0, all VAT is non-reclaimable
        let event = make_event(
            3,
            "expense",
            12000,
            Some(2000),
            Some(10000),
            Some(10000),
            Some(false),
        );
        let bd = calculate_expense_breakdown(&event, false);
        assert_eq!(bd.reclaimable_vat_minor, 0);
        assert_eq!(bd.non_reclaimable_vat_minor, 2000);
    }

    #[test]
    fn test_expense_breakdown_partial_deductible() {
        // 20% VAT, 75% expense-deductible, can_reclaim=false
        let event = make_event(4, "expense", 12000, Some(2000), Some(0), Some(7500), None);
        let bd = calculate_expense_breakdown(&event, false);
        assert_eq!(bd.net_amount_minor, 10000);
        assert_eq!(bd.tax_deductible_cost_minor, 7500);
        assert_eq!(bd.non_tax_deductible_cost_minor, 2500);
    }

    #[test]
    fn test_expense_breakdown_nil_deductible_pct_defaults_to_full() {
        // expense_deductible_pct_bps = None → treated as 10000 (100%)
        let event = make_event(5, "expense", 12000, Some(2000), Some(0), None, None);
        let bd = calculate_expense_breakdown(&event, false);
        assert_eq!(bd.tax_deductible_cost_minor, 10000);
        assert_eq!(bd.non_tax_deductible_cost_minor, 0);
    }

    // --- calculate_revenue_breakdown ---

    #[test]
    fn test_revenue_breakdown() {
        let event = make_event(6, "revenue", 12000, Some(2000), None, None, None);
        let bd = calculate_revenue_breakdown(&event);
        assert_eq!(bd.net_amount_minor, 10000);
        assert_eq!(bd.vat_amount_minor, 2000);
        assert_eq!(bd.reclaimable_vat_minor, 0);
        assert_eq!(bd.non_reclaimable_vat_minor, 0);
        assert_eq!(bd.tax_deductible_cost_minor, 0);
        assert_eq!(bd.non_tax_deductible_cost_minor, 0);
    }

    // --- determine_can_reclaim ---

    #[test]
    fn test_determine_can_reclaim_none() {
        assert!(!determine_can_reclaim(
            "none",
            None,
            "2026-01-01",
            Some(true)
        ));
        assert!(!determine_can_reclaim(
            "none",
            None,
            "2026-01-01",
            Some(false)
        ));
        assert!(!determine_can_reclaim("none", None, "2026-01-01", None));
    }

    #[test]
    fn test_determine_can_reclaim_all_year() {
        assert!(determine_can_reclaim(
            "all_year",
            None,
            "2026-01-01",
            Some(true)
        ));
        assert!(!determine_can_reclaim(
            "all_year",
            None,
            "2026-01-01",
            Some(false)
        ));
        assert!(!determine_can_reclaim("all_year", None, "2026-01-01", None));
    }

    #[test]
    fn test_determine_can_reclaim_from_date() {
        let from = Some("2026-06-01");
        // Before the from_date: always false
        assert!(!determine_can_reclaim(
            "from_date",
            from,
            "2026-05-31",
            Some(true)
        ));
        // On the from_date: depends on reclaimed_vat
        assert!(determine_can_reclaim(
            "from_date",
            from,
            "2026-06-01",
            Some(true)
        ));
        assert!(!determine_can_reclaim(
            "from_date",
            from,
            "2026-06-01",
            Some(false)
        ));
        // After the from_date: depends on reclaimed_vat
        assert!(determine_can_reclaim(
            "from_date",
            from,
            "2026-07-15",
            Some(true)
        ));
        assert!(!determine_can_reclaim(
            "from_date",
            from,
            "2026-07-15",
            Some(false)
        ));
    }

    #[test]
    fn test_determine_can_reclaim_null_reclaimed_vat() {
        // None reclaimed_vat → false regardless of vat_status
        assert!(!determine_can_reclaim("all_year", None, "2026-01-01", None));
        assert!(!determine_can_reclaim(
            "from_date",
            Some("2026-01-01"),
            "2026-06-01",
            None
        ));
    }

    // --- calculate_flat_tax ---

    #[test]
    fn test_flat_tax() {
        assert_eq!(calculate_flat_tax(100000, 1900), 19000);
        assert_eq!(calculate_flat_tax(0, 1900), 0);
        assert_eq!(calculate_flat_tax(100000, 0), 0);
    }

    // --- calculate_progressive_tax ---

    #[test]
    fn test_progressive_tax_single_tier() {
        // Single tier (last tier, so no upper cap): 19% on entire basis
        let tiers = vec![TaxTier {
            threshold_minor: 1_000_000_00,
            rate_bps: 1900,
        }];
        assert_eq!(calculate_progressive_tax(50_000_00, &tiers), 9_500_00);
    }

    #[test]
    fn test_progressive_tax_multi_tier() {
        // Tier 1: 12% on 0–10000_00
        // Tier 2: 19% on 10000_00–30000_00
        // Tier 3 (last): 25% on 30000_00+
        // Basis = 50000_00
        // Band 1: 10000_00 * 1200 / 10000 = 1200_00
        // Band 2: 20000_00 * 1900 / 10000 = 3800_00
        // Band 3: 20000_00 * 2500 / 10000 = 5000_00
        // Total:  10000_00
        let tiers = vec![
            TaxTier {
                threshold_minor: 10000_00,
                rate_bps: 1200,
            },
            TaxTier {
                threshold_minor: 30000_00,
                rate_bps: 1900,
            },
            TaxTier {
                threshold_minor: i64::MAX,
                rate_bps: 2500,
            },
        ];
        assert_eq!(calculate_progressive_tax(50000_00, &tiers), 10000_00);
    }

    #[test]
    fn test_progressive_tax_zero_basis() {
        let tiers = vec![TaxTier {
            threshold_minor: 1_000_000_00,
            rate_bps: 1900,
        }];
        assert_eq!(calculate_progressive_tax(0, &tiers), 0);
    }

    // --- calculate_tax_model_results ---

    #[test]
    fn test_aggregate_no_events() {
        let model = make_flat_model("physical", 1900, None, None, None, None);
        let result = calculate_tax_model_results(&model, &[]);
        assert_eq!(result.total_income_minor, 0);
        assert_eq!(result.total_tax_deductible_expenses_minor, 0);
        assert_eq!(result.total_non_tax_deductible_expenses_minor, 0);
        assert_eq!(result.tax_basis_minor, 0);
        assert_eq!(result.tax_amount_minor, 0);
        assert_eq!(result.total_profit_minor, 0);
        assert_eq!(result.reserve_fund_generation_minor, 0);
        assert_eq!(result.dividend_minor, 0);
        assert_eq!(result.withholding_tax_minor, 0);
        assert_eq!(result.event_breakdowns.len(), 0);
    }

    #[test]
    fn test_aggregate_physical_person() {
        // Revenue: 100000 (no VAT) → net 100000
        // Expense: 24000 gross, 20% VAT, 100% reclaimable, 100% deductible, can_reclaim=true
        //   net = 20000, vat = 4000, tax_deductible = 20000
        // model: flat 20% tax, physical person, vat_status=none → reclaim is false (status=none)
        let model = make_flat_model("physical", 2000, None, None, None, None);
        let revenue = EventDataForCalc {
            event_date: "2026-01-01T00:00:00".to_owned(),
            ..make_event(1, "revenue", 100_000, None, None, None, None)
        };
        let expense = EventDataForCalc {
            event_date: "2026-02-01T00:00:00".to_owned(),
            ..make_event(
                2,
                "expense",
                24_000,
                Some(2000),
                Some(10000),
                Some(10000),
                Some(true),
            )
        };
        let events = vec![revenue, expense];
        let result = calculate_tax_model_results(&model, &events);

        // vat_status="none" → can_reclaim=false → all VAT is cost
        // net expense = 20000, tax_deductible = 20000 (100%)
        assert_eq!(result.total_income_minor, 100_000);
        assert_eq!(result.total_tax_deductible_expenses_minor, 20_000);
        assert_eq!(result.total_non_tax_deductible_expenses_minor, 0);
        assert_eq!(result.tax_basis_minor, 80_000);
        // 20% flat tax on 80000 = 16000
        assert_eq!(result.tax_amount_minor, 16_000);
        // profit = 100000 - 20000 - 0 - 16000 = 64000
        assert_eq!(result.total_profit_minor, 64_000);
        // physical person: no reserve/dividend/withholding
        assert_eq!(result.reserve_fund_generation_minor, 0);
        assert_eq!(result.dividend_minor, 0);
        assert_eq!(result.withholding_tax_minor, 0);
        assert_eq!(result.net_dividend_minor, 0);
        // monthly burden = 16000 / 12 = 1333
        assert_eq!(result.monthly_tax_burden_minor, 1_333);
        assert_eq!(result.person_type, "physical");
        assert_eq!(result.event_breakdowns.len(), 2);
    }

    #[test]
    fn test_aggregate_legal_person() {
        // Same events as physical test, flat 20% tax
        // Legal person: reserve_fund 10%, max 50000, current 0 → reserve = 10% of 64000 = 6400
        // dividend = 64000 - 6400 = 57600
        // withholding 7% = 57600 * 700 / 10000 = 4032
        // net_dividend = 57600 - 4032 = 53568
        let model = make_flat_model("legal", 2000, Some(1000), Some(0), Some(50_000), Some(700));
        let revenue = make_event(1, "revenue", 100_000, None, None, None, None);
        let expense = make_event(
            2,
            "expense",
            24_000,
            Some(2000),
            Some(10000),
            Some(10000),
            Some(true),
        );
        let result = calculate_tax_model_results(&model, &[revenue, expense]);

        assert_eq!(result.total_profit_minor, 64_000);
        assert_eq!(result.reserve_fund_generation_minor, 6_400);
        assert_eq!(result.dividend_minor, 57_600);
        assert_eq!(result.withholding_tax_minor, 4_032);
        assert_eq!(result.net_dividend_minor, 53_568);
        assert_eq!(result.monthly_tax_burden_minor, (16_000 + 4_032) / 12);
    }

    #[test]
    fn test_aggregate_reserve_fund_cap() {
        // Reserve target = 10% of 64000 = 6400, but cap = 50000 - 45000 = 5000
        let model = make_flat_model(
            "legal",
            2000,
            Some(1000),
            Some(45_000),
            Some(50_000),
            Some(0),
        );
        let revenue = make_event(1, "revenue", 100_000, None, None, None, None);
        let expense = make_event(
            2,
            "expense",
            24_000,
            Some(2000),
            Some(10000),
            Some(10000),
            Some(true),
        );
        let result = calculate_tax_model_results(&model, &[revenue, expense]);

        assert_eq!(result.reserve_fund_generation_minor, 5_000);
        assert_eq!(result.dividend_minor, 64_000 - 5_000);
    }

    #[test]
    fn test_aggregate_negative_profit() {
        // Expenses exceed income: profit < 0, reserve_fund = 0, dividend = 0
        let model = make_flat_model("legal", 2000, Some(1000), Some(0), Some(50_000), Some(700));
        let revenue = make_event(1, "revenue", 10_000, None, None, None, None);
        // expense net = 20000 (deductible), tax_basis = max(0, 10000-20000) = 0
        let expense = make_event(2, "expense", 24_000, Some(2000), None, None, None);
        let result = calculate_tax_model_results(&model, &[revenue, expense]);

        assert_eq!(result.tax_basis_minor, 0);
        assert_eq!(result.tax_amount_minor, 0);
        // profit = 10000 - 20000 - 0 - 0 = -10000
        assert_eq!(result.total_profit_minor, -10_000);
        assert_eq!(result.reserve_fund_generation_minor, 0);
        assert_eq!(result.dividend_minor, 0);
        assert_eq!(result.withholding_tax_minor, 0);
    }
}
