use rusqlite::Connection;

use super::business_use::AssetUseTerms;
use super::purchase::split_purchase;
use crate::error::AppError;
use crate::features::persons::repository::get_person_vat_payer;

pub(crate) struct PurchaseRequest<'a> {
    pub person_id: Option<i64>,
    pub typed_price_minor: Option<i64>,
    pub purchase_date: Option<&'a str>,
    pub depreciation_period_months: Option<i64>,
    pub given_terms: Option<&'a AssetUseTerms>,
    pub stored_terms: &'a AssetUseTerms,
}

pub(crate) struct ResolvedPurchase {
    pub price_minor: Option<i64>,
    pub start_value_minor: Option<i64>,
    pub terms: AssetUseTerms,
}

pub(crate) fn resolve_purchase(
    conn: &Connection,
    request: PurchaseRequest,
) -> Result<ResolvedPurchase, AppError> {
    let price_or_date_cleared =
        request.typed_price_minor.is_none() || request.purchase_date.is_none();
    let terms = match request.given_terms {
        Some(given) => given.clone(),
        None if price_or_date_cleared => AssetUseTerms::default(),
        None => request.stored_terms.clone(),
    };
    let vat_split = terms
        .purchase_vat
        .as_ref()
        .map(|purchase_vat| split_purchase(purchase_vat, terms.pct_at_purchase()));
    let price_minor = vat_split.map_or(request.typed_price_minor, |split| {
        Some(split.depreciating_minor)
    });
    let purchase_month = price_minor
        .and(request.purchase_date)
        .and_then(|date| date.get(..7));
    let period_months = request.depreciation_period_months.filter(|&m| m != 0);
    terms.validate(purchase_month, period_months)?;

    let vat_facts_are_new =
        terms.purchase_vat.is_some() && terms.purchase_vat != request.stored_terms.purchase_vat;
    if let (true, Some(person_id)) = (vat_facts_are_new, request.person_id) {
        if !get_person_vat_payer(conn, person_id)? {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Only a VAT payer's asset can have a VAT split".into(),
            });
        }
    }
    Ok(ResolvedPurchase {
        price_minor,
        start_value_minor: vat_split.map(|split| split.start_value_minor),
        terms,
    })
}
