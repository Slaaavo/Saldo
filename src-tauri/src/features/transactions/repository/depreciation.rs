use crate::error::AppError;
use crate::features::persons::repository::get_default_expense_account_id;
use crate::shared::{format_end_of_month, last_day_of_month, local_now, with_savepoint_app};
use rusqlite::{params, Connection, OptionalExtension};

use super::taxable_events::insert_taxable_event_data;

// ---------------------------------------------------------------------------
// Public context struct (re-used by Phase 8 recalculation)
// ---------------------------------------------------------------------------

pub(crate) struct AssetDepreciationContext {
    pub asset_id: i64,
    pub asset_name: String,
    pub person_id: i64,
    pub purchase_price_minor: i64,
    pub purchase_date: String,
    pub depreciation_period_months: i64,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parse a `YYYY-MM-DD[...]` string into `(year, month, day)`.
fn parse_ymd(date: &str) -> Result<(i32, i32, i32), AppError> {
    let s = &date[..10]; // take exactly the date portion
    let parts: Vec<&str> = s.splitn(3, '-').collect();
    if parts.len() != 3 {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: format!("Invalid date string: {}", date),
        });
    }
    let year: i32 = parts[0].parse().map_err(|_| AppError {
        code: "VALIDATION".into(),
        message: format!("Invalid year in date: {}", date),
    })?;
    let month: i32 = parts[1].parse().map_err(|_| AppError {
        code: "VALIDATION".into(),
        message: format!("Invalid month in date: {}", date),
    })?;
    let day: i32 = parts[2].parse().map_err(|_| AppError {
        code: "VALIDATION".into(),
        message: format!("Invalid day in date: {}", date),
    })?;
    Ok((year, month, day))
}

/// Add `offset` months to a (year, month) pair, returning the new (year, month).
fn add_months(year: i32, month: i32, offset: i64) -> (i32, u32) {
    let total_months = (year as i64) * 12 + (month as i64 - 1) + offset;
    let new_year = (total_months / 12) as i32;
    let new_month = (total_months % 12 + 1) as u32;
    (new_year, new_month)
}

// ---------------------------------------------------------------------------
// Per-asset generation (pub(crate) so Phase 8 can call it)
// ---------------------------------------------------------------------------

/// Generate all missing monthly depreciation events for a single depreciating asset.
/// Returns the number of new events created.
pub(crate) fn generate_depreciation_for_single_asset(
    conn: &Connection,
    ctx: &AssetDepreciationContext,
) -> Result<i64, AppError> {
    with_savepoint_app(conn, || {
        let (purchase_year, purchase_month, purchase_day) = parse_ymd(&ctx.purchase_date)?;

        let today_str = local_now();
        let (today_year, today_month, _) = parse_ymd(&today_str)?;

        let today_offset =
            (today_year - purchase_year) as i64 * 12 + (today_month - purchase_month) as i64;

        // No events for assets purchased in the future.
        if today_offset < 0 {
            return Ok(0);
        }

        let last_eligible_offset = ctx.depreciation_period_months - 1;
        let max_offset = (today_offset - 1).min(last_eligible_offset);

        let monthly_amount = ctx.purchase_price_minor / ctx.depreciation_period_months;

        // Pre-compute proration values for the first month (only relevant when n > 1 and
        // the purchase happened mid-month).
        let (prorated_first, first_month_shortfall) =
            if purchase_day > 1 && ctx.depreciation_period_months > 1 {
                let total_days = last_day_of_month(purchase_year, purchase_month as u32) as i64;
                let remaining_days = total_days - purchase_day as i64 + 1;
                let pf = monthly_amount * remaining_days / total_days;
                let shortfall = monthly_amount - pf;
                (pf, shortfall)
            } else {
                (monthly_amount, 0_i64)
            };

        let mut created: i64 = 0;

        for offset in 0..=max_offset {
            let (event_year, event_month) = add_months(purchase_year, purchase_month, offset);
            let year_month_prefix = format!("{:04}-{:02}%", event_year, event_month);

            // Idempotency: skip if a system-generated event for this asset in this month
            // already exists.
            let exists: Option<i64> = conn
                .query_row(
                    "SELECT 1
                     FROM event e
                     JOIN event_data ed ON ed.id = e.latest_data_id
                     WHERE e.linked_asset_id = ?1
                       AND e.is_system_generated = 1
                       AND e.deleted_at IS NULL
                       AND ed.event_date LIKE ?2",
                    params![ctx.asset_id, year_month_prefix],
                    |row| row.get(0),
                )
                .optional()
                .map_err(AppError::from)?;

            if exists.is_some() {
                continue;
            }

            // Compute the depreciation amount for this month.
            let is_first = offset == 0;
            let is_last = offset == last_eligible_offset;

            let amount = if is_first && is_last {
                // Single-month asset: expense the full purchase price.
                ctx.purchase_price_minor
            } else if is_first {
                prorated_first
            } else if is_last {
                // Last month gets the integer-division remainder plus the proration shortfall
                // subtracted from month 1, so the total always equals purchase_price_minor.
                ctx.purchase_price_minor - (ctx.depreciation_period_months - 1) * monthly_amount
                    + first_month_shortfall
            } else {
                monthly_amount
            };

            let expense_account_id = get_default_expense_account_id(conn, ctx.person_id)?;

            conn.execute(
                "INSERT INTO event (account_id, event_type, linked_asset_id, is_system_generated)
                 VALUES (?1, 'expense', ?2, 1)",
                params![expense_account_id, ctx.asset_id],
            )
            .map_err(AppError::from)?;
            let event_id = conn.last_insert_rowid();

            let event_date = format_end_of_month(event_year, event_month);
            let note = format!(
                "Depreciation: {} ({:02}/{})",
                ctx.asset_name, event_month, event_year
            );

            insert_taxable_event_data(
                conn,
                event_id,
                amount,
                &event_date,
                Some(note.as_str()),
                None,
                None,
                Some(10000),
                None,
            )
            .map_err(AppError::from)?;

            created += 1;
        }

        Ok(created)
    })
}

// ---------------------------------------------------------------------------
// Top-level generator — called at startup
// ---------------------------------------------------------------------------

pub fn generate_depreciation_events(conn: &Connection) -> Result<(), AppError> {
    // ── Depreciating assets ──────────────────────────────────────────────────
    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.name, a.person_id,
                    a.purchase_price_minor, a.purchase_date, a.depreciation_period_months
             FROM account a
             WHERE a.account_type = 'asset'
               AND a.purchase_price_minor IS NOT NULL
               AND a.purchase_date IS NOT NULL
               AND a.depreciation_period_months IS NOT NULL
               AND a.depreciation_period_months > 0",
        )
        .map_err(AppError::from)?;

    let depreciating: Vec<AssetDepreciationContext> = stmt
        .query_map([], |row| {
            Ok(AssetDepreciationContext {
                asset_id: row.get(0)?,
                asset_name: row.get(1)?,
                person_id: row.get(2)?,
                purchase_price_minor: row.get(3)?,
                purchase_date: row.get(4)?,
                depreciation_period_months: row.get(5)?,
            })
        })
        .map_err(AppError::from)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;

    for ctx in &depreciating {
        match generate_depreciation_for_single_asset(conn, ctx) {
            Ok(n) if n > 0 => eprintln!(
                "[depreciation] {} new event(s) for asset '{}' (id={})",
                n, ctx.asset_name, ctx.asset_id
            ),
            Ok(_) => {}
            Err(e) => eprintln!(
                "[depreciation] error for asset '{}' (id={}): {}",
                ctx.asset_name, ctx.asset_id, e.message
            ),
        }
    }

    // ── Instant-expense assets (depreciation_period_months IS NULL or = 0) ──
    struct InstantAsset {
        id: i64,
        name: String,
        person_id: i64,
        purchase_price_minor: i64,
        purchase_date: String,
    }

    let mut instant_stmt = conn
        .prepare(
            "SELECT a.id, a.name, a.person_id,
                    a.purchase_price_minor, a.purchase_date
             FROM account a
             WHERE a.account_type = 'asset'
               AND a.purchase_price_minor IS NOT NULL
               AND a.purchase_date IS NOT NULL
               AND (a.depreciation_period_months IS NULL OR a.depreciation_period_months = 0)",
        )
        .map_err(AppError::from)?;

    let instant: Vec<InstantAsset> = instant_stmt
        .query_map([], |row| {
            Ok(InstantAsset {
                id: row.get(0)?,
                name: row.get(1)?,
                person_id: row.get(2)?,
                purchase_price_minor: row.get(3)?,
                purchase_date: row.get(4)?,
            })
        })
        .map_err(AppError::from)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;

    for asset in &instant {
        let exists: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM event
                 WHERE linked_asset_id = ?1
                   AND is_system_generated = 1
                   AND deleted_at IS NULL",
                params![asset.id],
                |row| row.get(0),
            )
            .optional()
            .map_err(AppError::from)?;

        if exists.is_some() {
            continue;
        }

        with_savepoint_app(conn, || {
            let expense_account_id = get_default_expense_account_id(conn, asset.person_id)?;

            conn.execute(
                "INSERT INTO event (account_id, event_type, linked_asset_id, is_system_generated)
                 VALUES (?1, 'expense', ?2, 1)",
                params![expense_account_id, asset.id],
            )
            .map_err(AppError::from)?;
            let event_id = conn.last_insert_rowid();

            // Use the purchase_date directly; normalise to full datetime if needed.
            let event_date = if asset.purchase_date.len() == 10 {
                format!("{}T00:00:00", asset.purchase_date)
            } else {
                asset.purchase_date.clone()
            };
            let note = format!("Depreciation: {} (instant)", asset.name);

            insert_taxable_event_data(
                conn,
                event_id,
                asset.purchase_price_minor,
                &event_date,
                Some(note.as_str()),
                None,
                None,
                Some(10000),
                None,
            )
            .map_err(AppError::from)?;

            eprintln!(
                "[depreciation] instant expense event for asset '{}' (id={})",
                asset.name, asset.id
            );
            Ok(())
        })?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Recalculation — hard-delete all system-generated events and regenerate
// ---------------------------------------------------------------------------

pub(crate) fn recalculate_depreciation_for_asset(
    conn: &Connection,
    asset_account_id: i64,
) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        // 1. Clean up orphaned taxable_cashflow_link rows first (before deleting the
        //    referenced events, to avoid FK constraint violations).
        conn.execute(
            "DELETE FROM taxable_cashflow_link
             WHERE cashflow_event_id IN (
                       SELECT id FROM event
                       WHERE linked_asset_id = ?1 AND is_system_generated = 1
                   )
                OR taxable_event_id IN (
                       SELECT id FROM event
                       WHERE linked_asset_id = ?1 AND is_system_generated = 1
                   )",
            params![asset_account_id],
        )
        .map_err(AppError::from)?;

        // 2. Delete event_data rows for system-generated events of this asset.
        conn.execute(
            "DELETE FROM event_data WHERE event_id IN (
                 SELECT id FROM event
                 WHERE linked_asset_id = ?1 AND is_system_generated = 1
             )",
            params![asset_account_id],
        )
        .map_err(AppError::from)?;

        // 3. Hard-delete the system-generated event rows.
        conn.execute(
            "DELETE FROM event WHERE linked_asset_id = ?1 AND is_system_generated = 1",
            params![asset_account_id],
        )
        .map_err(AppError::from)?;

        // 4. Re-query the asset to determine current depreciation fields.
        struct AssetRow {
            name: String,
            person_id: i64,
            purchase_price_minor: Option<i64>,
            purchase_date: Option<String>,
            depreciation_period_months: Option<i64>,
        }

        let asset: Option<AssetRow> = conn
            .query_row(
                "SELECT name, person_id, purchase_price_minor, purchase_date,
                        depreciation_period_months
                 FROM account WHERE id = ?1 AND account_type = 'asset'",
                params![asset_account_id],
                |row| {
                    Ok(AssetRow {
                        name: row.get(0)?,
                        person_id: row.get(1)?,
                        purchase_price_minor: row.get(2)?,
                        purchase_date: row.get(3)?,
                        depreciation_period_months: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)?;

        let asset = match asset {
            Some(a) => a,
            // Account no longer exists or is not an asset — nothing to do.
            None => return Ok(()),
        };

        let (price, date) = match (asset.purchase_price_minor, asset.purchase_date) {
            (Some(p), Some(d)) => (p, d),
            // Incomplete fields — clearing depreciation means no events.
            _ => return Ok(()),
        };

        let period = asset.depreciation_period_months.unwrap_or(0);

        if period > 0 {
            // Depreciating asset: delegate to the shared per-asset generator.
            let ctx = AssetDepreciationContext {
                asset_id: asset_account_id,
                asset_name: asset.name,
                person_id: asset.person_id,
                purchase_price_minor: price,
                purchase_date: date,
                depreciation_period_months: period,
            };
            generate_depreciation_for_single_asset(conn, &ctx)?;
        } else {
            // Instant expense: price and date set but no depreciation period.
            let expense_account_id = get_default_expense_account_id(conn, asset.person_id)?;
            conn.execute(
                "INSERT INTO event (account_id, event_type, linked_asset_id, is_system_generated)
                 VALUES (?1, 'expense', ?2, 1)",
                params![expense_account_id, asset_account_id],
            )
            .map_err(AppError::from)?;
            let event_id = conn.last_insert_rowid();

            let event_date = if date.len() == 10 {
                format!("{}T00:00:00", date)
            } else {
                date.clone()
            };
            let note = format!("Depreciation: {} (instant)", asset.name);

            insert_taxable_event_data(
                conn,
                event_id,
                price,
                &event_date,
                Some(note.as_str()),
                None,
                None,
                Some(10000),
                None,
            )
            .map_err(AppError::from)?;
        }

        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};

    /// Returns the person_id of the default seeded person (always 1 after migrations).
    fn default_person_id() -> i64 {
        1
    }

    /// Creates an asset account with depreciation fields and returns its ID.
    fn mk_depreciating_asset(
        conn: &Connection,
        name: &str,
        purchase_price_minor: i64,
        purchase_date: &str,
        depreciation_period_months: i64,
    ) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: name.to_owned(),
                currency_id: 1,
                account_type: "asset".to_owned(),
                person_id: Some(default_person_id()),
                purchase_price_minor: Some(purchase_price_minor),
                purchase_date: Some(purchase_date.to_owned()),
                depreciation_period_months: Some(depreciation_period_months),
                ..Default::default()
            },
        )
        .expect("create_account failed")
    }

    /// Creates an instant-expense asset (no depreciation period) and returns its ID.
    fn mk_instant_asset(
        conn: &Connection,
        name: &str,
        purchase_price_minor: i64,
        purchase_date: &str,
    ) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: name.to_owned(),
                currency_id: 1,
                account_type: "asset".to_owned(),
                person_id: Some(default_person_id()),
                purchase_price_minor: Some(purchase_price_minor),
                purchase_date: Some(purchase_date.to_owned()),
                depreciation_period_months: None,
                ..Default::default()
            },
        )
        .expect("create_account failed")
    }

    fn count_events_for_asset(conn: &Connection, asset_id: i64) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM event WHERE linked_asset_id = ?1 AND deleted_at IS NULL",
            params![asset_id],
            |row| row.get(0),
        )
        .expect("count query failed")
    }

    fn get_event_data_for_asset(
        conn: &Connection,
        asset_id: i64,
    ) -> Vec<(i64, String, Option<i64>)> {
        let mut stmt = conn
            .prepare(
                "SELECT ed.amount_minor, ed.event_date, ed.expense_deductible_pct_bps
                 FROM event e
                 JOIN event_data ed ON ed.id = e.latest_data_id
                 WHERE e.linked_asset_id = ?1
                   AND e.deleted_at IS NULL
                 ORDER BY ed.event_date ASC",
            )
            .expect("prepare failed");
        stmt.query_map(params![asset_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .expect("query failed")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect failed")
    }

    // ── Test: correct event count for a 12-month asset past full term ────────

    #[test]
    fn test_depreciation_creates_correct_event_count() {
        let conn = initialize_in_memory().expect("DB init failed");
        // Purchase in 2024-01-01 → by April 2026, all 12 months are in the past.
        let asset_id = mk_depreciating_asset(&conn, "Car", 12000, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        assert_eq!(count_events_for_asset(&conn, asset_id), 12);
    }

    // ── Test: idempotent — running twice produces no duplicates ──────────────

    #[test]
    fn test_depreciation_idempotent() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_depreciating_asset(&conn, "Laptop", 12000, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("first run failed");
        generate_depreciation_events(&conn).expect("second run failed");
        assert_eq!(count_events_for_asset(&conn, asset_id), 12);
    }

    // ── Test: first month prorated when purchase is mid-month ────────────────

    #[test]
    fn test_depreciation_proration_mid_month() {
        let conn = initialize_in_memory().expect("DB init failed");
        // Jan has 31 days. Bought on Jan 16 → remaining = 31-16+1 = 16 days.
        // monthly_amount = 3100 / 12 = 258 (floor)
        // prorated_first = 258 * 16 / 31 = 133 (floor)
        let asset_id = mk_depreciating_asset(&conn, "Machine", 3100, "2024-01-16", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        let rows = get_event_data_for_asset(&conn, asset_id);
        assert_eq!(rows.len(), 12);
        // First month (Jan 2024) must be prorated: 258 * 16 / 31 = 133
        assert_eq!(rows[0].0, 133, "First month should be prorated");
        // Middle months should be monthly_amount = 258
        assert_eq!(rows[1].0, 258, "Second month should be full monthly_amount");
    }

    // ── Test: no proration when purchased on the 1st ────────────────────────

    #[test]
    fn test_depreciation_no_proration_first_of_month() {
        let conn = initialize_in_memory().expect("DB init failed");
        // monthly_amount = 1200 / 12 = 100
        let asset_id = mk_depreciating_asset(&conn, "Equipment", 1200, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        let rows = get_event_data_for_asset(&conn, asset_id);
        assert_eq!(rows.len(), 12);
        // First month should be exactly monthly_amount = 100
        assert_eq!(rows[0].0, 100, "First month should not be prorated");
    }

    // ── Test: last month accumulates remainder from integer division ─────────

    #[test]
    fn test_depreciation_last_month_remainder() {
        let conn = initialize_in_memory().expect("DB init failed");
        // purchase_price_minor=1000, n=12, monthly_amount=83 (floor)
        // Total of months 0-10 (no proration, 11 months): 11 * 83 = 913
        // Last month: 1000 - 11*83 = 1000 - 913 = 87
        let asset_id = mk_depreciating_asset(&conn, "Tool", 1000, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        let rows = get_event_data_for_asset(&conn, asset_id);
        assert_eq!(rows.len(), 12);
        // Last month (Dec 2024) should be 87
        assert_eq!(rows[11].0, 87, "Last month should include remainder");
        // Verify total = purchase_price_minor
        let total: i64 = rows.iter().map(|r| r.0).sum();
        assert_eq!(total, 1000);
    }

    // ── Test: single instant-expense event created ───────────────────────────

    #[test]
    fn test_instant_expense_single_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_instant_asset(&conn, "Consumable", 500, "2024-03-15");
        generate_depreciation_events(&conn).expect("generator failed");
        assert_eq!(count_events_for_asset(&conn, asset_id), 1);
    }

    // ── Test: event created on default expense account, not asset account ────

    #[test]
    fn test_depreciation_on_default_expense_account() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_depreciating_asset(&conn, "Vehicle", 6000, "2024-01-01", 6);
        generate_depreciation_events(&conn).expect("generator failed");

        // Fetch the account_id from one of the generated events.
        let event_account_id: i64 = conn
            .query_row(
                "SELECT e.account_id FROM event e WHERE e.linked_asset_id = ?1 AND e.deleted_at IS NULL LIMIT 1",
                params![asset_id],
                |row| row.get(0),
            )
            .expect("query failed");

        // The asset_id itself should NOT be the account_id.
        assert_ne!(event_account_id, asset_id);

        // The account should be of type 'default_expense'.
        let account_type: String = conn
            .query_row(
                "SELECT account_type FROM account WHERE id = ?1",
                params![event_account_id],
                |row| row.get(0),
            )
            .expect("query failed");
        assert_eq!(account_type, "default_expense");
    }

    // ── Test: system-generated fields are set correctly ──────────────────────

    #[test]
    fn test_system_generated_fields() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_depreciating_asset(&conn, "Printer", 2400, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");

        let rows: Vec<(i64, i64, Option<i64>)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT e.is_system_generated, e.linked_asset_id,
                            ed.expense_deductible_pct_bps
                     FROM event e
                     JOIN event_data ed ON ed.id = e.latest_data_id
                     WHERE e.linked_asset_id = ?1 AND e.deleted_at IS NULL",
                )
                .expect("prepare failed");
            stmt.query_map(params![asset_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .expect("query failed")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect failed")
        };

        assert!(!rows.is_empty());
        for (is_sys, linked, deductible) in &rows {
            assert_eq!(*is_sys, 1, "is_system_generated must be 1");
            assert_eq!(*linked, asset_id, "linked_asset_id must match");
            assert_eq!(
                *deductible,
                Some(10000),
                "expense_deductible_pct_bps must be 10000"
            );
        }
    }

    // ── Test: future months not generated ────────────────────────────────────

    #[test]
    fn test_future_months_not_generated() {
        let conn = initialize_in_memory().expect("DB init failed");
        // 60-month asset started Jan 2024; expected count grows with each passing month.
        let asset_id = mk_depreciating_asset(&conn, "Building", 60000, "2024-01-01", 60);
        generate_depreciation_events(&conn).expect("generator failed");
        let count = count_events_for_asset(&conn, asset_id);
        let now = local_now();
        let today_year: i32 = now[0..4].parse().unwrap();
        let today_month: u32 = now[5..7].parse().unwrap();
        let today_offset = (today_year - 2024) as i64 * 12 + (today_month as i64 - 1);
        let expected = today_offset.min(60);
        assert_eq!(
            count as i64, expected,
            "Should only generate events up to but not including the current month"
        );
    }

    // ── Test: future purchase date — no events ───────────────────────────────

    #[test]
    fn test_future_purchase_date_no_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_depreciating_asset(&conn, "FutureAsset", 10000, "2030-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        assert_eq!(count_events_for_asset(&conn, asset_id), 0);
    }

    // ── Test: asset purchased in the current month — no events yet ───────────

    #[test]
    fn test_same_month_purchase_no_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        // An asset purchased in the current month should produce no events yet
        // (the current month has not ended, so the expense cannot be booked).
        let today = local_now();
        let purchase_date = &today[..10];
        let asset_id = mk_depreciating_asset(&conn, "CurrentMonthAsset", 12000, purchase_date, 12);
        generate_depreciation_events(&conn).expect("generator failed");
        assert_eq!(
            count_events_for_asset(&conn, asset_id),
            0,
            "No events should be generated for an asset purchased in the current month"
        );
    }

    // ── Test: total across all months equals purchase_price_minor (with proration) ──

    #[test]
    fn test_total_equals_purchase_price_with_proration() {
        let conn = initialize_in_memory().expect("DB init failed");
        // Bought Jan 16, 2024 → prorated first month; total must still = 3100
        let asset_id = mk_depreciating_asset(&conn, "ProrationCheck", 3100, "2024-01-16", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        let rows = get_event_data_for_asset(&conn, asset_id);
        assert_eq!(rows.len(), 12);
        let total: i64 = rows.iter().map(|r| r.0).sum();
        assert_eq!(
            total, 3100,
            "Sum of all depreciation amounts must equal purchase_price_minor"
        );
    }

    // ── Phase 8 tests: recalculate_depreciation_for_asset ────────────────────

    // Test: recalculate after no changes → same count, no duplicates
    #[test]
    fn test_recalculate_no_duplicates() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_depreciating_asset(&conn, "Laptop2", 12000, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("first run failed");
        let count_before = count_events_for_asset(&conn, asset_id);

        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        let count_after = count_events_for_asset(&conn, asset_id);
        assert_eq!(
            count_after, count_before,
            "Recalculate should produce same count, no duplicates"
        );
    }

    // Test: extend period from 12 to 24 months → 24 events
    #[test]
    fn test_recalculate_extend_period() {
        let conn = initialize_in_memory().expect("DB init failed");
        // 12-month asset; all 12 months (Jan-Dec 2024) are in the past by April 2026.
        let asset_id = mk_depreciating_asset(&conn, "ExtendAsset", 24000, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        assert_eq!(count_events_for_asset(&conn, asset_id), 12);

        // Extend to 24 months (Jan 2024 – Dec 2025, all in the past).
        conn.execute(
            "UPDATE account SET depreciation_period_months = 24 WHERE id = ?1",
            params![asset_id],
        )
        .expect("update failed");

        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        assert_eq!(
            count_events_for_asset(&conn, asset_id),
            24,
            "Expected 24 events after extending period"
        );
    }

    // Test: shorten period from 24 to 12 months → only 12 events remain
    #[test]
    fn test_recalculate_shorten_period() {
        let conn = initialize_in_memory().expect("DB init failed");
        // 24-month asset; all 24 months (Jan 2024 – Dec 2025) are in the past by April 2026.
        let asset_id = mk_depreciating_asset(&conn, "ShortenAsset", 24000, "2024-01-01", 24);
        generate_depreciation_events(&conn).expect("generator failed");
        assert_eq!(count_events_for_asset(&conn, asset_id), 24);

        // Shorten to 12 months (only Jan–Dec 2024 eligible).
        conn.execute(
            "UPDATE account SET depreciation_period_months = 12 WHERE id = ?1",
            params![asset_id],
        )
        .expect("update failed");

        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        assert_eq!(
            count_events_for_asset(&conn, asset_id),
            12,
            "Expected 12 events after shortening period"
        );
    }

    // Test: change purchase_price → regenerated events use new amounts
    #[test]
    fn test_recalculate_change_price() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_depreciating_asset(&conn, "PriceChange", 12000, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");

        // Verify original first-month amount is 1000 (12000 / 12).
        let rows_before = get_event_data_for_asset(&conn, asset_id);
        assert_eq!(
            rows_before[0].0, 1000,
            "Original first-month amount should be 1000"
        );

        // Double the purchase price.
        conn.execute(
            "UPDATE account SET purchase_price_minor = 24000 WHERE id = ?1",
            params![asset_id],
        )
        .expect("update failed");

        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        let rows_after = get_event_data_for_asset(&conn, asset_id);
        assert_eq!(rows_after.len(), 12, "Should still have 12 events");
        assert_eq!(
            rows_after[0].0, 2000,
            "First-month amount should reflect new price (24000 / 12 = 2000)"
        );
        let total: i64 = rows_after.iter().map(|r| r.0).sum();
        assert_eq!(total, 24000, "Total must equal new purchase price");
    }

    // Test: clear all depreciation fields → all system-generated events deleted
    #[test]
    fn test_recalculate_clear_fields() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_depreciating_asset(&conn, "ClearAsset", 12000, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        assert_eq!(count_events_for_asset(&conn, asset_id), 12);

        // Clear all depreciation fields.
        conn.execute(
            "UPDATE account SET purchase_price_minor = NULL, purchase_date = NULL, depreciation_period_months = NULL WHERE id = ?1",
            params![asset_id],
        )
        .expect("update failed");

        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        assert_eq!(
            count_events_for_asset(&conn, asset_id),
            0,
            "All system-generated events should be deleted when fields are cleared"
        );
    }

    // Test: orphaned taxable_cashflow_link rows are cleaned up during recalculation
    #[test]
    fn test_recalculate_cleans_orphaned_links() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_depreciating_asset(&conn, "LinkedAsset", 12000, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");

        // Get the ID of one system-generated depreciation event.
        let depreciation_event_id: i64 = conn
            .query_row(
                "SELECT id FROM event WHERE linked_asset_id = ?1 AND is_system_generated = 1 LIMIT 1",
                params![asset_id],
                |row| row.get(0),
            )
            .expect("query failed");

        // Create a regular account and insert a bare event to serve as the cashflow event.
        let cashflow_account_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Checking".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                person_id: Some(default_person_id()),
                ..Default::default()
            },
        )
        .expect("create account failed");
        conn.execute(
            "INSERT INTO event (account_id, event_type) VALUES (?1, 'balance_update')",
            params![cashflow_account_id],
        )
        .expect("insert event failed");
        let cashflow_event_id = conn.last_insert_rowid();

        // Insert an orphan link row.
        conn.execute(
            "INSERT INTO taxable_cashflow_link (taxable_event_id, cashflow_event_id) VALUES (?1, ?2)",
            params![depreciation_event_id, cashflow_event_id],
        )
        .expect("insert link failed");

        let link_before: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM taxable_cashflow_link WHERE taxable_event_id = ?1",
                params![depreciation_event_id],
                |row| row.get(0),
            )
            .expect("count failed");
        assert_eq!(link_before, 1, "Link should exist before recalculation");

        // Recalculate — should clean up the orphaned link.
        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        // The link must be gone (keyed by the old cashflow_event_id which is still valid).
        let link_after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM taxable_cashflow_link WHERE cashflow_event_id = ?1",
                params![cashflow_event_id],
                |row| row.get(0),
            )
            .expect("count failed");
        assert_eq!(
            link_after, 0,
            "Orphaned link should be deleted during recalculation"
        );

        // New depreciation events should have been regenerated.
        assert_eq!(
            count_events_for_asset(&conn, asset_id),
            12,
            "Events should be regenerated after recalculation"
        );
    }

    // Test: change purchase_date → earliest event reflects new start month
    #[test]
    fn test_recalculate_change_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        // 12-month asset starting Jan 2024; all 12 months are past by April 2026.
        let asset_id = mk_depreciating_asset(&conn, "DateChangeAsset", 12000, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        let initial_count = count_events_for_asset(&conn, asset_id);
        assert_eq!(initial_count, 12);

        // Move purchase_date forward to July 2024.
        conn.execute(
            "UPDATE account SET purchase_date = '2024-07-01' WHERE id = ?1",
            params![asset_id],
        )
        .expect("update failed");

        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        // Minimum event_date must now be in July 2024 (not January 2024).
        let min_date: String = conn
            .query_row(
                "SELECT MIN(ed.event_date)
                 FROM event e
                 JOIN event_data ed ON ed.id = e.latest_data_id
                 WHERE e.linked_asset_id = ?1
                   AND e.is_system_generated = 1",
                params![asset_id],
                |row| row.get(0),
            )
            .expect("min date query failed");

        assert!(
            min_date.starts_with("2024-07"),
            "Earliest event should be in July 2024, got: {}",
            min_date
        );
    }

    // Test: set period to NULL → transitions from 12 monthly events to 1 instant expense
    #[test]
    fn test_recalculate_period_zero_creates_instant_expense() {
        let conn = initialize_in_memory().expect("DB init failed");
        // 12-month asset starting Jan 2024; all 12 months are past by April 2026.
        let asset_id = mk_depreciating_asset(&conn, "InstantSwitch", 12000, "2024-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        let initial_count = count_events_for_asset(&conn, asset_id);
        assert!(
            initial_count > 1,
            "Should have more than 1 event before change"
        );

        // Clear the depreciation period → instant expense.
        conn.execute(
            "UPDATE account SET depreciation_period_months = NULL WHERE id = ?1",
            params![asset_id],
        )
        .expect("update failed");

        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        assert_eq!(
            count_events_for_asset(&conn, asset_id),
            1,
            "Should have exactly 1 instant-expense event after clearing depreciation period"
        );
    }

    // Test: recalculate on a freshly created asset generates events immediately
    // (simulates the create_account command path)
    #[test]
    fn test_recalculate_generates_events_for_new_asset() {
        let conn = initialize_in_memory().expect("DB init failed");
        // Create a depreciating asset without calling generate_depreciation_events first.
        let asset_id = mk_depreciating_asset(&conn, "NewAsset", 12000, "2024-01-01", 12);
        // Call recalculate_depreciation_for_asset directly — the same function the command now invokes.
        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");
        // Events should have been generated without needing generate_depreciation_events.
        assert!(
            count_events_for_asset(&conn, asset_id) > 0,
            "Depreciation events should be generated immediately on asset creation"
        );
    }
}
