use std::collections::BTreeMap;

use crate::error::AppError;
use crate::features::tax_models::models::{
    ActualMonths, EarlierProfit, EntryKind, EventDataForCalc, ForecastFigures, ForecastLine,
    MinimumTaxBand, TaxModelBracketRow, TaxModelDetail, TaxModelRow, TypedPair,
};
use crate::shared::{local_now, with_savepoint_app};
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension};

pub struct BracketInput {
    pub sort_order: i64,
    pub lower_bound_minor: i64,
    pub rate_type: String,
    pub flat_rate_bps: Option<i64>,
    pub tiers_json: Option<String>,
}

pub struct CreateTaxModelParams {
    pub name: String,
    pub calendar_year: i64,
    pub person_id: i64,
    pub vat_status: String,
    pub vat_from_date: Option<String>,
    pub reserve_fund_current_minor: Option<i64>,
    pub reserve_fund_pct_bps: Option<i64>,
    pub reserve_fund_max_minor: Option<i64>,
    pub dividend_tax_rate_bps: Option<i64>,
    pub brackets: Vec<BracketInput>,
    pub minimum_tax_bands: Vec<MinimumTaxBand>,
    pub actuals_through_month: ActualMonths,
}

pub struct UpdateTaxModelParams {
    pub model_id: i64,
    pub name: String,
    pub calendar_year: i64,
    pub person_id: i64,
    pub vat_status: String,
    pub vat_from_date: Option<String>,
    pub reserve_fund_current_minor: Option<i64>,
    pub reserve_fund_pct_bps: Option<i64>,
    pub reserve_fund_max_minor: Option<i64>,
    pub dividend_tax_rate_bps: Option<i64>,
    pub brackets: Vec<BracketInput>,
    pub minimum_tax_bands: Vec<MinimumTaxBand>,
}

pub struct SaveTaxForecastFiguresParams {
    pub model_id: i64,
    pub figures: ForecastFigures,
}

fn not_found() -> AppError {
    AppError {
        code: "NOT_FOUND".into(),
        message: "Tax model not found.".into(),
    }
}

fn insert_minimum_tax_bands(
    conn: &Connection,
    model_id: i64,
    bands: &[MinimumTaxBand],
) -> Result<(), AppError> {
    for band in bands {
        conn.execute(
            "INSERT INTO tax_model_minimum_tax_band (tax_model_id, lower_bound_minor, amount_minor)
             VALUES (?1, ?2, ?3)",
            params![model_id, band.lower_bound_minor, band.amount_minor],
        )
        .map_err(AppError::from)?;
    }
    Ok(())
}

fn insert_brackets(
    conn: &Connection,
    model_id: i64,
    brackets: &[BracketInput],
) -> Result<(), AppError> {
    for b in brackets {
        conn.execute(
            "INSERT INTO tax_model_bracket
             (tax_model_id, sort_order, lower_bound_minor, rate_type, flat_rate_bps, tiers_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                model_id,
                b.sort_order,
                b.lower_bound_minor,
                b.rate_type,
                b.flat_rate_bps,
                b.tiers_json,
            ],
        )
        .map_err(AppError::from)?;
    }
    Ok(())
}

pub fn create_tax_model(conn: &Connection, p: CreateTaxModelParams) -> Result<i64, AppError> {
    let now = local_now();
    with_savepoint_app(conn, || {
        conn.execute(
            "INSERT INTO tax_model
             (name, calendar_year, person_id, vat_status, vat_from_date,
              reserve_fund_current_minor, reserve_fund_pct_bps, reserve_fund_max_minor,
              dividend_tax_rate_bps, actuals_through_month, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                p.name,
                p.calendar_year,
                p.person_id,
                p.vat_status,
                p.vat_from_date,
                p.reserve_fund_current_minor,
                p.reserve_fund_pct_bps,
                p.reserve_fund_max_minor,
                p.dividend_tax_rate_bps,
                p.actuals_through_month.get(),
                now,
                now,
            ],
        )
        .map_err(AppError::from)?;
        let model_id = conn.last_insert_rowid();
        insert_brackets(conn, model_id, &p.brackets)?;
        insert_minimum_tax_bands(conn, model_id, &p.minimum_tax_bands)?;
        Ok(model_id)
    })
}

pub fn list_tax_models(conn: &Connection) -> rusqlite::Result<Vec<TaxModelRow>> {
    let mut stmt = conn.prepare(
        "SELECT tm.id, tm.name, tm.calendar_year, tm.person_id,
                p.name AS person_name, p.person_type,
                tm.vat_status, tm.vat_from_date,
                tm.reserve_fund_current_minor, tm.reserve_fund_pct_bps,
                tm.reserve_fund_max_minor, tm.dividend_tax_rate_bps,
                tm.created_at, tm.updated_at
         FROM tax_model tm
         JOIN person p ON p.id = tm.person_id
         ORDER BY tm.calendar_year DESC, tm.name ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(TaxModelRow {
            id: row.get(0)?,
            name: row.get(1)?,
            calendar_year: row.get(2)?,
            person_id: row.get(3)?,
            person_name: row.get(4)?,
            person_type: row.get(5)?,
            vat_status: row.get(6)?,
            vat_from_date: row.get(7)?,
            reserve_fund_current_minor: row.get(8)?,
            reserve_fund_pct_bps: row.get(9)?,
            reserve_fund_max_minor: row.get(10)?,
            dividend_tax_rate_bps: row.get(11)?,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
        })
    })?;
    rows.collect()
}

pub fn get_tax_model(conn: &Connection, model_id: i64) -> Result<TaxModelDetail, AppError> {
    let maybe_row: Option<(TaxModelRow, i64, i64)> = conn
        .query_row(
            "SELECT tm.id, tm.name, tm.calendar_year, tm.person_id,
                    p.name AS person_name, p.person_type,
                    tm.vat_status, tm.vat_from_date,
                    tm.reserve_fund_current_minor, tm.reserve_fund_pct_bps,
                    tm.reserve_fund_max_minor, tm.dividend_tax_rate_bps,
                    tm.created_at, tm.updated_at,
                    tm.actuals_through_month, tm.payroll_monthly_minor
             FROM tax_model tm
             JOIN person p ON p.id = tm.person_id
             WHERE tm.id = ?1",
            params![model_id],
            |row| {
                let model_row = TaxModelRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    calendar_year: row.get(2)?,
                    person_id: row.get(3)?,
                    person_name: row.get(4)?,
                    person_type: row.get(5)?,
                    vat_status: row.get(6)?,
                    vat_from_date: row.get(7)?,
                    reserve_fund_current_minor: row.get(8)?,
                    reserve_fund_pct_bps: row.get(9)?,
                    reserve_fund_max_minor: row.get(10)?,
                    dividend_tax_rate_bps: row.get(11)?,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                };
                Ok((model_row, row.get(14)?, row.get(15)?))
            },
        )
        .optional()
        .map_err(AppError::from)?;

    let (row, actuals_through_month, payroll_monthly_minor) = maybe_row.ok_or_else(not_found)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, sort_order, lower_bound_minor, rate_type, flat_rate_bps, tiers_json
             FROM tax_model_bracket
             WHERE tax_model_id = ?1
             ORDER BY sort_order ASC",
        )
        .map_err(AppError::from)?;

    let brackets: Vec<TaxModelBracketRow> = stmt
        .query_map(params![model_id], |r| {
            Ok(TaxModelBracketRow {
                id: r.get(0)?,
                sort_order: r.get(1)?,
                lower_bound_minor: r.get(2)?,
                rate_type: r.get(3)?,
                flat_rate_bps: r.get(4)?,
                tiers_json: r.get(5)?,
            })
        })
        .map_err(AppError::from)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;

    Ok(TaxModelDetail {
        id: row.id,
        name: row.name,
        calendar_year: row.calendar_year,
        person_id: row.person_id,
        person_name: row.person_name,
        person_type: row.person_type,
        vat_status: row.vat_status,
        vat_from_date: row.vat_from_date,
        reserve_fund_current_minor: row.reserve_fund_current_minor,
        reserve_fund_pct_bps: row.reserve_fund_pct_bps,
        reserve_fund_max_minor: row.reserve_fund_max_minor,
        dividend_tax_rate_bps: row.dividend_tax_rate_bps,
        created_at: row.created_at,
        updated_at: row.updated_at,
        brackets,
        minimum_tax_bands: read_minimum_tax_bands(conn, model_id)?,
        figures: ForecastFigures {
            actuals_through_month: ActualMonths::new(actuals_through_month)?,
            payroll_monthly_minor,
            typed_values: read_typed_values(conn, model_id)?,
            earlier_profits: read_earlier_profits(conn, model_id)?,
        },
    })
}

fn read_typed_values(
    conn: &Connection,
    model_id: i64,
) -> Result<BTreeMap<ForecastLine, TypedPair>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT line, so_far_minor, rest_minor
             FROM tax_model_typed_value
             WHERE tax_model_id = ?1",
        )
        .map_err(AppError::from)?;
    let rows = stmt
        .query_map(params![model_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                TypedPair {
                    so_far_minor: r.get(1)?,
                    rest_minor: r.get(2)?,
                },
            ))
        })
        .map_err(AppError::from)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;
    rows.into_iter()
        .map(|(line, pair)| Ok((ForecastLine::parse(&line)?, pair)))
        .collect()
}

fn read_earlier_profits(conn: &Connection, model_id: i64) -> Result<Vec<EarlierProfit>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT profit_year, amount_minor, dividend_tax_rate_bps
             FROM tax_model_earlier_profit
             WHERE tax_model_id = ?1
             ORDER BY profit_year ASC",
        )
        .map_err(AppError::from)?;
    let rows = stmt
        .query_map(params![model_id], |r| {
            Ok(EarlierProfit {
                profit_year: r.get(0)?,
                amount_minor: r.get(1)?,
                dividend_tax_rate_bps: r.get(2)?,
            })
        })
        .map_err(AppError::from)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;
    Ok(rows)
}

fn read_minimum_tax_bands(
    conn: &Connection,
    model_id: i64,
) -> Result<Vec<MinimumTaxBand>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT lower_bound_minor, amount_minor
             FROM tax_model_minimum_tax_band
             WHERE tax_model_id = ?1
             ORDER BY lower_bound_minor ASC",
        )
        .map_err(AppError::from)?;
    let rows = stmt
        .query_map(params![model_id], |r| {
            Ok(MinimumTaxBand {
                lower_bound_minor: r.get(0)?,
                amount_minor: r.get(1)?,
            })
        })
        .map_err(AppError::from)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;
    Ok(rows)
}

pub fn update_tax_model(conn: &Connection, p: UpdateTaxModelParams) -> Result<(), AppError> {
    let now = local_now();
    with_savepoint_app(conn, || {
        let affected = conn
            .execute(
                "UPDATE tax_model
                 SET name = ?1, calendar_year = ?2, person_id = ?3,
                     vat_status = ?4, vat_from_date = ?5,
                     reserve_fund_current_minor = ?6, reserve_fund_pct_bps = ?7,
                     reserve_fund_max_minor = ?8, dividend_tax_rate_bps = ?9,
                     updated_at = ?10
                 WHERE id = ?11",
                params![
                    p.name,
                    p.calendar_year,
                    p.person_id,
                    p.vat_status,
                    p.vat_from_date,
                    p.reserve_fund_current_minor,
                    p.reserve_fund_pct_bps,
                    p.reserve_fund_max_minor,
                    p.dividend_tax_rate_bps,
                    now,
                    p.model_id,
                ],
            )
            .map_err(AppError::from)?;
        if affected == 0 {
            return Err(not_found());
        }
        conn.execute(
            "DELETE FROM tax_model_bracket WHERE tax_model_id = ?1",
            params![p.model_id],
        )
        .map_err(AppError::from)?;
        insert_brackets(conn, p.model_id, &p.brackets)?;
        conn.execute(
            "DELETE FROM tax_model_minimum_tax_band WHERE tax_model_id = ?1",
            params![p.model_id],
        )
        .map_err(AppError::from)?;
        insert_minimum_tax_bands(conn, p.model_id, &p.minimum_tax_bands)?;
        Ok(())
    })
}

pub fn save_tax_forecast_figures(
    conn: &Connection,
    p: SaveTaxForecastFiguresParams,
) -> Result<(), AppError> {
    let now = local_now();
    let figures = p.figures;
    with_savepoint_app(conn, || {
        let affected = conn
            .execute(
                "UPDATE tax_model
                 SET actuals_through_month = ?1, payroll_monthly_minor = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![
                    figures.actuals_through_month.get(),
                    figures.payroll_monthly_minor,
                    now,
                    p.model_id,
                ],
            )
            .map_err(AppError::from)?;
        if affected == 0 {
            return Err(not_found());
        }
        conn.execute(
            "DELETE FROM tax_model_typed_value WHERE tax_model_id = ?1",
            params![p.model_id],
        )
        .map_err(AppError::from)?;
        for (line, pair) in &figures.typed_values {
            conn.execute(
                "INSERT INTO tax_model_typed_value (tax_model_id, line, so_far_minor, rest_minor)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    p.model_id,
                    line.as_str(),
                    pair.so_far_minor,
                    pair.rest_minor
                ],
            )
            .map_err(AppError::from)?;
        }
        conn.execute(
            "DELETE FROM tax_model_earlier_profit WHERE tax_model_id = ?1",
            params![p.model_id],
        )
        .map_err(AppError::from)?;
        for row in &figures.earlier_profits {
            conn.execute(
                "INSERT INTO tax_model_earlier_profit
                 (tax_model_id, profit_year, amount_minor, dividend_tax_rate_bps)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    p.model_id,
                    row.profit_year,
                    row.amount_minor,
                    row.dividend_tax_rate_bps,
                ],
            )
            .map_err(AppError::from)?;
        }
        Ok(())
    })
}

pub fn delete_tax_model(conn: &Connection, model_id: i64) -> Result<(), AppError> {
    let affected = conn
        .execute("DELETE FROM tax_model WHERE id = ?1", params![model_id])
        .map_err(AppError::from)?;
    if affected == 0 {
        return Err(not_found());
    }
    Ok(())
}

pub fn delete_tax_models_for_person(conn: &Connection, person_id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM tax_model WHERE person_id = ?1",
        params![person_id],
    )
    .map_err(AppError::from)?;
    Ok(())
}

pub struct ListTaxableEventsForModelParams {
    pub person_id: i64,
    pub calendar_year: i32,
}

fn unreadable_entry(column: usize, column_type: Type, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(column, column_type, message.into())
}

pub fn list_taxable_events_for_model(
    conn: &Connection,
    params: ListTaxableEventsForModelParams,
) -> Result<Vec<EventDataForCalc>, AppError> {
    let year_str = params.calendar_year.to_string();
    let mut stmt = conn
        .prepare(
            "SELECT e.id, e.event_type, ed.amount_minor, ed.event_date, ed.note,
                    ed.vat_rate_bps, ed.vat_reclaimable_pct_bps,
                    ed.expense_deductible_pct_bps, ed.reclaimed_vat,
                    COALESCE(asset.depreciation_period_months, 0) > 0,
                    e.linked_prepaid_event_id IS NOT NULL,
                    CAST(strftime('%m', ed.event_date) AS INTEGER)
             FROM event e
             JOIN event_data ed ON ed.id = e.latest_data_id
             JOIN account a ON a.id = e.account_id
             LEFT JOIN account asset ON asset.id = e.linked_asset_id
             WHERE e.deleted_at IS NULL
               AND e.event_type IN ('revenue', 'expense')
               AND a.person_id = ?1
               AND strftime('%Y', ed.event_date) = ?2
             ORDER BY ed.event_date ASC",
        )
        .map_err(AppError::from)?;

    let rows = stmt
        .query_map(params![params.person_id, year_str], |row| {
            let event_id: i64 = row.get(0)?;
            let kind = match (
                row.get::<_, String>(1)?.as_str(),
                row.get::<_, bool>(9)?,
                row.get::<_, bool>(10)?,
            ) {
                ("revenue", _, _) => EntryKind::Revenue,
                ("expense", true, _) => EntryKind::Depreciation,
                ("expense", false, true) => EntryKind::Prepaid,
                ("expense", false, false) => EntryKind::Expense,
                (other, _, _) => {
                    return Err(unreadable_entry(
                        1,
                        Type::Text,
                        format!("Event {event_id} has the non-taxable type '{other}'."),
                    ))
                }
            };

            let month = row
                .get::<_, Option<i64>>(11)?
                .and_then(|month| u8::try_from(month).ok())
                .filter(|month| (1..=12).contains(month))
                .ok_or_else(|| {
                    unreadable_entry(
                        11,
                        Type::Integer,
                        format!("Event {event_id} has no valid month."),
                    )
                })?;
            Ok(EventDataForCalc {
                event_id,
                kind,
                month,
                amount_minor: row.get(2)?,
                event_date: row.get(3)?,
                note: row.get(4)?,
                vat_rate_bps: row.get(5)?,
                vat_reclaimable_pct_bps: row.get(6)?,
                expense_deductible_pct_bps: row.get(7)?,
                reclaimed_vat: row.get::<_, Option<i64>>(8)?.map(|v| v != 0),
            })
        })
        .map_err(AppError::from)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::persons::repository::get_default_expense_account_id;
    use crate::features::tax_models::calculation::calculate_tax_model_results;
    use crate::features::transactions::repository::depreciation::generate_depreciation_events;
    use crate::features::transactions::repository::prepaid_expenses::{
        generate_prepaid_child_events, PrepaidExpenseContext,
    };
    use crate::features::transactions::repository::{
        create_taxable_event, CreateTaxableEventParams,
    };

    fn get_default_person_id(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM person WHERE is_default = 1", [], |row| {
            row.get(0)
        })
        .expect("default person not found")
    }

    fn flat_bracket(sort_order: i64, lower_minor: i64, rate_bps: i64) -> BracketInput {
        BracketInput {
            sort_order,
            lower_bound_minor: lower_minor,
            rate_type: "flat".to_owned(),
            flat_rate_bps: Some(rate_bps),
            tiers_json: None,
        }
    }

    fn band(lower_bound_minor: i64, amount_minor: i64) -> MinimumTaxBand {
        MinimumTaxBand {
            lower_bound_minor,
            amount_minor,
        }
    }

    fn model_params(person_id: i64, calendar_year: i64) -> CreateTaxModelParams {
        CreateTaxModelParams {
            name: format!("{calendar_year} model"),
            calendar_year,
            person_id,
            vat_status: "none".to_owned(),
            vat_from_date: None,
            reserve_fund_current_minor: None,
            reserve_fund_pct_bps: None,
            reserve_fund_max_minor: None,
            dividend_tax_rate_bps: None,
            brackets: vec![flat_bracket(0, 0, 1000)],
            minimum_tax_bands: vec![],
            actuals_through_month: ActualMonths::new(12).unwrap(),
        }
    }

    fn sample_figures() -> ForecastFigures {
        ForecastFigures {
            actuals_through_month: ActualMonths::new(9).unwrap(),
            payroll_monthly_minor: 4_000_00,
            typed_values: BTreeMap::from([
                (
                    ForecastLine::Revenue,
                    TypedPair {
                        so_far_minor: Some(90_000_00),
                        rest_minor: None,
                    },
                ),
                (
                    ForecastLine::DeductibleCosts,
                    TypedPair {
                        so_far_minor: None,
                        rest_minor: Some(5_600_00),
                    },
                ),
            ]),
            earlier_profits: vec![
                EarlierProfit {
                    profit_year: 2024,
                    amount_minor: 4_000_00,
                    dividend_tax_rate_bps: 1000,
                },
                EarlierProfit {
                    profit_year: 2025,
                    amount_minor: 15_000_00,
                    dividend_tax_rate_bps: 700,
                },
            ],
        }
    }

    fn save(conn: &Connection, model_id: i64, figures: ForecastFigures) {
        save_tax_forecast_figures(conn, SaveTaxForecastFiguresParams { model_id, figures })
            .expect("save_tax_forecast_figures failed");
    }

    fn count_rows(conn: &Connection, table: &str, model_id: i64) -> i64 {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE tax_model_id = ?1"),
            params![model_id],
            |row| row.get(0),
        )
        .expect("count query failed")
    }

    #[test]
    fn test_create_and_get_tax_model() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);

        let id = create_tax_model(
            &conn,
            CreateTaxModelParams {
                name: "My Tax Model".to_owned(),
                calendar_year: 2025,
                person_id,
                vat_status: "none".to_owned(),
                vat_from_date: None,
                reserve_fund_current_minor: None,
                reserve_fund_pct_bps: None,
                reserve_fund_max_minor: None,
                dividend_tax_rate_bps: None,
                brackets: vec![flat_bracket(0, 0, 1900)],
                minimum_tax_bands: vec![band(0, 340_00), band(50_000_00, 960_00)],
                actuals_through_month: ActualMonths::new(9).unwrap(),
            },
        )
        .expect("create_tax_model failed");

        let detail = get_tax_model(&conn, id).expect("get_tax_model failed");
        assert_eq!(detail.id, id);
        assert_eq!(detail.name, "My Tax Model");
        assert_eq!(detail.calendar_year, 2025);
        assert_eq!(detail.person_id, person_id);
        assert_eq!(detail.vat_status, "none");
        assert_eq!(detail.brackets.len(), 1);
        assert_eq!(detail.brackets[0].sort_order, 0);
        assert_eq!(detail.brackets[0].flat_rate_bps, Some(1900));
        assert_eq!(
            detail.minimum_tax_bands,
            vec![band(0, 340_00), band(50_000_00, 960_00)]
        );
        assert_eq!(
            detail.figures,
            ForecastFigures {
                actuals_through_month: ActualMonths::new(9).unwrap(),
                ..ForecastFigures::default()
            }
        );
    }

    #[test]
    fn saved_figures_read_back_and_replace_the_earlier_ones() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_tax_model(&conn, model_params(get_default_person_id(&conn), 2026))
            .expect("create_tax_model failed");

        save(&conn, id, sample_figures());
        assert_eq!(get_tax_model(&conn, id).unwrap().figures, sample_figures());

        let replacement = ForecastFigures {
            actuals_through_month: ActualMonths::new(10).unwrap(),
            payroll_monthly_minor: 0,
            typed_values: BTreeMap::from([(
                ForecastLine::Payroll,
                TypedPair {
                    so_far_minor: Some(40_000_00),
                    rest_minor: Some(8_000_00),
                },
            )]),
            earlier_profits: vec![EarlierProfit {
                profit_year: 2023,
                amount_minor: 1_000_00,
                dividend_tax_rate_bps: 700,
            }],
        };
        save(&conn, id, replacement.clone());
        save(&conn, id, replacement.clone());
        assert_eq!(get_tax_model(&conn, id).unwrap().figures, replacement);
    }

    #[test]
    fn every_forecast_line_round_trips() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_tax_model(&conn, model_params(get_default_person_id(&conn), 2026))
            .expect("create_tax_model failed");
        let figures = ForecastFigures {
            actuals_through_month: ActualMonths::new(6).unwrap(),
            typed_values: ForecastLine::ALL
                .into_iter()
                .zip(1..)
                .map(|(line, n)| {
                    (
                        line,
                        TypedPair {
                            so_far_minor: Some(n * 100),
                            rest_minor: Some(n * 1000),
                        },
                    )
                })
                .collect(),
            ..ForecastFigures::default()
        };
        save(&conn, id, figures.clone());
        assert_eq!(get_tax_model(&conn, id).unwrap().figures, figures);
    }

    #[test]
    fn saving_figures_for_a_missing_model_is_not_found() {
        let conn = initialize_in_memory().expect("DB init failed");
        let err = save_tax_forecast_figures(
            &conn,
            SaveTaxForecastFiguresParams {
                model_id: 999_999,
                figures: sample_figures(),
            },
        )
        .unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");
    }

    #[test]
    fn an_unknown_stored_line_fails_the_read() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_tax_model(&conn, model_params(get_default_person_id(&conn), 2026))
            .expect("create_tax_model failed");
        conn.execute(
            "INSERT INTO tax_model_typed_value (tax_model_id, line, so_far_minor) VALUES (?1, 'bonus', 1)",
            params![id],
        )
        .unwrap();
        assert!(get_tax_model(&conn, id).is_err());
    }

    #[test]
    fn test_update_replaces_brackets_and_bands_but_keeps_figures() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);

        let id = create_tax_model(
            &conn,
            CreateTaxModelParams {
                name: "To Update".to_owned(),
                calendar_year: 2024,
                person_id,
                vat_status: "none".to_owned(),
                vat_from_date: None,
                reserve_fund_current_minor: None,
                reserve_fund_pct_bps: None,
                reserve_fund_max_minor: None,
                dividend_tax_rate_bps: None,
                brackets: vec![flat_bracket(0, 0, 1500)],
                minimum_tax_bands: vec![band(0, 340_00)],
                actuals_through_month: ActualMonths::new(12).unwrap(),
            },
        )
        .expect("create_tax_model failed");
        save(&conn, id, sample_figures());

        update_tax_model(
            &conn,
            UpdateTaxModelParams {
                model_id: id,
                name: "Updated Model".to_owned(),
                calendar_year: 2024,
                person_id,
                vat_status: "all_year".to_owned(),
                vat_from_date: None,
                reserve_fund_current_minor: None,
                reserve_fund_pct_bps: None,
                reserve_fund_max_minor: None,
                dividend_tax_rate_bps: None,
                brackets: vec![flat_bracket(0, 0, 1500), flat_bracket(1, 1_000_000, 2500)],
                minimum_tax_bands: vec![band(0, 400_00), band(100_000_00, 1_000_00)],
            },
        )
        .expect("update_tax_model failed");

        let detail = get_tax_model(&conn, id).expect("get_tax_model failed");
        assert_eq!(detail.name, "Updated Model");
        assert_eq!(detail.vat_status, "all_year");
        assert_eq!(detail.brackets.len(), 2);
        assert_eq!(detail.brackets[1].lower_bound_minor, 1_000_000);
        assert_eq!(detail.brackets[1].flat_rate_bps, Some(2500));
        assert_eq!(
            detail.minimum_tax_bands,
            vec![band(0, 400_00), band(100_000_00, 1_000_00)]
        );
        assert_eq!(detail.figures, sample_figures());
    }

    #[test]
    fn test_delete_tax_model() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);

        let id = create_tax_model(
            &conn,
            CreateTaxModelParams {
                name: "To Delete".to_owned(),
                calendar_year: 2023,
                person_id,
                vat_status: "none".to_owned(),
                vat_from_date: None,
                reserve_fund_current_minor: None,
                reserve_fund_pct_bps: None,
                reserve_fund_max_minor: None,
                dividend_tax_rate_bps: None,
                brackets: vec![flat_bracket(0, 0, 2000)],
                minimum_tax_bands: vec![band(0, 340_00)],
                actuals_through_month: ActualMonths::new(12).unwrap(),
            },
        )
        .expect("create_tax_model failed");
        save(&conn, id, sample_figures());

        delete_tax_model(&conn, id).expect("delete_tax_model failed");

        let result = get_tax_model(&conn, id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "NOT_FOUND");

        for table in [
            "tax_model_bracket",
            "tax_model_minimum_tax_band",
            "tax_model_typed_value",
            "tax_model_earlier_profit",
        ] {
            assert_eq!(
                count_rows(&conn, table, id),
                0,
                "ON DELETE CASCADE did not clear {table}"
            );
        }
    }

    #[test]
    fn test_not_found() {
        let conn = initialize_in_memory().expect("DB init failed");
        let result = get_tax_model(&conn, 999_999);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "NOT_FOUND");
    }

    #[test]
    fn test_list_taxable_events_for_model_integration() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);

        // Resolve the default person's pre-created taxable accounts (migration 024).
        let (revenue_account_id, expense_account_id): (i64, i64) = conn
            .query_row(
                "SELECT default_revenue_account_id, default_expense_account_id FROM person WHERE id = ?1",
                params![person_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("resolve default accounts failed");

        // Revenue event: 100 000 minor, no VAT, in 2026.
        create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id: revenue_account_id,
                event_type: "revenue".to_owned(),
                amount_minor: 100_000,
                event_date: "2026-03-10T00:00:00".to_owned(),
                ..Default::default()
            },
        )
        .expect("create revenue event failed");

        // Expense event: 24 000 minor, 20% VAT (2000 bps), 100% reclaimable and deductible, in 2026.
        create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id: expense_account_id,
                event_type: "expense".to_owned(),
                amount_minor: 24_000,
                event_date: "2026-07-20T00:00:00".to_owned(),
                vat_rate_bps: Some(2000),
                vat_reclaimable_pct_bps: Some(10000),
                expense_deductible_pct_bps: Some(10000),
                reclaimed_vat: Some(false),
                ..Default::default()
            },
        )
        .expect("create expense event failed");

        // A 2025 event that must NOT appear in the 2026 query.
        create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id: revenue_account_id,
                event_type: "revenue".to_owned(),
                amount_minor: 50_000,
                event_date: "2025-12-31T00:00:00".to_owned(),
                ..Default::default()
            },
        )
        .expect("create 2025 event failed");

        let events = list_taxable_events_for_model(
            &conn,
            ListTaxableEventsForModelParams {
                person_id,
                calendar_year: 2026,
            },
        )
        .expect("list_taxable_events_for_model failed");

        assert_eq!(events.len(), 2, "expected exactly 2 events for 2026");

        let rev = events
            .iter()
            .find(|e| e.kind == EntryKind::Revenue)
            .expect("revenue event missing");
        assert_eq!(rev.amount_minor, 100_000);
        assert_eq!(rev.vat_rate_bps, None);
        assert_eq!(rev.month, 3);

        let exp = events
            .iter()
            .find(|e| e.kind == EntryKind::Expense)
            .expect("expense event missing");
        assert_eq!(exp.month, 7);
        assert_eq!(exp.amount_minor, 24_000);
        assert_eq!(exp.vat_rate_bps, Some(2000));
        assert_eq!(exp.vat_reclaimable_pct_bps, Some(10000));
        assert_eq!(exp.expense_deductible_pct_bps, Some(10000));
        assert_eq!(exp.reclaimed_vat, Some(false));

        // Verify aggregate totals through calculate_tax_model_results (flat 20%, vat_status="none").
        // vat_status=none → can_reclaim=false → no VAT reclaimed, so all VAT is cost.
        // net expense = 24000 * 10000 / 12000 = 20000; cost = 20000 + 4000 VAT = 24000
        // tax_deductible = 24000; tax_basis = max(0, 100000 - 24000) = 76000
        // tax = 76000 * 2000 / 10000 = 15200
        // total_profit = 100000 - 24000 - 0 - 15200 = 60800
        let model = TaxModelDetail {
            id: 1,
            name: "Integration Test Model".to_owned(),
            calendar_year: 2026,
            person_id,
            person_name: "Personal".to_owned(),
            person_type: "physical".to_owned(),
            vat_status: "none".to_owned(),
            vat_from_date: None,
            reserve_fund_current_minor: None,
            reserve_fund_pct_bps: None,
            reserve_fund_max_minor: None,
            dividend_tax_rate_bps: None,
            created_at: "2026-01-01T00:00:00".to_owned(),
            updated_at: "2026-01-01T00:00:00".to_owned(),
            brackets: vec![TaxModelBracketRow {
                id: 1,
                sort_order: 0,
                lower_bound_minor: 0,
                rate_type: "flat".to_owned(),
                flat_rate_bps: Some(2000),
                tiers_json: None,
            }],
            minimum_tax_bands: vec![],
            figures: ForecastFigures::default(),
        };
        let result = calculate_tax_model_results(&model, &events);

        assert_eq!(result.total_income_minor, 100_000);
        assert_eq!(result.total_tax_deductible_expenses_minor, 24_000);
        assert_eq!(result.tax_basis_minor, 76_000);
        assert_eq!(result.tax_amount_minor, 15_200);
        assert_eq!(result.total_profit_minor, 60_800);
    }

    #[test]
    fn test_prorated_child_expense_events_appear_in_correct_tax_year() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);

        let (_, expense_account_id): (i64, i64) = conn
            .query_row(
                "SELECT default_revenue_account_id, default_expense_account_id FROM person WHERE id = ?1",
                params![person_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("resolve default accounts failed");

        // Simulate system-generated child expense events from a prepaid expense spanning 2025-2026.
        // The parent prepaid_expense is excluded from tax queries (event_type filter). Only the
        // child expense events (event_type = 'expense') are visible to the tax engine.

        // Year 2025 prorated share: 12 000 minor.
        create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id: expense_account_id,
                event_type: "expense".to_owned(),
                amount_minor: 12_000,
                event_date: "2025-06-15T00:00:00".to_owned(),
                expense_deductible_pct_bps: Some(10000),
                ..Default::default()
            },
        )
        .expect("create 2025 child event failed");

        // Year 2026 prorated share: 12 000 minor.
        create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id: expense_account_id,
                event_type: "expense".to_owned(),
                amount_minor: 12_000,
                event_date: "2026-01-01T00:00:00".to_owned(),
                expense_deductible_pct_bps: Some(10000),
                ..Default::default()
            },
        )
        .expect("create 2026 child event failed");

        // 2025 tax query must return only the 2025 prorated share.
        let events_2025 = list_taxable_events_for_model(
            &conn,
            ListTaxableEventsForModelParams {
                person_id,
                calendar_year: 2025,
            },
        )
        .expect("list for 2025 failed");

        assert_eq!(events_2025.len(), 1, "expected exactly 1 event for 2025");
        assert_eq!(events_2025[0].amount_minor, 12_000);
        assert_eq!(events_2025[0].expense_deductible_pct_bps, Some(10000));

        // 2026 tax query must return only the 2026 prorated share.
        let events_2026 = list_taxable_events_for_model(
            &conn,
            ListTaxableEventsForModelParams {
                person_id,
                calendar_year: 2026,
            },
        )
        .expect("list for 2026 failed");

        assert_eq!(events_2026.len(), 1, "expected exactly 1 event for 2026");
        assert_eq!(events_2026[0].amount_minor, 12_000);
        assert_eq!(events_2026[0].expense_deductible_pct_bps, Some(10000));

        // Each year's tax calculation must use only its own prorated share.
        // Flat 10% rate, no income → tax basis = max(0, 0 - 12000) = 0, tax = 0.
        // The important assertion is total_tax_deductible_expenses = 12 000 per year, not 24 000.
        let make_model = |year: i64| TaxModelDetail {
            id: year,
            name: format!("{year} Test Model"),
            calendar_year: year,
            person_id,
            person_name: "Personal".to_owned(),
            person_type: "physical".to_owned(),
            vat_status: "none".to_owned(),
            vat_from_date: None,
            reserve_fund_current_minor: None,
            reserve_fund_pct_bps: None,
            reserve_fund_max_minor: None,
            dividend_tax_rate_bps: None,
            created_at: format!("{year}-01-01T00:00:00"),
            updated_at: format!("{year}-01-01T00:00:00"),
            brackets: vec![TaxModelBracketRow {
                id: 1,
                sort_order: 0,
                lower_bound_minor: 0,
                rate_type: "flat".to_owned(),
                flat_rate_bps: Some(1000),
                tiers_json: None,
            }],
            minimum_tax_bands: vec![],
            figures: ForecastFigures::default(),
        };

        let result_2025 = calculate_tax_model_results(&make_model(2025), &events_2025);
        assert_eq!(result_2025.total_tax_deductible_expenses_minor, 12_000);
        assert_eq!(result_2025.total_income_minor, 0);

        let result_2026 = calculate_tax_model_results(&make_model(2026), &events_2026);
        assert_eq!(result_2026.total_tax_deductible_expenses_minor, 12_000);
        assert_eq!(result_2026.total_income_minor, 0);
    }

    #[test]
    fn asset_entries_are_depreciation_with_their_month() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);
        create_account(
            &conn,
            CreateAccountParams {
                name: "Laptop".to_owned(),
                currency_id: 1,
                account_type: "asset".to_owned(),
                person_id: Some(person_id),
                purchase_price_minor: Some(120_000),
                purchase_date: Some("2024-01-01".to_owned()),
                depreciation_period_months: Some(12),
                ..Default::default()
            },
        )
        .expect("create asset failed");
        generate_depreciation_events(&conn).expect("generate depreciation failed");
        let expense_account_id: i64 = conn
            .query_row(
                "SELECT default_expense_account_id FROM person WHERE id = ?1",
                params![person_id],
                |row| row.get(0),
            )
            .unwrap();
        create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id: expense_account_id,
                event_type: "expense".to_owned(),
                amount_minor: 5_000,
                event_date: "2024-05-20T00:00:00".to_owned(),
                ..Default::default()
            },
        )
        .expect("create expense failed");

        let events = list_taxable_events_for_model(
            &conn,
            ListTaxableEventsForModelParams {
                person_id,
                calendar_year: 2024,
            },
        )
        .expect("list_taxable_events_for_model failed");

        let depreciation_months: Vec<u8> = events
            .iter()
            .filter(|e| e.kind == EntryKind::Depreciation)
            .map(|e| e.month)
            .collect();
        assert_eq!(depreciation_months, (1..=12).collect::<Vec<u8>>());
        let expenses: Vec<(i64, u8)> = events
            .iter()
            .filter(|e| e.kind == EntryKind::Expense)
            .map(|e| (e.amount_minor, e.month))
            .collect();
        assert_eq!(expenses, vec![(5_000, 5)]);
    }

    fn kinds_and_months(conn: &Connection, person_id: i64, year: i32) -> Vec<(EntryKind, u8)> {
        list_taxable_events_for_model(
            conn,
            ListTaxableEventsForModelParams {
                person_id,
                calendar_year: year,
            },
        )
        .expect("list_taxable_events_for_model failed")
        .iter()
        .map(|e| (e.kind, e.month))
        .collect()
    }

    #[test]
    fn an_instant_write_off_is_an_expense() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);
        for (name, period) in [("Chair", Some(0)), ("Desk", None)] {
            create_account(
                &conn,
                CreateAccountParams {
                    name: name.to_owned(),
                    currency_id: 1,
                    account_type: "asset".to_owned(),
                    person_id: Some(person_id),
                    purchase_price_minor: Some(30_000),
                    purchase_date: Some("2024-03-10".to_owned()),
                    depreciation_period_months: period,
                    ..Default::default()
                },
            )
            .expect("create asset failed");
        }
        generate_depreciation_events(&conn).expect("generate depreciation failed");

        assert_eq!(
            kinds_and_months(&conn, person_id, 2024),
            vec![(EntryKind::Expense, 3), (EntryKind::Expense, 3)]
        );
    }

    #[test]
    fn prepaid_year_shares_are_prepaid_and_the_parent_stays_out() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = get_default_person_id(&conn);
        let account_id = get_default_expense_account_id(&conn, person_id).unwrap();
        let parent_event_id = create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id,
                event_type: "prepaid_expense".to_owned(),
                amount_minor: 24_000,
                event_date: "2026-07-01T00:00:00".to_owned(),
                prepaid_until: Some("2027-06-30".to_owned()),
                ..Default::default()
            },
        )
        .expect("create prepaid expense failed");
        generate_prepaid_child_events(
            &conn,
            &PrepaidExpenseContext {
                parent_event_id,
                account_id,
                expense_date: "2026-07-01".to_owned(),
                prepaid_until: "2027-06-30".to_owned(),
                total_amount_minor: 24_000,
                expense_deductible_pct_bps: None,
                note: None,
                vat_rate_bps: None,
                vat_reclaimable_pct_bps: None,
                reclaimed_vat: false,
            },
        )
        .expect("generate prepaid shares failed");

        assert_eq!(
            kinds_and_months(&conn, person_id, 2026),
            vec![(EntryKind::Prepaid, 7)]
        );
        assert_eq!(
            kinds_and_months(&conn, person_id, 2027),
            vec![(EntryKind::Prepaid, 1)]
        );
    }
}
