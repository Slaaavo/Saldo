use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::shared::{div_round, with_savepoint_app};

use super::taxable_events::insert_taxable_event_data;

// ---------------------------------------------------------------------------
// Public context struct
// ---------------------------------------------------------------------------

pub(crate) struct PrepaidExpenseContext {
    pub parent_event_id: i64,
    pub account_id: i64,
    pub expense_date: String,
    pub prepaid_until: String,
    pub total_amount_minor: i64,
    pub expense_deductible_pct_bps: Option<i64>,
    pub note: Option<String>,
}

// ---------------------------------------------------------------------------
// Segment struct
// ---------------------------------------------------------------------------

pub(crate) struct PrepaidSegment {
    /// ISO datetime of the first day of the segment: `"YYYY-MM-DDT00:00:00"`.
    pub event_date: String,
    pub amount_minor: i64,
    pub year: i32,
}

// ---------------------------------------------------------------------------
// Pure segment-calculation function
// ---------------------------------------------------------------------------

/// Split a prepaid expense date range into per-calendar-year segments.
///
/// - `expense_date`: `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS` — only the date portion is used.
/// - `prepaid_until`: `YYYY-MM-DD` — the last covered day (inclusive).
/// - `total_amount_minor`: total amount to distribute across segments.
///
/// All segments except the last receive `div_round(total * segment_days, total_days)`.
/// The last segment receives the remainder, guaranteeing `sum(amounts) == total_amount_minor`.
pub(crate) fn calculate_prepaid_segments(
    expense_date: &str,
    prepaid_until: &str,
    total_amount_minor: i64,
) -> Vec<PrepaidSegment> {
    let start =
        NaiveDate::parse_from_str(&expense_date[..10], "%Y-%m-%d").expect("invalid expense_date");
    let end =
        NaiveDate::parse_from_str(&prepaid_until[..10], "%Y-%m-%d").expect("invalid prepaid_until");

    // Inclusive day count.
    let total_days = (end - start).num_days() + 1;

    let start_year = start.year();
    let end_year = end.year();

    let mut segments = Vec::new();
    let mut sum_so_far: i64 = 0;

    for year in start_year..=end_year {
        let seg_start = if year == start_year {
            start
        } else {
            NaiveDate::from_ymd_opt(year, 1, 1).unwrap()
        };

        let seg_end = if year == end_year {
            end
        } else {
            NaiveDate::from_ymd_opt(year, 12, 31).unwrap()
        };

        let segment_days = (seg_end - seg_start).num_days() + 1; // inclusive

        let is_last = year == end_year;
        let amount = if is_last {
            total_amount_minor - sum_so_far
        } else {
            div_round(total_amount_minor * segment_days, total_days)
        };

        sum_so_far += amount;

        let event_date = format!("{}T00:00:00", seg_start.format("%Y-%m-%d"));

        segments.push(PrepaidSegment {
            event_date,
            amount_minor: amount,
            year,
        });
    }

    segments
}

// ---------------------------------------------------------------------------
// Database operations
// ---------------------------------------------------------------------------

/// Insert system-generated yearly child events for a prepaid expense.
pub(crate) fn generate_prepaid_child_events(
    conn: &Connection,
    ctx: &PrepaidExpenseContext,
) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        let segments = calculate_prepaid_segments(
            &ctx.expense_date,
            &ctx.prepaid_until,
            ctx.total_amount_minor,
        );

        for segment in &segments {
            conn.execute(
                "INSERT INTO event (account_id, event_type, linked_prepaid_event_id, is_system_generated)
                 VALUES (?1, 'expense', ?2, 1)",
                params![ctx.account_id, ctx.parent_event_id],
            )
            .map_err(AppError::from)?;
            let event_id = conn.last_insert_rowid();

            let note = match &ctx.note {
                Some(n) => format!("Prepaid: {} ({})", n, segment.year),
                None => format!("Prepaid ({})", segment.year),
            };

            insert_taxable_event_data(
                conn,
                event_id,
                segment.amount_minor,
                &segment.event_date,
                Some(note.as_str()),
                None,
                None,
                ctx.expense_deductible_pct_bps,
                None,
                None,
            )
            .map_err(AppError::from)?;
        }

        Ok(())
    })
}

/// Hard-delete all system-generated child events for the given parent prepaid event.
pub(crate) fn delete_prepaid_child_events(
    conn: &Connection,
    parent_event_id: i64,
) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        // 1. Clean up taxable_cashflow_link rows referencing the children (both sides),
        //    before deleting the referenced event rows.
        conn.execute(
            "DELETE FROM taxable_cashflow_link
             WHERE taxable_event_id IN (
                       SELECT id FROM event
                       WHERE linked_prepaid_event_id = ?1 AND is_system_generated = 1
                   )
                OR cashflow_event_id IN (
                       SELECT id FROM event
                       WHERE linked_prepaid_event_id = ?1 AND is_system_generated = 1
                   )",
            params![parent_event_id],
        )
        .map_err(AppError::from)?;

        // 2. Delete event_data rows for the children.
        conn.execute(
            "DELETE FROM event_data WHERE event_id IN (
                 SELECT id FROM event
                 WHERE linked_prepaid_event_id = ?1 AND is_system_generated = 1
             )",
            params![parent_event_id],
        )
        .map_err(AppError::from)?;

        // 3. Hard-delete the child event rows.
        conn.execute(
            "DELETE FROM event
             WHERE linked_prepaid_event_id = ?1 AND is_system_generated = 1",
            params![parent_event_id],
        )
        .map_err(AppError::from)?;

        Ok(())
    })
}

/// Delete existing child events and regenerate them from the updated context.
#[allow(dead_code)]
pub(crate) fn regenerate_prepaid_child_events(
    conn: &Connection,
    ctx: &PrepaidExpenseContext,
) -> Result<(), AppError> {
    delete_prepaid_child_events(conn, ctx.parent_event_id)?;
    generate_prepaid_child_events(conn, ctx)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sum_amounts(segments: &[PrepaidSegment]) -> i64 {
        segments.iter().map(|s| s.amount_minor).sum()
    }

    #[test]
    fn test_two_year_span() {
        // 2024-07-01 to 2025-12-31
        // Segment 1: 2024-07-01..2024-12-31 = 184 days
        // Segment 2: 2025-01-01..2025-12-31 = 365 days
        // Total: 549 days
        let segs = calculate_prepaid_segments("2024-07-01", "2025-12-31", 10_000);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].year, 2024);
        assert_eq!(segs[0].event_date, "2024-07-01T00:00:00");
        assert_eq!(segs[1].year, 2025);
        assert_eq!(segs[1].event_date, "2025-01-01T00:00:00");
        assert_eq!(sum_amounts(&segs), 10_000);
    }

    #[test]
    fn test_three_year_span() {
        // 2024-06-01 to 2026-05-31
        // Segment 1: 2024-06-01..2024-12-31 = 214 days
        // Segment 2: 2025-01-01..2025-12-31 = 365 days
        // Segment 3: 2026-01-01..2026-05-31 = 151 days
        // Total: 730 days
        let segs = calculate_prepaid_segments("2024-06-01", "2026-05-31", 10_000);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].year, 2024);
        assert_eq!(segs[0].event_date, "2024-06-01T00:00:00");
        assert_eq!(segs[1].year, 2025);
        assert_eq!(segs[1].event_date, "2025-01-01T00:00:00");
        assert_eq!(segs[2].year, 2026);
        assert_eq!(segs[2].event_date, "2026-01-01T00:00:00");
        assert_eq!(sum_amounts(&segs), 10_000);
    }

    #[test]
    fn test_leap_year_crossing() {
        // 2023-11-01 to 2024-03-31
        // Segment 1: 2023-11-01..2023-12-31 = 61 days (Nov: 30, Dec: 31)
        // Segment 2: 2024-01-01..2024-03-31 = 91 days (Jan: 31, Feb: 29, Mar: 31)
        // Total: 152 days — Feb 29 2024 is real.
        let segs = calculate_prepaid_segments("2023-11-01", "2024-03-31", 10_000);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].year, 2023);
        assert_eq!(segs[0].event_date, "2023-11-01T00:00:00");
        assert_eq!(segs[1].year, 2024);
        assert_eq!(segs[1].event_date, "2024-01-01T00:00:00");
        assert_eq!(sum_amounts(&segs), 10_000);
    }

    #[test]
    fn test_exact_year_boundary() {
        // 2024-01-01 to 2024-12-31 → single segment, entire amount unchanged.
        let segs = calculate_prepaid_segments("2024-01-01", "2024-12-31", 10_000);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].year, 2024);
        assert_eq!(segs[0].event_date, "2024-01-01T00:00:00");
        assert_eq!(segs[0].amount_minor, 10_000);
        assert_eq!(sum_amounts(&segs), 10_000);
    }

    #[test]
    fn test_datetime_format_expense_date() {
        // expense_date supplied as full datetime string — only date portion is used.
        let segs = calculate_prepaid_segments("2024-07-01T14:30:00", "2025-12-31", 10_000);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].event_date, "2024-07-01T00:00:00");
        assert_eq!(sum_amounts(&segs), 10_000);
    }

    // -----------------------------------------------------------------------
    // Integration tests: regenerate_prepaid_child_events
    // -----------------------------------------------------------------------

    use rusqlite::{params, Connection};

    fn setup_test_db() -> (Connection, i64) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE currency (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 code TEXT NOT NULL UNIQUE,
                 name TEXT NOT NULL,
                 minor_units INTEGER NOT NULL DEFAULT 2
             );
             CREATE TABLE account (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL,
                 currency_id INTEGER NOT NULL REFERENCES currency(id)
             );
             CREATE TABLE event (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 account_id INTEGER NOT NULL REFERENCES account(id),
                 event_type TEXT NOT NULL,
                 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
                 deleted_at TEXT NULL,
                 latest_data_id INTEGER NULL,
                 is_system_generated INTEGER NOT NULL DEFAULT 0,
                 linked_prepaid_event_id INTEGER DEFAULT NULL REFERENCES event(id),
                 linked_asset_id INTEGER DEFAULT NULL
             );
             CREATE TABLE event_data (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 event_id INTEGER NOT NULL REFERENCES event(id),
                 amount_minor INTEGER NOT NULL,
                 event_date TEXT NOT NULL,
                 note TEXT NULL,
                 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now')),
                 vat_rate_bps INTEGER NULL,
                 vat_reclaimable_pct_bps INTEGER NULL,
                 expense_deductible_pct_bps INTEGER NULL,
                 prepaid_until TEXT NULL,
                 reclaimed_vat INTEGER NULL
             );
             CREATE TABLE taxable_cashflow_link (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 taxable_event_id INTEGER NOT NULL REFERENCES event(id),
                 cashflow_event_id INTEGER NOT NULL REFERENCES event(id),
                 UNIQUE (cashflow_event_id)
             );
             CREATE TRIGGER trg_eventdata_after_insert
             AFTER INSERT ON event_data
             BEGIN
                 UPDATE event SET latest_data_id = NEW.id WHERE id = NEW.event_id;
             END;
             INSERT INTO currency (code, name, minor_units) VALUES ('EUR', 'Euro', 2);
             INSERT INTO account (name, currency_id) VALUES ('Test Account', 1);",
        )
        .unwrap();
        let account_id: i64 = conn
            .query_row(
                "SELECT id FROM account WHERE name = 'Test Account'",
                params![],
                |row| row.get(0),
            )
            .unwrap();
        (conn, account_id)
    }

    #[test]
    fn test_regenerate_updates_amounts() {
        let (conn, account_id) = setup_test_db();

        // Insert parent prepaid_expense event.
        conn.execute(
            "INSERT INTO event (account_id, event_type) VALUES (?1, 'prepaid_expense')",
            params![account_id],
        )
        .unwrap();
        let parent_event_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO event_data (event_id, amount_minor, event_date) VALUES (?1, ?2, ?3)",
            params![parent_event_id, 10_000i64, "2024-01-01T00:00:00"],
        )
        .unwrap();

        // Generate initial children: 2024-01-01 → 2025-12-31 (2 yearly segments).
        let mut ctx = PrepaidExpenseContext {
            parent_event_id,
            account_id,
            expense_date: "2024-01-01".to_string(),
            prepaid_until: "2025-12-31".to_string(),
            total_amount_minor: 10_000,
            expense_deductible_pct_bps: None,
            note: None,
        };
        generate_prepaid_child_events(&conn, &ctx).unwrap();

        let initial_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event
                 WHERE linked_prepaid_event_id = ?1 AND is_system_generated = 1",
                params![parent_event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(initial_count, 2);

        // Regenerate with a new total — old children must be gone.
        ctx.total_amount_minor = 20_000;
        regenerate_prepaid_child_events(&conn, &ctx).unwrap();

        // Exactly 2 children still (old ones deleted, new ones inserted).
        let new_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event
                 WHERE linked_prepaid_event_id = ?1 AND is_system_generated = 1",
                params![parent_event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(new_count, 2);

        // New child amounts must sum to the updated total.
        let new_sum: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(ed.amount_minor), 0)
                 FROM event e
                 JOIN event_data ed ON ed.event_id = e.id
                 WHERE e.linked_prepaid_event_id = ?1 AND e.is_system_generated = 1",
                params![parent_event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(new_sum, 20_000);
    }

    #[test]
    fn test_regenerate_updates_segments_on_date_change() {
        let (conn, account_id) = setup_test_db();

        // Insert parent prepaid_expense event.
        conn.execute(
            "INSERT INTO event (account_id, event_type) VALUES (?1, 'prepaid_expense')",
            params![account_id],
        )
        .unwrap();
        let parent_event_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO event_data (event_id, amount_minor, event_date) VALUES (?1, ?2, ?3)",
            params![parent_event_id, 30_000i64, "2024-01-01T00:00:00"],
        )
        .unwrap();

        // Generate initial children: 2024-01-01 → 2025-12-31 (2 yearly segments).
        let mut ctx = PrepaidExpenseContext {
            parent_event_id,
            account_id,
            expense_date: "2024-01-01".to_string(),
            prepaid_until: "2025-12-31".to_string(),
            total_amount_minor: 30_000,
            expense_deductible_pct_bps: None,
            note: None,
        };
        generate_prepaid_child_events(&conn, &ctx).unwrap();

        let initial_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event
                 WHERE linked_prepaid_event_id = ?1 AND is_system_generated = 1",
                params![parent_event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(initial_count, 2);

        // Regenerate with a 3-year span (one extra year added).
        ctx.prepaid_until = "2026-12-31".to_string();
        regenerate_prepaid_child_events(&conn, &ctx).unwrap();

        // Exactly 3 child events now (2024, 2025, 2026).
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event
                 WHERE linked_prepaid_event_id = ?1 AND is_system_generated = 1",
                params![parent_event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);

        // Sum of new child amounts must equal total.
        let sum: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(ed.amount_minor), 0)
                 FROM event e
                 JOIN event_data ed ON ed.event_id = e.id
                 WHERE e.linked_prepaid_event_id = ?1 AND e.is_system_generated = 1",
                params![parent_event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sum, 30_000);
    }

    #[test]
    fn test_delete_prepaid_child_events() {
        let (conn, account_id) = setup_test_db();

        // Insert parent prepaid_expense event.
        conn.execute(
            "INSERT INTO event (account_id, event_type) VALUES (?1, 'prepaid_expense')",
            params![account_id],
        )
        .unwrap();
        let parent_event_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO event_data (event_id, amount_minor, event_date) VALUES (?1, ?2, ?3)",
            params![parent_event_id, 12_000i64, "2024-01-01T00:00:00"],
        )
        .unwrap();

        // Generate children: 2024-01-01 → 2025-12-31 (2 yearly segments).
        let ctx = PrepaidExpenseContext {
            parent_event_id,
            account_id,
            expense_date: "2024-01-01".to_string(),
            prepaid_until: "2025-12-31".to_string(),
            total_amount_minor: 12_000,
            expense_deductible_pct_bps: None,
            note: None,
        };
        generate_prepaid_child_events(&conn, &ctx).unwrap();

        // Verify children were created.
        let child_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event WHERE linked_prepaid_event_id = ?1",
                params![parent_event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(child_count, 2);

        // Grab first child id to insert a taxable_cashflow_link.
        let first_child_id: i64 = conn
            .query_row(
                "SELECT id FROM event WHERE linked_prepaid_event_id = ?1 ORDER BY id LIMIT 1",
                params![parent_event_id],
                |row| row.get(0),
            )
            .unwrap();

        // Insert a taxable_cashflow_link referencing the child as the taxable side.
        conn.execute(
            "INSERT INTO taxable_cashflow_link (taxable_event_id, cashflow_event_id) VALUES (?1, ?1)",
            params![first_child_id],
        )
        .unwrap();

        // Delete the children.
        delete_prepaid_child_events(&conn, parent_event_id).unwrap();

        // Child event rows must NOT exist.
        let remaining_children: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event WHERE linked_prepaid_event_id = ?1",
                params![parent_event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            remaining_children, 0,
            "child event rows should be hard-deleted"
        );

        // Child event_data rows must NOT exist.
        let remaining_event_data: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event_data WHERE event_id = ?1",
                params![first_child_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            remaining_event_data, 0,
            "child event_data rows should be deleted"
        );

        // taxable_cashflow_link rows for children must NOT exist.
        let remaining_links: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM taxable_cashflow_link WHERE taxable_event_id = ?1",
                params![first_child_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            remaining_links, 0,
            "taxable_cashflow_link rows for children should be deleted"
        );

        // Parent event must still exist.
        let parent_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event WHERE id = ?1",
                params![parent_event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            parent_count, 1,
            "parent event should still exist after deleting children"
        );
    }
}
