use super::forecast::{self, LineTotals};
use super::models::{
    EarlierProfit, EarlierProfitResult, EntryKind, EventDataForCalc, MinimumTaxBand,
    TaxCalculationResult, TaxEventBreakdown, TaxModelBracketRow, TaxModelDetail, TaxTier,
};
use crate::shared::div_round;

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
            let event_day = event_date.get(..10).unwrap_or(event_date);
            let from_day = from_date.get(..10).unwrap_or(from_date);
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

fn pick_band<T>(items: &[T], lower_bound: impl Fn(&T) -> i64, value: i64) -> Option<&T> {
    items
        .iter()
        .rev()
        .find(|item| lower_bound(item) < value)
        .or_else(|| items.first().filter(|first| value >= lower_bound(first)))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BracketTax {
    pub tax_minor: i64,
    pub flat_rate_bps: Option<i64>,
}

pub fn calculate_tax_from_brackets(
    brackets: &[TaxModelBracketRow],
    pick_value: i64,
    tax_basis_minor: i64,
) -> BracketTax {
    let Some(bracket) = pick_band(brackets, |b| b.lower_bound_minor, pick_value) else {
        return BracketTax::default();
    };
    match bracket.rate_type.as_str() {
        "flat" => {
            let rate = bracket.flat_rate_bps.unwrap_or(0);
            BracketTax {
                tax_minor: calculate_flat_tax(tax_basis_minor, rate),
                flat_rate_bps: Some(rate),
            }
        }
        "progressive" => {
            let tiers: Vec<TaxTier> = bracket
                .tiers_json
                .as_deref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
            BracketTax {
                tax_minor: calculate_progressive_tax(tax_basis_minor, &tiers),
                flat_rate_bps: None,
            }
        }
        _ => BracketTax::default(),
    }
}

fn minimum_tax(
    bands: &[MinimumTaxBand],
    revenue_minor: i64,
    computed_tax_minor: i64,
) -> Option<i64> {
    pick_band(bands, |b| b.lower_bound_minor, revenue_minor)
        .map(|band| band.amount_minor)
        .filter(|&minimum| computed_tax_minor < minimum)
}

fn allocate_loss(earlier: &[EarlierProfit], loss_minor: i64) -> Vec<EarlierProfitResult> {
    let mut oldest_first: Vec<&EarlierProfit> = earlier.iter().collect();
    oldest_first.sort_by_key(|row| row.profit_year);
    let mut loss_left = loss_minor;
    oldest_first
        .into_iter()
        .map(|row| {
            let used = loss_left.min(row.amount_minor);
            loss_left -= used;
            let payable_minor = row.amount_minor - used;
            EarlierProfitResult {
                profit_year: row.profit_year,
                amount_minor: row.amount_minor,
                payable_minor,
                dividend_tax_rate_bps: row.dividend_tax_rate_bps,
                dividend_tax_minor: payable_minor * row.dividend_tax_rate_bps / 10000,
            }
        })
        .collect()
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
    // VAT that cannot be reclaimed is part of what the expense costs.
    let cost_minor = net_amount_minor + non_reclaimable_vat_minor;
    let tax_deductible_cost_minor = cost_minor * expense_deductible_pct_bps / 10000;
    let non_tax_deductible_cost_minor = cost_minor - tax_deductible_cost_minor;
    TaxEventBreakdown {
        event_id: event.event_id,
        kind: event.kind,
        event_date: event.event_date.clone(),
        note: event.note.clone(),
        amount_minor: event.amount_minor,
        net_amount_minor,
        vat_amount_minor,
        reclaimable_vat_minor,
        non_reclaimable_vat_minor,
        tax_deductible_cost_minor,
        non_tax_deductible_cost_minor,
        not_counted: None,
    }
}

pub fn calculate_revenue_breakdown(event: &EventDataForCalc) -> TaxEventBreakdown {
    let vat_rate_bps = event.vat_rate_bps.unwrap_or(0);
    let net_amount_minor = calculate_net_amount(event.amount_minor, vat_rate_bps);
    let vat_amount_minor = calculate_vat_amount(event.amount_minor, net_amount_minor);
    TaxEventBreakdown {
        event_id: event.event_id,
        kind: event.kind,
        event_date: event.event_date.clone(),
        note: event.note.clone(),
        amount_minor: event.amount_minor,
        net_amount_minor,
        vat_amount_minor,
        reclaimable_vat_minor: 0,
        non_reclaimable_vat_minor: 0,
        tax_deductible_cost_minor: 0,
        non_tax_deductible_cost_minor: 0,
        not_counted: None,
    }
}

fn calculate_breakdown(model: &TaxModelDetail, event: &EventDataForCalc) -> TaxEventBreakdown {
    match event.kind {
        EntryKind::Revenue => calculate_revenue_breakdown(event),
        EntryKind::Depreciation | EntryKind::Prepaid | EntryKind::Expense => {
            let can_reclaim = determine_can_reclaim(
                &model.vat_status,
                model.vat_from_date.as_deref(),
                &event.event_date,
                event.reclaimed_vat,
            );
            calculate_expense_breakdown(event, can_reclaim)
        }
    }
}

#[derive(Default)]
struct Dividends {
    reserve_minor: i64,
    from_year_minor: i64,
    from_year_tax_minor: i64,
    earlier: Vec<EarlierProfitResult>,
}

fn company_dividends(model: &TaxModelDetail, profit_minor: i64) -> Dividends {
    let (reserve_minor, from_year_minor) = if profit_minor > 0 {
        let target = profit_minor * model.reserve_fund_pct_bps.unwrap_or(0) / 10000;
        let room = (model.reserve_fund_max_minor.unwrap_or(0)
            - model.reserve_fund_current_minor.unwrap_or(0))
        .max(0);
        let reserve = target.min(room);
        (reserve, profit_minor - reserve)
    } else {
        (0, 0)
    };
    Dividends {
        reserve_minor,
        from_year_minor,
        from_year_tax_minor: from_year_minor * model.dividend_tax_rate_bps.unwrap_or(0) / 10000,
        earlier: allocate_loss(&model.figures.earlier_profits, (-profit_minor).max(0)),
    }
}

pub fn calculate_tax_model_results(
    model: &TaxModelDetail,
    events: &[EventDataForCalc],
) -> TaxCalculationResult {
    let entries = events
        .iter()
        .map(|event| (event.month, calculate_breakdown(model, event)))
        .collect();
    let forecast = forecast::build(&model.figures, entries);
    let LineTotals {
        revenue_minor,
        deductible_minor,
        non_deductible_minor,
    } = forecast.totals;

    let tax_basis_minor = (revenue_minor - deductible_minor).max(0);
    let revenue_pick_minor = revenue_minor.max(0);
    let bracket_pick_minor = if model.is_company() {
        revenue_pick_minor
    } else {
        tax_basis_minor
    };
    let BracketTax {
        tax_minor: computed_tax_minor,
        flat_rate_bps: applied_flat_rate_bps,
    } = calculate_tax_from_brackets(&model.brackets, bracket_pick_minor, tax_basis_minor);
    let minimum_tax_minor = if model.is_company() {
        minimum_tax(
            &model.minimum_tax_bands,
            revenue_pick_minor,
            computed_tax_minor,
        )
    } else {
        None
    };
    let tax_amount_minor = minimum_tax_minor.unwrap_or(computed_tax_minor);
    let total_profit_minor =
        revenue_minor - deductible_minor - non_deductible_minor - tax_amount_minor;

    let dividends = if model.is_company() {
        company_dividends(model, total_profit_minor)
    } else {
        Dividends::default()
    };
    let dividend_minor = dividends.from_year_minor
        + dividends
            .earlier
            .iter()
            .map(|row| row.payable_minor)
            .sum::<i64>();
    let withholding_tax_minor = dividends.from_year_tax_minor
        + dividends
            .earlier
            .iter()
            .map(|row| row.dividend_tax_minor)
            .sum::<i64>();

    TaxCalculationResult {
        total_income_minor: revenue_minor,
        total_tax_deductible_expenses_minor: deductible_minor,
        total_non_tax_deductible_expenses_minor: non_deductible_minor,
        tax_basis_minor,
        tax_amount_minor,
        total_profit_minor,
        reserve_fund_generation_minor: dividends.reserve_minor,
        dividend_minor,
        withholding_tax_minor,
        net_dividend_minor: dividend_minor - withholding_tax_minor,
        monthly_tax_burden_minor: div_round(tax_amount_minor + dividends.from_year_tax_minor, 12),
        person_type: model.person_type.clone(),
        event_breakdowns: forecast.entries,
        actuals_through_month: model.figures.actuals_through_month.get(),
        lines: forecast.lines,
        applied_flat_rate_bps,
        computed_tax_minor,
        minimum_tax_minor,
        dividend_from_year_minor: dividends.from_year_minor,
        dividend_from_year_tax_minor: dividends.from_year_tax_minor,
        earlier_profits: dividends.earlier,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::features::tax_models::models::{
        ActualMonths, EntryNotCountedReason, ForecastCell, ForecastFigures, ForecastLine,
        ForecastLineResult, TypedPair, ValueSource,
    };

    fn make_event(
        event_id: i64,
        kind: EntryKind,
        amount_minor: i64,
        vat_rate_bps: Option<i64>,
        vat_reclaimable_pct_bps: Option<i64>,
        expense_deductible_pct_bps: Option<i64>,
        reclaimed_vat: Option<bool>,
    ) -> EventDataForCalc {
        EventDataForCalc {
            event_id,
            kind,
            month: 1,
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
            minimum_tax_bands: vec![],
            figures: ForecastFigures::default(),
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
            EntryKind::Expense,
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
            EntryKind::Expense,
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
        // The non-reclaimable VAT is part of the cost: 10000 + 1000
        assert_eq!(bd.tax_deductible_cost_minor, 11000);
        assert_eq!(bd.non_tax_deductible_cost_minor, 0);
    }

    #[test]
    fn test_expense_breakdown_no_reclaim() {
        // can_reclaim=false → reclaimable_vat = 0, all VAT is non-reclaimable
        let event = make_event(
            3,
            EntryKind::Expense,
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
        // cost base = net 10000 + non-reclaimable VAT 2000 = 12000, split 75/25
        let event = make_event(
            4,
            EntryKind::Expense,
            12000,
            Some(2000),
            Some(0),
            Some(7500),
            None,
        );
        let bd = calculate_expense_breakdown(&event, false);
        assert_eq!(bd.net_amount_minor, 10000);
        assert_eq!(bd.tax_deductible_cost_minor, 9000);
        assert_eq!(bd.non_tax_deductible_cost_minor, 3000);
    }

    #[test]
    fn test_expense_breakdown_nil_deductible_pct_defaults_to_full() {
        // expense_deductible_pct_bps = None → treated as 10000 (100%)
        let event = make_event(
            5,
            EntryKind::Expense,
            12000,
            Some(2000),
            Some(0),
            None,
            None,
        );
        let bd = calculate_expense_breakdown(&event, false);
        assert_eq!(bd.tax_deductible_cost_minor, 12000);
        assert_eq!(bd.non_tax_deductible_cost_minor, 0);
    }

    #[test]
    fn test_expense_breakdown_partial_reclaim_partial_deductible() {
        // net 10000, VAT 2000 of which 50% reclaimable → 1000 non-reclaimable
        // cost base 11000 split 60/40 → 6600 / 4400
        let event = make_event(
            7,
            EntryKind::Expense,
            12000,
            Some(2000),
            Some(5000),
            Some(6000),
            Some(true),
        );
        let bd = calculate_expense_breakdown(&event, true);
        assert_eq!(bd.tax_deductible_cost_minor, 6600);
        assert_eq!(bd.non_tax_deductible_cost_minor, 4400);
    }

    // --- calculate_revenue_breakdown ---

    #[test]
    fn test_revenue_breakdown() {
        let event = make_event(6, EntryKind::Revenue, 12000, Some(2000), None, None, None);
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
    fn test_determine_can_reclaim_does_not_panic_on_multibyte_dates() {
        // Byte 10 falls inside 'é'; the comparison must not slice through it.
        assert!(determine_can_reclaim(
            "from_date",
            Some("2026-01-01"),
            "2026-06-1é",
            Some(true)
        ));
        assert!(!determine_can_reclaim(
            "from_date",
            Some("2026-06-1é"),
            "2026-01-01",
            Some(true)
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

    // --- calculate_tax_from_brackets ---

    fn bracket(
        sort_order: i64,
        lower_bound_minor: i64,
        rate_type: &str,
        flat_rate_bps: Option<i64>,
        tiers_json: Option<&str>,
    ) -> TaxModelBracketRow {
        TaxModelBracketRow {
            id: sort_order + 1,
            sort_order,
            lower_bound_minor,
            rate_type: rate_type.to_owned(),
            flat_rate_bps,
            tiers_json: tiers_json.map(str::to_owned),
        }
    }

    fn three_brackets() -> Vec<TaxModelBracketRow> {
        vec![
            bracket(0, 0, "flat", Some(1000), None),
            bracket(1, 100_000, "flat", Some(2000), None),
            bracket(
                2,
                200_000,
                "progressive",
                None,
                Some(
                    r#"[{"thresholdMinor":250000,"rateBps":3000},{"thresholdMinor":0,"rateBps":4000}]"#,
                ),
            ),
        ]
    }

    fn tax_by_basis(basis: i64, brackets: &[TaxModelBracketRow]) -> (i64, Option<i64>) {
        let tax = calculate_tax_from_brackets(brackets, basis, basis);
        (tax.tax_minor, tax.flat_rate_bps)
    }

    #[test]
    fn test_brackets_zero_basis_uses_first_bracket() {
        assert_eq!(tax_by_basis(0, &three_brackets()), (0, Some(1000)));
    }

    #[test]
    fn test_brackets_basis_below_bound_uses_previous_bracket() {
        assert_eq!(tax_by_basis(99_999, &three_brackets()), (9_999, Some(1000)));
    }

    #[test]
    fn test_brackets_basis_at_bound_stays_in_the_bracket_below() {
        assert_eq!(
            tax_by_basis(100_000, &three_brackets()),
            (10_000, Some(1000))
        );
    }

    #[test]
    fn test_brackets_basis_just_above_bound_uses_the_next_bracket() {
        assert_eq!(
            tax_by_basis(100_001, &three_brackets()),
            (20_000, Some(2000))
        );
    }

    #[test]
    fn test_brackets_basis_above_bound_uses_that_bracket_for_whole_basis() {
        assert_eq!(
            tax_by_basis(150_000, &three_brackets()),
            (30_000, Some(2000))
        );
    }

    #[test]
    fn test_brackets_progressive_bracket_tiers_apply_to_whole_basis() {
        // 30% on 0..250000 = 75000, 40% on 250000..300000 = 20000
        assert_eq!(tax_by_basis(300_000, &three_brackets()), (95_000, None));
    }

    #[test]
    fn test_brackets_basis_below_first_bound_is_untaxed() {
        let brackets = vec![bracket(0, 50_000, "flat", Some(1000), None)];
        assert_eq!(tax_by_basis(40_000, &brackets), (0, None));
    }

    #[test]
    fn test_brackets_pick_value_and_basis_are_separate() {
        assert_eq!(
            calculate_tax_from_brackets(&three_brackets(), 150_000, 10_000),
            BracketTax {
                tax_minor: 2_000,
                flat_rate_bps: Some(2000)
            }
        );
    }

    // --- pick_band ---

    #[test]
    fn pick_band_keeps_each_upper_bound_inside_its_band() {
        let bounds = [0, 100];
        let pick = |value| pick_band(&bounds, |b| *b, value).copied();
        assert_eq!(pick(0), Some(0));
        assert_eq!(pick(50), Some(0));
        assert_eq!(pick(100), Some(0));
        assert_eq!(pick(101), Some(100));
        assert_eq!(pick(1_000_000), Some(100));
    }

    #[test]
    fn pick_band_below_a_first_bound_above_zero_picks_nothing() {
        let bounds = [50];
        let pick = |value| pick_band(&bounds, |b| *b, value).copied();
        assert_eq!(pick(0), None);
        assert_eq!(pick(49), None);
        assert_eq!(pick(50), Some(50));
        assert_eq!(pick(51), Some(50));
    }

    #[test]
    fn pick_band_of_no_bands_is_none() {
        assert_eq!(pick_band(&[] as &[i64], |b| *b, 10), None);
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
        //   net = 20000, vat = 4000
        // model: flat 20% tax, physical person, vat_status=none → reclaim is false (status=none)
        let model = make_flat_model("physical", 2000, None, None, None, None);
        let revenue = EventDataForCalc {
            event_date: "2026-01-01T00:00:00".to_owned(),
            ..make_event(1, EntryKind::Revenue, 100_000, None, None, None, None)
        };
        let expense = EventDataForCalc {
            event_date: "2026-02-01T00:00:00".to_owned(),
            month: 2,
            ..make_event(
                2,
                EntryKind::Expense,
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
        // cost = net 20000 + VAT 4000 = 24000, tax_deductible = 24000 (100%)
        assert_eq!(result.total_income_minor, 100_000);
        assert_eq!(result.total_tax_deductible_expenses_minor, 24_000);
        assert_eq!(result.total_non_tax_deductible_expenses_minor, 0);
        assert_eq!(result.tax_basis_minor, 76_000);
        // 20% flat tax on 76000 = 15200
        assert_eq!(result.tax_amount_minor, 15_200);
        // profit = 100000 - 24000 - 0 - 15200 = 60800
        assert_eq!(result.total_profit_minor, 60_800);
        // physical person: no reserve/dividend/withholding
        assert_eq!(result.reserve_fund_generation_minor, 0);
        assert_eq!(result.dividend_minor, 0);
        assert_eq!(result.withholding_tax_minor, 0);
        assert_eq!(result.net_dividend_minor, 0);
        // monthly burden = 15200 / 12 = 1266.67 → 1267
        assert_eq!(result.monthly_tax_burden_minor, 1_267);
        assert_eq!(result.person_type, "physical");
        assert_eq!(result.event_breakdowns.len(), 2);
    }

    #[test]
    fn test_aggregate_legal_person() {
        // Same events as physical test, flat 20% tax
        // Legal person: reserve_fund 10%, max 50000, current 0 → reserve = 10% of 60800 = 6080
        // dividend = 60800 - 6080 = 54720
        // withholding 7% = 54720 * 700 / 10000 = 3830
        // net_dividend = 54720 - 3830 = 50890
        let model = make_flat_model("legal", 2000, Some(1000), Some(0), Some(50_000), Some(700));
        let revenue = make_event(1, EntryKind::Revenue, 100_000, None, None, None, None);
        let expense = make_event(
            2,
            EntryKind::Expense,
            24_000,
            Some(2000),
            Some(10000),
            Some(10000),
            Some(true),
        );
        let result = calculate_tax_model_results(&model, &[revenue, expense]);

        assert_eq!(result.total_profit_minor, 60_800);
        assert_eq!(result.reserve_fund_generation_minor, 6_080);
        assert_eq!(result.dividend_minor, 54_720);
        assert_eq!(result.withholding_tax_minor, 3_830);
        assert_eq!(result.net_dividend_minor, 50_890);
        // (15200 + 3830) / 12 = 1585.83 → 1586
        assert_eq!(result.monthly_tax_burden_minor, 1_586);
    }

    #[test]
    fn test_aggregate_reserve_fund_cap() {
        // Reserve target = 10% of 60800 = 6080, but cap = 50000 - 45000 = 5000
        let model = make_flat_model(
            "legal",
            2000,
            Some(1000),
            Some(45_000),
            Some(50_000),
            Some(0),
        );
        let revenue = make_event(1, EntryKind::Revenue, 100_000, None, None, None, None);
        let expense = make_event(
            2,
            EntryKind::Expense,
            24_000,
            Some(2000),
            Some(10000),
            Some(10000),
            Some(true),
        );
        let result = calculate_tax_model_results(&model, &[revenue, expense]);

        assert_eq!(result.reserve_fund_generation_minor, 5_000);
        assert_eq!(result.dividend_minor, 60_800 - 5_000);
    }

    #[test]
    fn test_aggregate_negative_profit() {
        // Expenses exceed income: profit < 0, reserve_fund = 0, dividend = 0
        let model = make_flat_model("legal", 2000, Some(1000), Some(0), Some(50_000), Some(700));
        let revenue = make_event(1, EntryKind::Revenue, 10_000, None, None, None, None);
        // expense cost = net 20000 + non-reclaimable VAT 4000 = 24000 (deductible),
        // tax_basis = max(0, 10000-24000) = 0
        let expense = make_event(2, EntryKind::Expense, 24_000, Some(2000), None, None, None);
        let result = calculate_tax_model_results(&model, &[revenue, expense]);

        assert_eq!(result.tax_basis_minor, 0);
        assert_eq!(result.tax_amount_minor, 0);
        // profit = 10000 - 24000 - 0 - 0 = -14000
        assert_eq!(result.total_profit_minor, -14_000);
        assert_eq!(result.reserve_fund_generation_minor, 0);
        assert_eq!(result.dividend_minor, 0);
        assert_eq!(result.withholding_tax_minor, 0);
    }

    // --- the forecast and the tax rules ---

    fn figures(
        month: i64,
        payroll_monthly_minor: i64,
        typed: &[(ForecastLine, Option<i64>, Option<i64>)],
        earlier_profits: Vec<EarlierProfit>,
    ) -> ForecastFigures {
        ForecastFigures {
            actuals_through_month: ActualMonths::new(month).unwrap(),
            payroll_monthly_minor,
            typed_values: typed
                .iter()
                .map(|&(line, so_far_minor, rest_minor)| {
                    (
                        line,
                        TypedPair {
                            so_far_minor,
                            rest_minor,
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>(),
            earlier_profits,
        }
    }

    fn whole_year(revenue_minor: i64, deductible_minor: i64) -> ForecastFigures {
        figures(
            12,
            0,
            &[
                (ForecastLine::Revenue, Some(revenue_minor), None),
                (ForecastLine::DeductibleCosts, Some(deductible_minor), None),
            ],
            vec![],
        )
    }

    fn band(lower_bound_minor: i64, amount_minor: i64) -> MinimumTaxBand {
        MinimumTaxBand {
            lower_bound_minor,
            amount_minor,
        }
    }

    fn earlier(profit_year: i64, amount_minor: i64, dividend_tax_rate_bps: i64) -> EarlierProfit {
        EarlierProfit {
            profit_year,
            amount_minor,
            dividend_tax_rate_bps,
        }
    }

    fn rates_10_and_21() -> Vec<TaxModelBracketRow> {
        vec![
            bracket(0, 0, "flat", Some(1000), None),
            bracket(1, 100_000_00, "flat", Some(2100), None),
        ]
    }

    fn model(person_type: &str, figures: ForecastFigures) -> TaxModelDetail {
        TaxModelDetail {
            brackets: rates_10_and_21(),
            minimum_tax_bands: vec![
                band(0, 340_00),
                band(50_000_00, 960_00),
                band(250_000_00, 1_920_00),
            ],
            figures,
            ..make_flat_model(
                person_type,
                0,
                Some(1000),
                Some(500_00),
                Some(500_00),
                Some(700),
            )
        }
    }

    fn worked_example(typed_deductible_rest: Option<i64>) -> TaxModelDetail {
        model(
            "legal",
            figures(
                9,
                4_000_00,
                &[
                    (ForecastLine::Revenue, Some(90_000_00), None),
                    (ForecastLine::Payroll, Some(36_000_00), None),
                    (
                        ForecastLine::DeductibleCosts,
                        Some(18_000_00),
                        typed_deductible_rest,
                    ),
                    (ForecastLine::NonDeductibleCosts, Some(1_500_00), None),
                ],
                vec![earlier(2025, 15_000_00, 700)],
            ),
        )
    }

    fn depreciation_350_a_month() -> Vec<EventDataForCalc> {
        (1..=12u8)
            .map(|month| EventDataForCalc {
                event_date: format!("2026-{month:02}-28T23:59:59"),
                month,
                ..make_event(
                    i64::from(month),
                    EntryKind::Depreciation,
                    350_00,
                    None,
                    None,
                    Some(10000),
                    None,
                )
            })
            .collect()
    }

    fn cell(amount_minor: i64, source: ValueSource, untyped_minor: i64) -> ForecastCell {
        ForecastCell {
            amount_minor,
            source,
            untyped_minor,
        }
    }

    #[test]
    fn worked_example_fills_every_cell_with_its_source() {
        let result =
            calculate_tax_model_results(&worked_example(None), &depreciation_350_a_month());

        let row = |line, so_far, rest, year_minor| ForecastLineResult {
            line,
            so_far,
            rest,
            year_minor,
        };
        assert_eq!(result.actuals_through_month, 9);
        assert_eq!(
            result.lines,
            vec![
                row(
                    ForecastLine::Revenue,
                    cell(90_000_00, ValueSource::Typed, 0),
                    cell(30_000_00, ValueSource::Average, 30_000_00),
                    120_000_00,
                ),
                row(
                    ForecastLine::Payroll,
                    cell(36_000_00, ValueSource::Typed, 0),
                    cell(12_000_00, ValueSource::MonthlyAmount, 12_000_00),
                    48_000_00,
                ),
                row(
                    ForecastLine::Depreciation,
                    cell(3_150_00, ValueSource::Schedule, 3_150_00),
                    cell(1_050_00, ValueSource::Schedule, 1_050_00),
                    4_200_00,
                ),
                row(
                    ForecastLine::DeductibleCosts,
                    cell(18_000_00, ValueSource::Typed, 0),
                    cell(6_000_00, ValueSource::Average, 6_000_00),
                    24_000_00,
                ),
                row(
                    ForecastLine::NonDeductibleCosts,
                    cell(1_500_00, ValueSource::Typed, 0),
                    cell(500_00, ValueSource::Average, 500_00),
                    2_000_00,
                ),
            ]
        );
        assert!(result
            .event_breakdowns
            .iter()
            .all(|e| e.not_counted.is_none()));
    }

    #[test]
    fn worked_example_tax_profit_and_dividends() {
        let result =
            calculate_tax_model_results(&worked_example(None), &depreciation_350_a_month());

        assert_eq!(result.total_income_minor, 120_000_00);
        assert_eq!(result.total_tax_deductible_expenses_minor, 76_200_00);
        assert_eq!(result.total_non_tax_deductible_expenses_minor, 2_000_00);
        assert_eq!(result.tax_basis_minor, 43_800_00);
        assert_eq!(result.applied_flat_rate_bps, Some(2100));
        assert_eq!(result.computed_tax_minor, 9_198_00);
        assert_eq!(result.minimum_tax_minor, None);
        assert_eq!(result.tax_amount_minor, 9_198_00);
        assert_eq!(result.total_profit_minor, 32_602_00);
        assert_eq!(result.reserve_fund_generation_minor, 0);
        assert_eq!(result.dividend_from_year_minor, 32_602_00);
        assert_eq!(result.dividend_from_year_tax_minor, 2_282_14);
        assert_eq!(
            result.earlier_profits,
            vec![EarlierProfitResult {
                profit_year: 2025,
                amount_minor: 15_000_00,
                payable_minor: 15_000_00,
                dividend_tax_rate_bps: 700,
                dividend_tax_minor: 1_050_00,
            }]
        );
        assert_eq!(result.dividend_minor, 47_602_00);
        assert_eq!(result.withholding_tax_minor, 3_332_14);
        assert_eq!(result.net_dividend_minor, 44_269_86);
        assert_eq!(
            result.monthly_tax_burden_minor, 956_68,
            "the burden leaves out the earlier profit's dividend tax"
        );
    }

    #[test]
    fn worked_example_with_typed_deductible_rest() {
        let result = calculate_tax_model_results(
            &worked_example(Some(5_600_00)),
            &depreciation_350_a_month(),
        );
        let deductible = result
            .lines
            .iter()
            .find(|l| l.line == ForecastLine::DeductibleCosts)
            .unwrap();
        assert_eq!(
            deductible.rest,
            cell(5_600_00, ValueSource::Typed, 6_000_00)
        );
        assert_eq!(result.tax_basis_minor, 44_200_00);
        assert_eq!(result.tax_amount_minor, 9_282_00);
    }

    #[test]
    fn a_loss_year_uses_up_earlier_profit_oldest_first() {
        let model = model(
            "legal",
            figures(
                12,
                0,
                &[
                    (ForecastLine::Revenue, Some(10_000_00), None),
                    (ForecastLine::DeductibleCosts, Some(15_000_00), None),
                ],
                vec![earlier(2024, 4_000_00, 1000), earlier(2025, 3_000_00, 700)],
            ),
        );
        let result = calculate_tax_model_results(&model, &[]);

        assert_eq!(result.computed_tax_minor, 0);
        assert_eq!(result.minimum_tax_minor, Some(340_00));
        assert_eq!(result.tax_amount_minor, 340_00);
        assert_eq!(result.total_profit_minor, -5_340_00);
        assert_eq!(result.reserve_fund_generation_minor, 0);
        assert_eq!(result.dividend_from_year_minor, 0);
        assert_eq!(result.dividend_from_year_tax_minor, 0);
        let payable: Vec<(i64, i64, i64)> = result
            .earlier_profits
            .iter()
            .map(|p| (p.profit_year, p.payable_minor, p.dividend_tax_minor))
            .collect();
        assert_eq!(payable, vec![(2024, 0, 0), (2025, 1_660_00, 116_20)]);
        assert_eq!(result.dividend_minor, 1_660_00);
        assert_eq!(result.withholding_tax_minor, 116_20);
        assert_eq!(result.net_dividend_minor, 1_543_80);
        assert_eq!(result.monthly_tax_burden_minor, 28_33);
    }

    #[test]
    fn a_loss_uses_up_the_oldest_year_whatever_order_the_years_come_in() {
        let model = model(
            "legal",
            figures(
                12,
                0,
                &[(ForecastLine::DeductibleCosts, Some(1_000_00), None)],
                vec![earlier(2025, 3_000_00, 700), earlier(2023, 500_00, 700)],
            ),
        );
        let result = calculate_tax_model_results(&model, &[]);
        let payable: Vec<(i64, i64)> = result
            .earlier_profits
            .iter()
            .map(|p| (p.profit_year, p.payable_minor))
            .collect();
        assert_eq!(result.total_profit_minor, -1_340_00);
        assert_eq!(payable, vec![(2023, 0), (2025, 2_160_00)]);
    }

    #[test]
    fn a_loss_bigger_than_every_earlier_profit_leaves_no_dividend() {
        let model = model(
            "legal",
            figures(
                12,
                0,
                &[(ForecastLine::DeductibleCosts, Some(10_000_00), None)],
                vec![earlier(2025, 3_000_00, 700)],
            ),
        );
        let result = calculate_tax_model_results(&model, &[]);
        assert_eq!(result.earlier_profits[0].payable_minor, 0);
        assert_eq!(result.dividend_minor, 0);
        assert_eq!(result.net_dividend_minor, 0);
    }

    #[test]
    fn company_bracket_follows_revenue_with_the_bound_inside() {
        let tax = |revenue_minor, deductible_minor| {
            let result = calculate_tax_model_results(
                &model("legal", whole_year(revenue_minor, deductible_minor)),
                &[],
            );
            (result.computed_tax_minor, result.applied_flat_rate_bps)
        };
        assert_eq!(tax(100_000_00, 60_000_00), (4_000_00, Some(1000)));
        assert_eq!(tax(100_000_01, 60_000_00), (8_400_00, Some(2100)));
        assert_eq!(tax(200_000_00, 199_000_00), (210_00, Some(2100)));
    }

    #[test]
    fn physical_person_bracket_follows_the_tax_base_with_the_bound_inside() {
        let tax = |revenue_minor, deductible_minor| {
            let result = calculate_tax_model_results(
                &model("physical", whole_year(revenue_minor, deductible_minor)),
                &[],
            );
            (result.computed_tax_minor, result.applied_flat_rate_bps)
        };
        assert_eq!(tax(300_000_00, 200_000_00), (10_000_00, Some(1000)));
        assert_eq!(tax(300_000_00, 199_999_99), (21_000_00, Some(2100)));
    }

    #[test]
    fn minimum_tax_replaces_a_lower_computed_tax_for_a_company() {
        let result = calculate_tax_model_results(&model("legal", whole_year(3_000_00, 0)), &[]);
        assert_eq!(result.computed_tax_minor, 300_00);
        assert_eq!(result.minimum_tax_minor, Some(340_00));
        assert_eq!(result.tax_amount_minor, 340_00);
        assert_eq!(result.total_profit_minor, 2_660_00);
    }

    #[test]
    fn negative_revenue_still_gets_the_first_bands_minimum_tax() {
        let credit_note = make_event(1, EntryKind::Revenue, -1_000_00, None, None, None, None);
        let result = calculate_tax_model_results(
            &model("legal", figures(12, 0, &[], vec![])),
            &[credit_note],
        );
        assert_eq!(result.total_income_minor, -1_000_00);
        assert_eq!(result.computed_tax_minor, 0);
        assert_eq!(result.applied_flat_rate_bps, Some(1000));
        assert_eq!(result.minimum_tax_minor, Some(340_00));
        assert_eq!(result.tax_amount_minor, 340_00);
    }

    #[test]
    fn a_physical_person_never_gets_the_minimum_tax() {
        let result = calculate_tax_model_results(&model("physical", whole_year(3_000_00, 0)), &[]);
        assert_eq!(result.computed_tax_minor, 300_00);
        assert_eq!(result.minimum_tax_minor, None);
        assert_eq!(result.tax_amount_minor, 300_00);
    }

    #[test]
    fn a_physical_person_ignores_earlier_profit() {
        let model = model(
            "physical",
            figures(12, 0, &[], vec![earlier(2025, 3_000_00, 700)]),
        );
        let result = calculate_tax_model_results(&model, &[]);
        assert!(result.earlier_profits.is_empty());
        assert_eq!(result.dividend_minor, 0);
        assert_eq!(result.withholding_tax_minor, 0);
    }

    #[test]
    fn an_entry_after_the_actual_months_is_listed_but_not_counted() {
        let model = model("legal", figures(9, 0, &[], vec![]));
        let october = EventDataForCalc {
            event_date: "2026-10-05T00:00:00".to_owned(),
            month: 10,
            ..make_event(1, EntryKind::Expense, 50_000, None, None, None, None)
        };
        let result = calculate_tax_model_results(&model, &[october]);
        assert_eq!(result.total_tax_deductible_expenses_minor, 0);
        assert_eq!(
            result.event_breakdowns[0].not_counted,
            Some(EntryNotCountedReason::AfterActualMonths)
        );
        assert_eq!(result.event_breakdowns[0].kind, EntryKind::Expense);
    }
}
