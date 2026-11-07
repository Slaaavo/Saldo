use crate::error::AppError;
use crate::features::assets::repository::get_use_terms;
use crate::features::assets::schedule::{asset_entries, parse_ymd, AssetFacts, PlannedEntry};
use crate::features::persons::repository::get_default_expense_account_id;
use crate::shared::{local_now, with_savepoint_app};
use rusqlite::{params, Connection, OptionalExtension};

use super::taxable_events::{insert_taxable_event_data, TaxableEventRow};

struct StoredAsset {
    id: i64,
    person_id: i64,
    facts: AssetFacts,
}

fn entry_exists(conn: &Connection, asset_id: i64, entry: &PlannedEntry) -> Result<bool, AppError> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1
             FROM event e
             JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.linked_asset_id = ?1
               AND e.is_system_generated = 1
               AND e.asset_entry = ?2
               AND e.deleted_at IS NULL
               AND substr(ed.event_date, 1, 7) = ?3",
            params![
                asset_id,
                entry.kind.as_str(),
                entry.event_date.get(..7).unwrap_or(&entry.event_date)
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(AppError::from)?;
    Ok(exists.is_some())
}

fn insert_entry(
    conn: &Connection,
    asset: &StoredAsset,
    entry: &PlannedEntry,
) -> Result<(), AppError> {
    let expense_account_id = get_default_expense_account_id(conn, asset.person_id)?;
    conn.execute(
        "INSERT INTO event (account_id, event_type, linked_asset_id, is_system_generated, asset_entry)
         VALUES (?1, 'expense', ?2, 1, ?3)",
        params![expense_account_id, asset.id, entry.kind.as_str()],
    )
    .map_err(AppError::from)?;
    let event_id = conn.last_insert_rowid();

    insert_taxable_event_data(
        conn,
        TaxableEventRow {
            event_id,
            amount_minor: entry.amount_minor,
            event_date: &entry.event_date,
            note: Some(entry.note.as_str()),
            vat_rate_bps: None,
            vat_reclaimable_pct_bps: None,
            expense_deductible_pct_bps: Some(entry.expense_deductible_pct_bps),
            prepaid_until: None,
            reclaimed_vat: None,
            private_use: false,
        },
    )
    .map_err(AppError::from)
}

fn waits_for_its_purchase_month(facts: &AssetFacts) -> Result<bool, AppError> {
    if facts.depreciation_period_months == 0 {
        return Ok(false);
    }
    let (purchase_year, purchase_month, _) = parse_ymd(&facts.purchase_date)?;
    let (today_year, today_month, _) = parse_ymd(&local_now())?;
    Ok((purchase_year, purchase_month) > (today_year, today_month))
}

fn write_missing_entries(conn: &Connection, asset: &StoredAsset) -> Result<i64, AppError> {
    if waits_for_its_purchase_month(&asset.facts)? {
        return Ok(0);
    }
    with_savepoint_app(conn, || {
        let mut created = 0;
        for entry in asset_entries(&asset.facts)? {
            if !entry_exists(conn, asset.id, &entry)? {
                insert_entry(conn, asset, &entry)?;
                created += 1;
            }
        }
        Ok(created)
    })
}

fn load_assets(conn: &Connection, asset_id: Option<i64>) -> Result<Vec<StoredAsset>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, person_id, purchase_price_minor, purchase_date,
                    COALESCE(depreciation_period_months, 0)
             FROM account
             WHERE account_type = 'asset'
               AND purchase_price_minor IS NOT NULL
               AND purchase_date IS NOT NULL
               AND (?1 IS NULL OR id = ?1)",
        )
        .map_err(AppError::from)?;

    let mut assets = stmt
        .query_map(params![asset_id], |row| {
            Ok(StoredAsset {
                id: row.get(0)?,
                person_id: row.get(2)?,
                facts: AssetFacts {
                    name: row.get(1)?,
                    purchase_price_minor: row.get(3)?,
                    purchase_date: row.get(4)?,
                    depreciation_period_months: row.get(5)?,
                    use_terms: Default::default(),
                },
            })
        })
        .map_err(AppError::from)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)?;

    for asset in &mut assets {
        asset.facts.use_terms = get_use_terms(conn, asset.id)?;
    }
    Ok(assets)
}

// ---------------------------------------------------------------------------
// Top-level generator — called at startup
// ---------------------------------------------------------------------------

pub fn generate_depreciation_events(conn: &Connection) -> Result<(), AppError> {
    for asset in &load_assets(conn, None)? {
        match write_missing_entries(conn, asset) {
            Ok(n) if n > 0 => eprintln!(
                "[depreciation] {} new event(s) for asset '{}' (id={})",
                n, asset.facts.name, asset.id
            ),
            Ok(_) => {}
            Err(e) => eprintln!(
                "[depreciation] error for asset '{}' (id={}): {}",
                asset.facts.name, asset.id, e.message
            ),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Recalculation — hard-delete all system-generated events and regenerate
// ---------------------------------------------------------------------------

/// Hard-delete every system-generated depreciation event of an asset, together with
/// its event_data rows and any taxable_cashflow_link rows that reference it.
pub(crate) fn delete_system_events_for_asset(
    conn: &Connection,
    asset_account_id: i64,
) -> Result<(), AppError> {
    // Link rows go first: they reference the events without ON DELETE CASCADE.
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

    conn.execute(
        "DELETE FROM event_data WHERE event_id IN (
             SELECT id FROM event
             WHERE linked_asset_id = ?1 AND is_system_generated = 1
         )",
        params![asset_account_id],
    )
    .map_err(AppError::from)?;

    conn.execute(
        "DELETE FROM event WHERE linked_asset_id = ?1 AND is_system_generated = 1",
        params![asset_account_id],
    )
    .map_err(AppError::from)?;

    Ok(())
}

pub(crate) fn recalculate_depreciation_for_asset(
    conn: &Connection,
    asset_account_id: i64,
) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        delete_system_events_for_asset(conn, asset_account_id)?;
        for asset in &load_assets(conn, Some(asset_account_id))? {
            write_missing_entries(conn, asset)?;
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
    use crate::features::assets::business_use::{AssetUseTerms, BusinessUseChange};
    use crate::features::assets::purchase::PurchaseVat;
    use crate::features::assets::repository::store_use_terms;

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

    // ── Test: first month gets full monthly_base (no daily proration) ─────────

    #[test]
    fn test_depreciation_proration_mid_month() {
        let conn = initialize_in_memory().expect("DB init failed");
        // Bought on Jan 16 — mid-month purchases no longer receive daily proration.
        // monthly_base = 3100 / 12 = 258
        // Months 1-11: 258 each = 2838; month 12 (last): 3100 - 2838 = 262. Total = 3100.
        let asset_id = mk_depreciating_asset(&conn, "Machine", 3100, "2024-01-16", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        let rows = get_event_data_for_asset(&conn, asset_id);
        assert_eq!(rows.len(), 12);
        // First month (Jan 2024): full-month depreciation = monthly_base = 258
        assert_eq!(rows[0].0, 258, "First month should be full monthly_base");
        // Second month should also be monthly_base = 258
        assert_eq!(rows[1].0, 258, "Second month should be full monthly_base");
        // Verify total equals purchase_price_minor
        let total: i64 = rows.iter().map(|r| r.0).sum();
        assert_eq!(total, 3100);
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

    // ── Test: all schedule months generated (including future ones) ──────────

    #[test]
    fn test_all_schedule_months_generated() {
        let conn = initialize_in_memory().expect("DB init failed");
        // 60-month asset started Jan 2024; all months in the schedule are generated
        // regardless of today's date.
        let asset_id = mk_depreciating_asset(&conn, "Building", 60000, "2024-01-01", 60);
        generate_depreciation_events(&conn).expect("generator failed");
        let count = count_events_for_asset(&conn, asset_id);
        // All months in the depreciation schedule are generated regardless of today's date.
        assert_eq!(count, 60);
    }

    // ── Test: future purchase date — no events ───────────────────────────────

    #[test]
    fn test_future_purchase_date_no_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_depreciating_asset(&conn, "FutureAsset", 10000, "2030-01-01", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        assert_eq!(count_events_for_asset(&conn, asset_id), 0);
    }

    // ── Test: asset purchased in the current month — full schedule generated ──

    #[test]
    fn test_same_month_purchase_generates_full_schedule() {
        let conn = initialize_in_memory().expect("DB init failed");
        // An asset purchased in the current month should generate events for the
        // full depreciation schedule, including the current month and all future
        // months. The future-purchase guard only blocks assets whose purchase date
        // is strictly after the current month.
        let today = local_now();
        let purchase_date = &today[..10];
        let asset_id = mk_depreciating_asset(&conn, "CurrentMonthAsset", 12000, purchase_date, 12);
        generate_depreciation_events(&conn).expect("generator failed");
        assert_eq!(
            count_events_for_asset(&conn, asset_id),
            12,
            "All 12 monthly events should be generated for an asset purchased in the current month"
        );
    }

    // ── Test: total across all months equals purchase_price_minor (mid-month purchase) ──

    #[test]
    fn test_total_equals_purchase_price_mid_month_purchase() {
        let conn = initialize_in_memory().expect("DB init failed");
        // Bought Jan 16, 2024 → no proration; first month gets monthly_base = 3100/12 = 258
        let asset_id = mk_depreciating_asset(&conn, "ProrationCheck", 3100, "2024-01-16", 12);
        generate_depreciation_events(&conn).expect("generator failed");
        let rows = get_event_data_for_asset(&conn, asset_id);
        assert_eq!(rows.len(), 12);
        assert_eq!(
            rows[0].0, 258,
            "First month should be monthly_base = 3100 / 12 = 258"
        );
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

    // ── Test: year-boundary December adjustment and overall total integrity ──

    #[test]
    fn test_yearly_december_adjustment() {
        let conn = initialize_in_memory().expect("DB init failed");
        // purchase_date = "2024-06-01", price = 4900, period = 48
        // monthly_base = 4900 / 48 = 102 (floor)
        let asset_id = mk_depreciating_asset(&conn, "YearlyAdjAsset", 4900, "2024-06-01", 48);
        generate_depreciation_events(&conn).expect("generator failed");

        let rows = get_event_data_for_asset(&conn, asset_id);

        // Total event count must be 48.
        assert_eq!(rows.len(), 48, "Expected 48 depreciation events");

        // Group amounts by calendar year using the first 4 chars of event_date
        // (event_date is "YYYY-MM-DDTHH:MM:SS").
        let mut year_sums: std::collections::BTreeMap<String, i64> =
            std::collections::BTreeMap::new();
        for row in &rows {
            let year = row.1[0..4].to_string();
            *year_sums.entry(year).or_insert(0) += row.0;
        }

        // Year 1: Jun–Dec 2024 — 7 events, yearly_target = round(4900 * 7 / 48) = 715
        assert_eq!(
            year_sums.get("2024").copied().unwrap_or(0),
            715,
            "Year 1 (2024, Jun–Dec) sum should be 715"
        );
        // Year 2: Jan–Dec 2025 — 12 events, yearly_target = round(4900 * 12 / 48) = 1225
        assert_eq!(
            year_sums.get("2025").copied().unwrap_or(0),
            1225,
            "Year 2 (2025, Jan–Dec) sum should be 1225"
        );
        // Year 3: Jan–Dec 2026 — 12 events, yearly_target = round(4900 * 12 / 48) = 1225
        assert_eq!(
            year_sums.get("2026").copied().unwrap_or(0),
            1225,
            "Year 3 (2026, Jan–Dec) sum should be 1225"
        );
        // Year 4: Jan–Dec 2027 — 12 events, yearly_target = round(4900 * 12 / 48) = 1225
        assert_eq!(
            year_sums.get("2027").copied().unwrap_or(0),
            1225,
            "Year 4 (2027, Jan–Dec) sum should be 1225"
        );
        // Year 5: Jan–May 2028 — 5 events, yearly_target = 4900 - 715 - 3*1225 = 510
        assert_eq!(
            year_sums.get("2028").copied().unwrap_or(0),
            510,
            "Year 5 (2028, Jan–May) sum should be 510"
        );

        // The last event (May 2028) absorbs the year-5 rounding remainder:
        // Jan–Apr = 4 * 102 = 408; May = 510 - 408 = 102.
        let last_row = rows.last().expect("rows must not be empty");
        assert_eq!(last_row.0, 102, "Last event (May 2028) should be 102");

        // Overall total must equal purchase_price_minor = 4900.
        let total: i64 = rows.iter().map(|r| r.0).sum();
        assert_eq!(
            total, 4900,
            "Overall sum must equal purchase_price_minor (4900)"
        );
    }

    // ── Test: all 60 months are generated including future months ────────────

    #[test]
    fn test_generates_all_months_including_future() {
        let conn = initialize_in_memory().expect("DB init failed");
        // purchase_date = "2026-01-01", period = 60 months → Jan 2026 – Dec 2030.
        // As of April 2026, only 4 months are in the past (Jan–Apr 2026).
        // The new algorithm generates the full schedule regardless of today's date.
        let asset_id = mk_depreciating_asset(&conn, "FutureMonthsAsset", 60000, "2026-01-01", 60);
        generate_depreciation_events(&conn).expect("generator failed");

        assert_eq!(
            count_events_for_asset(&conn, asset_id),
            60,
            "All 60 monthly events must be generated including future months up to Dec 2030"
        );

        // Verify that events in the future (e.g., 2030) were also generated.
        let rows = get_event_data_for_asset(&conn, asset_id);
        let has_2030_event = rows.iter().any(|r| r.1.starts_with("2030-"));
        assert!(
            has_2030_event,
            "At least one event in 2030 must be present, confirming future months are generated"
        );

        // Overall total must equal purchase_price_minor.
        let total: i64 = rows.iter().map(|r| r.0).sum();
        assert_eq!(
            total, 60000,
            "Overall sum must equal purchase_price_minor (60000)"
        );
    }

    // ── Test: no daily proration for a mid-month purchase ────────────────────

    #[test]
    fn test_no_daily_proration_mid_month() {
        let conn = initialize_in_memory().expect("DB init failed");
        // purchase_date = "2024-01-16" (mid-month), price = 3100, period = 12.
        // monthly_base = 3100 / 12 = 258 (floor).
        // The algorithm must NOT prorate the first month by remaining days in the month.
        // All non-last months get monthly_base = 258; the last month absorbs the remainder.
        // remainder = 3100 - 11 * 258 = 3100 - 2838 = 262.
        let asset_id = mk_depreciating_asset(&conn, "MidMonthNoPro", 3100, "2024-01-16", 12);
        generate_depreciation_events(&conn).expect("generator failed");

        let rows = get_event_data_for_asset(&conn, asset_id);
        assert_eq!(rows.len(), 12, "Expected 12 depreciation events");

        let monthly_base: i64 = 258; // 3100 / 12

        // First month (Jan 2024): must be monthly_base, NOT the old prorated value (~133).
        assert_eq!(
            rows[0].0, monthly_base,
            "First month must be full monthly_base ({}), not the old daily-prorated value",
            monthly_base
        );

        // Every non-last month must equal monthly_base.
        for i in 0..11 {
            assert_eq!(
                rows[i].0,
                monthly_base,
                "Month {} should equal monthly_base ({})",
                i + 1,
                monthly_base
            );
        }

        // Last month (Dec 2024) absorbs the remainder: 3100 - 11 * 258 = 262.
        let expected_last = 3100 - 11 * monthly_base; // 262
        assert_eq!(
            rows[11].0, expected_last,
            "Last month must absorb the remainder ({})",
            expected_last
        );

        // Overall total must equal purchase_price_minor.
        let total: i64 = rows.iter().map(|r| r.0).sum();
        assert_eq!(
            total, 3100,
            "Sum of all depreciation amounts must equal purchase_price_minor"
        );
    }

    fn kona_terms() -> AssetUseTerms {
        AssetUseTerms {
            purchase_vat: Some(PurchaseVat {
                gross_minor: 1_560_000,
                vat_rate_bps: 2300,
                vat_reclaimable_pct_bps: 5000,
            }),
            business_use_pct_bps: Some(8000),
            business_use_changes: Vec::new(),
        }
    }

    fn mk_kona(conn: &Connection) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: "Hyundai Kona".to_owned(),
                currency_id: 1,
                account_type: "asset".to_owned(),
                person_id: Some(default_person_id()),
                purchase_price_minor: Some(1_268_292),
                purchase_date: Some("2026-07-12".to_owned()),
                depreciation_period_months: Some(24),
                use_terms: kona_terms(),
                ..Default::default()
            },
        )
        .expect("create_account failed")
    }

    fn entries_of_kind(
        conn: &Connection,
        asset_id: i64,
        asset_entry: &str,
    ) -> Vec<(i64, String, i64)> {
        let mut stmt = conn
            .prepare(
                "SELECT ed.amount_minor, ed.event_date, ed.expense_deductible_pct_bps
                 FROM event e
                 JOIN event_data ed ON ed.id = e.latest_data_id
                 WHERE e.linked_asset_id = ?1
                   AND e.asset_entry = ?2
                   AND e.deleted_at IS NULL
                 ORDER BY ed.event_date ASC",
            )
            .unwrap();
        stmt.query_map(params![asset_id, asset_entry], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
    }

    #[test]
    fn a_mixed_asset_keeps_both_entries_of_its_purchase_month_over_two_runs() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_kona(&conn);

        generate_depreciation_events(&conn).expect("first run failed");
        generate_depreciation_events(&conn).expect("second run failed");

        let depreciation = entries_of_kind(&conn, asset_id, "depreciation");
        assert_eq!(depreciation.len(), 24);
        assert_eq!(depreciation[0].0, 52_846);
        assert!(depreciation[0].1.starts_with("2026-07"));
        assert_eq!(depreciation[0].2, 8000);
        assert_eq!(depreciation.iter().map(|row| row.0).sum::<i64>(), 1_268_292);

        assert_eq!(
            entries_of_kind(&conn, asset_id, "lost_vat"),
            vec![(145_854, "2026-07-12T00:00:00".to_owned(), 0)]
        );
    }

    #[test]
    fn a_share_change_splits_the_schedule_at_its_month() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_depreciating_asset(&conn, "Peugeot", 2_040_000, "2024-07-01", 48);
        store_use_terms(
            &conn,
            asset_id,
            &AssetUseTerms {
                business_use_changes: vec![BusinessUseChange {
                    from_month: "2026-07".to_owned(),
                    business_use_pct_bps: 8000,
                }],
                ..Default::default()
            },
        )
        .unwrap();

        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        let rows = entries_of_kind(&conn, asset_id, "depreciation");
        let share_in = |month: &str| {
            rows.iter()
                .find(|row| row.1.starts_with(month))
                .map(|row| row.2)
        };
        assert_eq!(share_in("2024-07"), Some(10000));
        assert_eq!(share_in("2026-06"), Some(10000));
        assert_eq!(share_in("2026-07"), Some(8000));
        assert_eq!(share_in("2028-06"), Some(8000));
    }

    #[test]
    fn an_edit_keeps_the_shares_and_rebuilds_the_lost_vat_entry() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_kona(&conn);
        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        let mut terms = kona_terms();
        terms.purchase_vat = Some(PurchaseVat {
            gross_minor: 1_230_000,
            vat_rate_bps: 2300,
            vat_reclaimable_pct_bps: 5000,
        });
        store_use_terms(&conn, asset_id, &terms).unwrap();
        conn.execute(
            "UPDATE account SET purchase_price_minor = 1000000 WHERE id = ?1",
            params![asset_id],
        )
        .unwrap();
        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        assert_eq!(
            entries_of_kind(&conn, asset_id, "lost_vat"),
            vec![(115_000, "2026-07-12T00:00:00".to_owned(), 0)]
        );
        let depreciation = entries_of_kind(&conn, asset_id, "depreciation");
        assert_eq!(depreciation.iter().map(|row| row.0).sum::<i64>(), 1_000_000);
        assert!(depreciation.iter().all(|row| row.2 == 8000));
    }

    #[test]
    fn lowering_the_share_at_purchase_to_full_use_removes_the_lost_vat_entry() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_kona(&conn);
        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        let mut terms = kona_terms();
        terms.business_use_pct_bps = Some(10000);
        store_use_terms(&conn, asset_id, &terms).unwrap();
        recalculate_depreciation_for_asset(&conn, asset_id).expect("recalculate failed");

        assert!(entries_of_kind(&conn, asset_id, "lost_vat").is_empty());
    }

    #[test]
    fn an_instant_write_off_takes_the_share_at_purchase() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Dashcam".to_owned(),
                currency_id: 1,
                account_type: "asset".to_owned(),
                person_id: Some(default_person_id()),
                purchase_price_minor: Some(100_000),
                purchase_date: Some("2026-07-12".to_owned()),
                use_terms: AssetUseTerms {
                    purchase_vat: Some(PurchaseVat {
                        gross_minor: 123_000,
                        vat_rate_bps: 2300,
                        vat_reclaimable_pct_bps: 5000,
                    }),
                    business_use_pct_bps: Some(8000),
                    business_use_changes: Vec::new(),
                },
                ..Default::default()
            },
        )
        .expect("create_account failed");

        generate_depreciation_events(&conn).expect("first run failed");
        generate_depreciation_events(&conn).expect("second run failed");

        assert_eq!(
            entries_of_kind(&conn, asset_id, "depreciation"),
            vec![(100_000, "2026-07-12T00:00:00".to_owned(), 8000)]
        );
        assert_eq!(entries_of_kind(&conn, asset_id, "lost_vat").len(), 1);
    }

    #[test]
    fn an_asset_without_terms_keeps_its_full_share_and_gets_no_lost_vat_entry() {
        let conn = initialize_in_memory().expect("DB init failed");
        let asset_id = mk_depreciating_asset(&conn, "Laptop", 120_000, "2026-01-01", 12);

        generate_depreciation_events(&conn).expect("generator failed");

        let rows = entries_of_kind(&conn, asset_id, "depreciation");
        assert_eq!(rows.len(), 12);
        assert!(rows.iter().all(|row| row.2 == 10000));
        assert!(entries_of_kind(&conn, asset_id, "lost_vat").is_empty());
    }
}
