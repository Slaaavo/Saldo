mod balance_updates;
mod cashflows;
mod events;
mod queries;
mod snapshot;
mod split_groups;
mod taxable_events;
pub mod taxable_links;

pub(crate) use balance_updates::create_balance_update_inner;
pub use balance_updates::{
    bulk_create_balance_updates, create_balance_update, BulkCreateBalanceUpdatesParams,
    CreateBalanceUpdateParams,
};

pub use cashflows::{bulk_create_cashflows, create_cashflow, create_transfer, CashflowEntry};

pub use events::{
    delete_event, update_event, update_transfer, UpdateEventParams, UpdateTransferParams,
};

pub use split_groups::{
    check_event_split_group_date_conflict, create_split_group_with_legs, update_split_group_date,
    CheckEventSplitGroupDateConflictParams, CreateSplitGroupWithLegsParams,
    UpdateSplitGroupDateParams,
};

pub use queries::{get_event_by_id, list_events, ListEventsQuery};

pub use snapshot::{get_accounts_snapshot, GetSnapshotParams};

pub use taxable_events::{
    create_taxable_event, create_taxable_split_group_with_legs, delete_split_group,
    update_taxable_event, update_taxable_split_group, CreateTaxableEventParams,
    CreateTaxableSplitGroupWithLegsParams, NewSplitLeg, TaxableEventLeg, UpdateTaxableEventParams,
    UpdateTaxableSplitGroupParams, UpdatedSplitLeg,
};

pub use taxable_links::{
    link_cashflows_to_taxable, list_eligible_cashflows, list_linked_cashflows,
    unlink_cashflow_from_taxable, EligibleCashflowsParams, LinkCashflowsParams,
};

// ---------------------------------------------------------------------------
// Shared EventWithData query infrastructure
// Used by queries.rs and taxable_links.rs to avoid duplicating the 28-column
// SELECT list, JOIN block, and row mapper.
// ---------------------------------------------------------------------------

pub(super) const EVENT_SELECT: &str = "
          e.id,
          e.account_id,
          CASE WHEN a.account_type IN ('default_revenue', 'default_expense')
               THEN (SELECT p.name FROM person p WHERE p.id = a.person_id)
               ELSE a.name END AS account_name,
          a.account_type,
          e.event_type,
          ed.event_date,
          ed.amount_minor,
          ed.note,
          e.created_at,
          c.code AS currency_code,
          c.minor_units AS currency_minor_units,
          e.linked_event_id,
          ed.counterpart_account_id,
          ed.bucket_id,
          ed.original_currency_id,
          ed.original_amount_minor,
          ed.fx_rate_mantissa,
          ed.fx_rate_exponent,
          counter_a.name AS counterpart_account_name,
          bucket_a.name AS bucket_name,
          orig_c.code AS original_currency_code,
          orig_c.minor_units AS original_currency_minor_units,
          e.split_group_id,
          sg.note AS split_group_note,
          ed.vat_rate_bps,
          ed.vat_deductible_pct_bps,
          ed.expense_deductible_pct_bps,
          ed.prepaid_period_months";

pub(super) const EVENT_JOINS: &str = "
        JOIN account a ON a.id = e.account_id
        JOIN currency c ON c.id = a.currency_id
        JOIN event_data ed ON ed.id = e.latest_data_id
        LEFT JOIN account counter_a ON counter_a.id = ed.counterpart_account_id
        LEFT JOIN account bucket_a ON bucket_a.id = ed.bucket_id
        LEFT JOIN currency orig_c ON orig_c.id = ed.original_currency_id
        LEFT JOIN split_group sg ON sg.id = e.split_group_id";

pub(super) fn map_event_row(
    row: &rusqlite::Row,
) -> rusqlite::Result<crate::features::transactions::models::EventWithData> {
    use crate::features::transactions::models::EventWithData;
    Ok(EventWithData {
        id: row.get(0)?,
        account_id: row.get(1)?,
        account_name: row.get(2)?,
        account_type: row.get(3)?,
        event_type: row.get(4)?,
        event_date: row.get(5)?,
        amount_minor: row.get(6)?,
        note: row.get(7)?,
        created_at: row.get(8)?,
        currency_code: row.get(9)?,
        currency_minor_units: row.get(10)?,
        linked_event_id: row.get(11)?,
        counterpart_account_id: row.get(12)?,
        bucket_id: row.get(13)?,
        original_currency_id: row.get(14)?,
        original_amount_minor: row.get(15)?,
        fx_rate_mantissa: row.get(16)?,
        fx_rate_exponent: row.get(17)?,
        counterpart_account_name: row.get(18)?,
        bucket_name: row.get(19)?,
        original_currency_code: row.get(20)?,
        original_currency_minor_units: row.get(21)?,
        split_group_id: row.get(22)?,
        split_group_note: row.get(23)?,
        vat_rate_bps: row.get(24)?,
        vat_deductible_pct_bps: row.get(25)?,
        expense_deductible_pct_bps: row.get(26)?,
        prepaid_period_months: row.get(27)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::currency::repository::{set_fx_rate_manual, SetFxRateManualParams};
    use rusqlite::{params, Connection};

    fn mk_account(conn: &Connection) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: "Test Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    #[test]
    fn snapshot_with_no_events_returns_zero() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].account_id, account_id);
        assert_eq!(snapshot[0].balance_minor, 0);
    }

    #[test]
    fn snapshot_reflects_balance_update() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 5000,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-03-01T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot[0].balance_minor, 5000);
    }

    #[test]
    fn snapshot_ignores_future_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 5000,
                event_date: "2026-06-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-03-01T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot[0].balance_minor, 0);
    }

    #[test]
    fn snapshot_uses_latest_event_by_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 3000,
                event_date: "2026-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 7000,
                event_date: "2026-02-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-03-01T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot[0].balance_minor, 7000);
    }

    #[test]
    fn snapshot_ignores_soft_deleted_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let event_id = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 5000,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        delete_event(&conn, event_id).unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-03-01T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot[0].balance_minor, 0);
    }

    #[test]
    fn update_event_creates_new_data_row() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let event_id = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 5000,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        update_event(
            &conn,
            UpdateEventParams {
                event_id,
                amount_minor: 9999,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-03-01T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot[0].balance_minor, 9999);
    }

    #[test]
    fn update_event_rejects_deleted_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let event_id = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 5000,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        delete_event(&conn, event_id).unwrap();
        let result = update_event(
            &conn,
            UpdateEventParams {
                event_id,
                amount_minor: 9999,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Cannot update a deleted event"));
    }

    #[test]
    fn update_event_rejects_nonexistent_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let result = update_event(
            &conn,
            UpdateEventParams {
                event_id: 999,
                amount_minor: 9999,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Event not found"));
    }

    #[test]
    fn list_events_returns_all_non_deleted() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 1000,
                event_date: "2026-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 2000,
                event_date: "2026-02-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let result = list_events(&conn, ListEventsQuery::default()).unwrap();
        assert_eq!(result.events.len(), 2);
    }

    #[test]
    fn list_events_filters_by_account() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(
            &conn,
            CreateAccountParams {
                name: "Second".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: acc1,
                amount_minor: 1000,
                event_date: "2026-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: acc2,
                amount_minor: 2000,
                event_date: "2026-02-01".to_owned(),
                note: None,
            },
        )
        .unwrap();

        let result_acc1 = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(acc1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result_acc1.events.len(), 1);
        assert_eq!(result_acc1.events[0].account_id, acc1);

        let result_acc2 = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(acc2),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result_acc2.events.len(), 1);
        assert_eq!(result_acc2.events[0].account_id, acc2);
    }

    #[test]
    fn list_events_filters_by_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 1000,
                event_date: "2026-01-15".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 2000,
                event_date: "2026-03-15".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let result = list_events(
            &conn,
            ListEventsQuery {
                before_date: Some("2026-02-01T23:59:59".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].amount_minor, 1000);
    }

    #[test]
    fn create_bucket_appears_in_snapshot() {
        let conn = initialize_in_memory().expect("DB init failed");
        let bucket_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Emergency Fund".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                initial_balance_minor: Some(20000),
                ..Default::default()
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let bucket = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();
        assert_eq!(bucket.account_type, "bucket");
        assert_eq!(bucket.balance_minor, 20000);
    }

    #[test]
    fn snapshot_returns_account_type() {
        let conn = initialize_in_memory().expect("DB init failed");
        mk_account(&conn);
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].account_type, "account");
    }

    #[test]
    fn bucket_balance_update_works() {
        let conn = initialize_in_memory().expect("DB init failed");
        let bucket_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Savings Bucket".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: bucket_id,
                amount_minor: 15000,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-03-01T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let bucket = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();
        assert_eq!(bucket.balance_minor, 15000);
    }

    #[test]
    fn list_events_includes_account_type() {
        let conn = initialize_in_memory().expect("DB init failed");
        let bucket_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Test Bucket".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: bucket_id,
                amount_minor: 5000,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let result = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(bucket_id),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].account_type, "bucket");

        let empty_account_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Empty Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let result_empty = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(empty_account_id),
                ..Default::default()
            },
        )
        .unwrap();
        // Account with no balance updates has no events
        assert_eq!(result_empty.events.len(), 0);
    }

    #[test]
    fn snapshot_orders_accounts_before_buckets() {
        let conn = initialize_in_memory().expect("DB init failed");
        create_account(
            &conn,
            CreateAccountParams {
                name: "Zebra Bucket".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_account(
            &conn,
            CreateAccountParams {
                name: "Alpha Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        // accounts come first (alphabetically 'account' < 'bucket'), then buckets
        let types: Vec<&str> = snapshot.iter().map(|r| r.account_type.as_str()).collect();
        let first_bucket_idx = types.iter().position(|t| *t == "bucket");
        let last_account_idx = types.iter().rposition(|t| *t == "account");
        if let (Some(fb), Some(la)) = (first_bucket_idx, last_account_idx) {
            assert!(la < fb, "All accounts should come before all buckets");
        }
    }

    #[test]
    fn snapshot_includes_currency_fields() {
        let conn = initialize_in_memory().expect("DB init failed");
        mk_account(&conn);
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot[0].currency_code, "EUR");
        assert_eq!(snapshot[0].currency_minor_units, 2);
        // EUR is the consolidation currency so converted == balance and no rate missing
        assert_eq!(
            snapshot[0].converted_balance_minor,
            snapshot[0].balance_minor
        );
        assert!(!snapshot[0].fx_rate_missing);
    }

    #[test]
    fn snapshot_foreign_currency_no_rate_uses_1_to_1_fallback() {
        let conn = initialize_in_memory().expect("DB init failed");
        // USD is seeded in migration 004
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let acc = create_account(
            &conn,
            CreateAccountParams {
                name: "USD Account".to_owned(),
                currency_id: usd,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: acc,
                amount_minor: 108420,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-03-01T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let row = snapshot.iter().find(|r| r.account_id == acc).unwrap();
        assert_eq!(row.balance_minor, 108420);
        // No FX rate → 1:1 fallback → converted = same value (minor_units both 2)
        assert_eq!(row.converted_balance_minor, 108420);
        assert!(row.fx_rate_missing);
    }

    #[test]
    fn snapshot_foreign_currency_with_rate_converts_correctly() {
        let conn = initialize_in_memory().expect("DB init failed");
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let acc = create_account(
            &conn,
            CreateAccountParams {
                name: "USD Account".to_owned(),
                currency_id: usd,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: acc,
                amount_minor: 108420,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        // Store rate: 1 EUR = 1.0842 USD (mantissa=10842, exponent=-4)
        set_fx_rate_manual(
            &conn,
            SetFxRateManualParams {
                from_currency_id: eur,
                to_currency_id: usd,
                date: "2026-03-01".to_owned(),
                rate_mantissa: 10842,
                rate_exponent: -4,
                is_direct: false,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-03-01T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let row = snapshot.iter().find(|r| r.account_id == acc).unwrap();
        assert_eq!(row.balance_minor, 108420);
        assert_eq!(row.converted_balance_minor, 100000); // 1084.20 USD → 1000.00 EUR
        assert!(!row.fx_rate_missing);
    }

    #[test]
    fn list_events_includes_currency_fields() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 5000,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let result = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(account_id),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.events[0].currency_code, "EUR");
        assert_eq!(result.events[0].currency_minor_units, 2);
    }

    #[test]
    fn list_events_returns_correct_total_count_with_limit() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 1000,
                event_date: "2026-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 2000,
                event_date: "2026-02-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 3000,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 4000,
                event_date: "2026-04-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 5000,
                event_date: "2026-05-01".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let result = list_events(
            &conn,
            ListEventsQuery {
                limit: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.total_count, 5);
    }

    #[test]
    fn snapshot_cashflow_after_balance_update_is_summed() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 5000,
                event_date: "2026-03-01T10:00:00".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id,
                amount_minor: 200,
                event_date: "2026-03-01T14:30:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-03-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot[0].balance_minor, 5200);
    }

    #[test]
    fn snapshot_cashflow_before_balance_update_not_counted() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id,
                amount_minor: 500,
                event_date: "2026-01-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 5000,
                event_date: "2026-03-01T00:00:00".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        // The cashflow at 2026-01-01 is before the anchor; only the anchor balance counts.
        assert_eq!(snapshot[0].balance_minor, 5000);
    }

    #[test]
    fn snapshot_with_no_balance_update_sums_all_cashflows() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id,
                amount_minor: 1000,
                event_date: "2026-01-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id,
                amount_minor: 500,
                event_date: "2026-02-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot[0].balance_minor, 1500);
    }

    #[test]
    fn create_transfer_creates_linked_pair() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(
            &conn,
            CreateAccountParams {
                name: "Second Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let (source_id, counterpart_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -10000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc2),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let source_linked: Option<i64> = conn
            .query_row(
                "SELECT linked_event_id FROM event WHERE id = ?1",
                params![source_id],
                |r| r.get(0),
            )
            .unwrap();
        let counterpart_linked: Option<i64> = conn
            .query_row(
                "SELECT linked_event_id FROM event WHERE id = ?1",
                params![counterpart_id],
                |r| r.get(0),
            )
            .unwrap();

        assert_eq!(source_linked, Some(counterpart_id));
        assert_eq!(counterpart_linked, Some(source_id));

        let snapshot1 = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let row1 = snapshot1.iter().find(|r| r.account_id == acc1).unwrap();
        let row2 = snapshot1.iter().find(|r| r.account_id == acc2).unwrap();
        assert_eq!(row1.balance_minor, -10000);
        assert_eq!(row2.balance_minor, 10000);
    }

    #[test]
    fn delete_transfer_also_deletes_counterpart() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(
            &conn,
            CreateAccountParams {
                name: "Second Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let (source_id, counterpart_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -10000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc2),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let deleted = delete_event(&conn, source_id).unwrap();
        assert_eq!(deleted.len(), 2);
        assert!(deleted.contains(&source_id));
        assert!(deleted.contains(&counterpart_id));

        let source_deleted_at: Option<String> = conn
            .query_row(
                "SELECT deleted_at FROM event WHERE id = ?1",
                params![source_id],
                |r| r.get(0),
            )
            .unwrap();
        let counterpart_deleted_at: Option<String> = conn
            .query_row(
                "SELECT deleted_at FROM event WHERE id = ?1",
                params![counterpart_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(source_deleted_at.is_some());
        assert!(counterpart_deleted_at.is_some());
    }

    #[test]
    fn snapshot_transfer_affects_both_accounts() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc_a = mk_account(&conn);
        let acc_b = create_account(
            &conn,
            CreateAccountParams {
                name: "Account B".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: acc_a,
                amount_minor: 10000,
                event_date: "2026-01-10T09:00:00".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: acc_b,
                amount_minor: 5000,
                event_date: "2026-01-10T09:00:00".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc_a,
                amount_minor: -3000,
                event_date: "2026-01-15T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc_b),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let row_a = snapshot.iter().find(|r| r.account_id == acc_a).unwrap();
        let row_b = snapshot.iter().find(|r| r.account_id == acc_b).unwrap();
        assert_eq!(row_a.balance_minor, 7000);
        assert_eq!(row_b.balance_minor, 8000);
    }

    #[test]
    fn delete_transfer_restores_snapshot_balances() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc_a = mk_account(&conn);
        let acc_b = create_account(
            &conn,
            CreateAccountParams {
                name: "Account B".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: acc_a,
                amount_minor: 10000,
                event_date: "2026-01-10T09:00:00".to_owned(),
                note: None,
            },
        )
        .unwrap();
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: acc_b,
                amount_minor: 5000,
                event_date: "2026-01-10T09:00:00".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let (source_id, _) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc_a,
                amount_minor: -3000,
                event_date: "2026-01-15T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc_b),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        delete_event(&conn, source_id).unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let row_a = snapshot.iter().find(|r| r.account_id == acc_a).unwrap();
        let row_b = snapshot.iter().find(|r| r.account_id == acc_b).unwrap();
        assert_eq!(row_a.balance_minor, 10000);
        assert_eq!(row_b.balance_minor, 5000);
    }

    #[test]
    fn snapshot_ignores_soft_deleted_cashflows() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 10000,
                event_date: "2026-03-01T10:00:00".to_owned(),
                note: None,
            },
        )
        .unwrap();
        let cashflow_id = create_cashflow(
            &conn,
            &CashflowEntry {
                account_id,
                amount_minor: -2000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
        delete_event(&conn, cashflow_id).unwrap();
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-03-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(snapshot[0].balance_minor, 10000);
    }

    #[test]
    fn update_transfer_same_currency() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(
            &conn,
            CreateAccountParams {
                name: "Second Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        let (from_id, to_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -5000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: Some("original note".to_string()),
                counterpart_account_id: Some(acc2),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        update_transfer(
            &conn,
            UpdateTransferParams {
                from_event_id: from_id,
                to_event_id: to_id,
                from_date: "2026-04-01T12:00:00".into(),
                to_date: "2026-04-01T12:00:00".into(),
                from_amount_minor: -8000,
                to_amount_minor: 8000,
                note: Some("updated note".into()),
                original_currency_id: None,
                original_amount_minor_for_from_leg: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let from_updated = get_event_by_id(&conn, from_id).unwrap().unwrap();
        assert_eq!(from_updated.amount_minor, -8000);
        assert_eq!(from_updated.event_date, "2026-04-01T12:00:00");
        assert_eq!(from_updated.note.as_deref(), Some("updated note"));
        assert_eq!(from_updated.counterpart_account_id, Some(acc2));

        let to_updated = get_event_by_id(&conn, to_id).unwrap().unwrap();
        assert_eq!(to_updated.amount_minor, 8000);
        assert_eq!(to_updated.event_date, "2026-04-01T12:00:00");
        assert_eq!(to_updated.note.as_deref(), Some("updated note"));

        let from_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event_data WHERE event_id = ?1",
                params![from_id],
                |r| r.get(0),
            )
            .unwrap();
        let to_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event_data WHERE event_id = ?1",
                params![to_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(from_count, 2);
        assert_eq!(to_count, 2);
    }

    #[test]
    fn update_transfer_cross_currency() {
        let conn = initialize_in_memory().expect("DB init failed");
        let usd_id: i64 = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get(0)
            })
            .unwrap();

        let acc_eur = mk_account(&conn);
        let acc_usd = create_account(
            &conn,
            CreateAccountParams {
                name: "USD Account".to_owned(),
                currency_id: usd_id,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        let (from_id, to_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc_eur,
                amount_minor: -10000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc_usd),
                bucket_id: None,
                original_currency_id: Some(usd_id),
                original_amount_minor: Some(10845),
                fx_rate_mantissa: Some(10845),
                fx_rate_exponent: Some(-4),
            },
        )
        .unwrap();

        let new_to_amount: i64 = 11000;

        update_transfer(
            &conn,
            UpdateTransferParams {
                from_event_id: from_id,
                to_event_id: to_id,
                from_date: "2026-04-01T12:00:00".into(),
                to_date: "2026-04-01T12:00:00".into(),
                from_amount_minor: -10200,
                to_amount_minor: new_to_amount,
                note: Some("cross-currency updated".into()),
                original_currency_id: Some(usd_id),
                original_amount_minor_for_from_leg: Some(new_to_amount),
                fx_rate_mantissa: Some(10784),
                fx_rate_exponent: Some(-4),
            },
        )
        .unwrap();

        let from_updated = get_event_by_id(&conn, from_id).unwrap().unwrap();
        assert_eq!(from_updated.original_currency_id, Some(usd_id));
        assert_eq!(from_updated.original_amount_minor, Some(new_to_amount));
        assert_eq!(from_updated.fx_rate_mantissa, Some(10784));
        assert_eq!(from_updated.fx_rate_exponent, Some(-4));

        let to_updated = get_event_by_id(&conn, to_id).unwrap().unwrap();
        assert_eq!(to_updated.original_currency_id, None);
        assert_eq!(to_updated.original_amount_minor, None);
        assert_eq!(to_updated.fx_rate_mantissa, None);
        assert_eq!(to_updated.fx_rate_exponent, None);
    }

    #[test]
    fn update_transfer_validation_not_linked() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(
            &conn,
            CreateAccountParams {
                name: "Second Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let acc3 = create_account(
            &conn,
            CreateAccountParams {
                name: "Third Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        let (pair1_from, _) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -1000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc2),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let (pair2_from, _) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -2000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc3),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        // pair1_from is linked to pair1_to, not pair2_from — validation should reject this
        let result = update_transfer(
            &conn,
            UpdateTransferParams {
                from_event_id: pair1_from,
                to_event_id: pair2_from,
                from_date: "2026-04-01T12:00:00".into(),
                to_date: "2026-04-01T12:00:00".into(),
                from_amount_minor: -1000,
                to_amount_minor: 1000,
                note: None,
                original_currency_id: None,
                original_amount_minor_for_from_leg: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "VALIDATION");
    }

    #[test]
    fn update_transfer_soft_deleted_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc1 = mk_account(&conn);
        let acc2 = create_account(
            &conn,
            CreateAccountParams {
                name: "Second Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        let (from_id, to_id) = create_transfer(
            &conn,
            &CashflowEntry {
                account_id: acc1,
                amount_minor: -5000,
                event_date: "2026-03-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: Some(acc2),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let now = crate::shared::local_now();
        conn.execute(
            "UPDATE event SET deleted_at = ?1 WHERE id = ?2",
            params![now, to_id],
        )
        .unwrap();

        let result = update_transfer(
            &conn,
            UpdateTransferParams {
                from_event_id: from_id,
                to_event_id: to_id,
                from_date: "2026-04-01T12:00:00".into(),
                to_date: "2026-04-01T12:00:00".into(),
                from_amount_minor: -5000,
                to_amount_minor: 5000,
                note: None,
                original_currency_id: None,
                original_amount_minor_for_from_leg: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "VALIDATION");
    }

    #[test]
    fn get_event_by_id_returns_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);
        let event_id = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 7500,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap();

        let maybe_event = get_event_by_id(&conn, event_id).unwrap();
        assert!(maybe_event.is_some());
        let event = maybe_event.unwrap();
        assert_eq!(event.id, event_id);
        assert_eq!(event.event_type, "balance_update");
        assert_eq!(event.amount_minor, 7500);

        let none = get_event_by_id(&conn, 999999).unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn cashflow_with_bucket_id_contributes_to_bucket_snapshot() {
        let conn = initialize_in_memory().expect("DB init failed");

        let source_id = mk_account(&conn);
        let bucket_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Tagged Bucket".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        // Cashflow on source account tagged to bucket
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id: source_id,
                amount_minor: 3000,
                event_date: "2026-06-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: Some(bucket_id),
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let bucket_row = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();

        // EUR is the consolidation currency → direct 1:1 conversion
        assert_eq!(bucket_row.cashflow_tagged_minor, 3000);
        assert_eq!(bucket_row.converted_balance_minor, 3000);
    }

    #[test]
    fn cashflow_after_snapshot_date_not_counted_in_bucket() {
        let conn = initialize_in_memory().expect("DB init failed");

        let source_id = mk_account(&conn);
        let bucket_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Tagged Bucket".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        // Cashflow dated AFTER the snapshot date
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id: source_id,
                amount_minor: 5000,
                event_date: "2026-12-01T12:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: Some(bucket_id),
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        // Snapshot is for a date BEFORE the cashflow
        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-06-30T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let bucket_row = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();

        assert_eq!(bucket_row.cashflow_tagged_minor, 0);
        assert_eq!(bucket_row.converted_balance_minor, 0);
    }

    #[test]
    fn list_events_filters_by_bucket_id() {
        let conn = initialize_in_memory().expect("DB init failed");

        let source_id = mk_account(&conn);
        let bucket_id = create_account(
            &conn,
            CreateAccountParams {
                name: "Some Bucket".to_owned(),
                currency_id: 1,
                account_type: "bucket".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        // Cashflow on source account tagged to bucket
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id: source_id,
                amount_minor: 2500,
                event_date: "2026-04-01T10:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: Some(bucket_id),
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        // Another cashflow on source account with no bucket tag
        create_cashflow(
            &conn,
            &CashflowEntry {
                account_id: source_id,
                amount_minor: 1000,
                event_date: "2026-04-02T10:00:00".to_string(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();

        // Filter by bucket_id only — should return only the tagged cashflow even though
        // event.account_id is the source account (not the bucket)
        let result = list_events(
            &conn,
            ListEventsQuery {
                bucket_ids: Some(vec![bucket_id]),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].account_id, source_id);
        assert_eq!(result.events[0].bucket_id, Some(bucket_id));
        assert_eq!(result.events[0].amount_minor, 2500);
    }

    #[test]
    fn snapshot_filters_by_person_id() {
        let conn = initialize_in_memory().expect("DB init failed");

        // The migration inserts a default person; fetch its id.
        let default_person_id: i64 = conn
            .query_row("SELECT id FROM person WHERE is_default = 1", [], |r| {
                r.get(0)
            })
            .expect("default person not found");

        // Create a second person.
        let second_person_id = crate::features::persons::repository::create_person(
            &conn,
            crate::features::persons::repository::CreatePersonParams {
                name: "Second Person".to_owned(),
                person_type: "physical".to_owned(),
            },
        )
        .expect("create second person failed");

        // Create one account per person.
        let acc1 = create_account(
            &conn,
            CreateAccountParams {
                name: "Person1 Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                person_id: Some(default_person_id),
                ..Default::default()
            },
        )
        .expect("create acc1 failed");

        let acc2 = create_account(
            &conn,
            CreateAccountParams {
                name: "Person2 Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                person_id: Some(second_person_id),
                ..Default::default()
            },
        )
        .expect("create acc2 failed");

        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                person_id: Some(default_person_id),
            },
        )
        .unwrap();

        let ids: Vec<i64> = snapshot.iter().map(|r| r.account_id).collect();
        assert!(ids.contains(&acc1), "acc1 should be in snapshot");
        assert!(!ids.contains(&acc2), "acc2 should NOT be in snapshot");
    }

    #[test]
    fn snapshot_without_person_filter_returns_all_accounts() {
        let conn = initialize_in_memory().expect("DB init failed");

        let default_person_id: i64 = conn
            .query_row("SELECT id FROM person WHERE is_default = 1", [], |r| {
                r.get(0)
            })
            .expect("default person not found");

        let second_person_id = crate::features::persons::repository::create_person(
            &conn,
            crate::features::persons::repository::CreatePersonParams {
                name: "Another Person".to_owned(),
                person_type: "legal".to_owned(),
            },
        )
        .expect("create second person failed");

        let _acc1 = create_account(
            &conn,
            CreateAccountParams {
                name: "Person1 Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                person_id: Some(default_person_id),
                ..Default::default()
            },
        )
        .expect("create acc1 failed");

        let _acc2 = create_account(
            &conn,
            CreateAccountParams {
                name: "Person2 Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                person_id: Some(second_person_id),
                ..Default::default()
            },
        )
        .expect("create acc2 failed");

        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                person_id: None,
            },
        )
        .unwrap();

        assert_eq!(
            snapshot.len(),
            2,
            "both accounts should be returned when person_id is None"
        );
    }

    // -----------------------------------------------------------------------
    // Taxable event tests
    // -----------------------------------------------------------------------

    #[test]
    fn create_taxable_event_appears_in_list_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);

        let event_id = create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id,
                event_type: "revenue".to_owned(),
                amount_minor: 50000,
                event_date: "2026-05-01".to_owned(),
                note: Some("consulting fee".to_owned()),
                vat_rate_bps: Some(2300),
                ..Default::default()
            },
        )
        .expect("create_taxable_event failed");

        let result = list_events(&conn, ListEventsQuery::default()).unwrap();
        assert_eq!(result.events.len(), 1);
        let ev = &result.events[0];
        assert_eq!(ev.id, event_id);
        assert_eq!(ev.event_type, "revenue");
        assert_eq!(ev.amount_minor, 50000);
        assert_eq!(ev.vat_rate_bps, Some(2300));
        assert_eq!(ev.vat_deductible_pct_bps, None);
        assert_eq!(ev.expense_deductible_pct_bps, None);
        assert_eq!(ev.prepaid_period_months, None);
        assert_eq!(ev.note.as_deref(), Some("consulting fee"));
    }

    #[test]
    fn create_taxable_split_group_two_expense_legs() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);

        let split_group_id = create_taxable_split_group_with_legs(
            &conn,
            CreateTaxableSplitGroupWithLegsParams {
                account_id,
                event_type: "expense".to_owned(),
                group_note: Some("office expenses".to_owned()),
                legs: vec![
                    TaxableEventLeg {
                        amount_minor: -12000,
                        event_date: "2026-06-01".to_owned(),
                        note: Some("stationery".to_owned()),
                        vat_rate_bps: Some(2300),
                        vat_deductible_pct_bps: Some(10000),
                        expense_deductible_pct_bps: Some(10000),
                        prepaid_period_months: None,
                    },
                    TaxableEventLeg {
                        amount_minor: -8000,
                        event_date: "2026-06-01".to_owned(),
                        note: Some("coffee".to_owned()),
                        vat_rate_bps: Some(2300),
                        vat_deductible_pct_bps: Some(5000),
                        expense_deductible_pct_bps: Some(5000),
                        prepaid_period_months: None,
                    },
                ],
            },
        )
        .expect("create_taxable_split_group_with_legs failed");

        let result = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(account_id),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.events.len(), 2, "both legs should appear");

        for ev in &result.events {
            assert_eq!(ev.event_type, "expense");
            assert_eq!(ev.split_group_id, Some(split_group_id));
            assert_eq!(ev.vat_rate_bps, Some(2300));
        }

        let amounts: Vec<i64> = result.events.iter().map(|e| e.amount_minor).collect();
        assert!(amounts.contains(&-12000));
        assert!(amounts.contains(&-8000));
    }

    #[test]
    fn taxable_event_does_not_affect_snapshot_balance() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);

        // Set a baseline balance.
        create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 10000,
                event_date: "2026-01-01".to_owned(),
                note: None,
            },
        )
        .unwrap();

        // Create a revenue event — should NOT change the balance.
        create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id,
                event_type: "revenue".to_owned(),
                amount_minor: 5000,
                event_date: "2026-05-01".to_owned(),
                note: None,
                ..Default::default()
            },
        )
        .unwrap();

        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let row = snapshot
            .iter()
            .find(|r| r.account_id == account_id)
            .unwrap();
        assert_eq!(
            row.balance_minor, 10000,
            "revenue event must not change balance"
        );
    }

    #[test]
    fn update_taxable_event_inserts_new_event_data_row() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);

        let event_id = create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id,
                event_type: "revenue".to_owned(),
                amount_minor: 30000,
                event_date: "2026-04-01".to_owned(),
                note: None,
                ..Default::default()
            },
        )
        .unwrap();

        update_taxable_event(
            &conn,
            UpdateTaxableEventParams {
                event_id,
                amount_minor: 35000,
                event_date: "2026-04-15".to_owned(),
                note: Some("updated".to_owned()),
                vat_rate_bps: Some(1000),
                ..Default::default()
            },
        )
        .expect("update_taxable_event failed");

        // Two event_data rows should exist.
        let data_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event_data WHERE event_id = ?1",
                params![event_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(data_count, 2, "update should insert a new event_data row");

        // latest_data_id should point to the updated row.
        let ev = get_event_by_id(&conn, event_id).unwrap().unwrap();
        assert_eq!(ev.amount_minor, 35000);
        assert_eq!(ev.event_date, "2026-04-15");
        assert_eq!(ev.vat_rate_bps, Some(1000));
        assert_eq!(ev.note.as_deref(), Some("updated"));
    }

    #[test]
    fn update_taxable_split_group_applies_all_changes() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);

        // Create a split group with 3 legs.
        let split_group_id = create_taxable_split_group_with_legs(
            &conn,
            CreateTaxableSplitGroupWithLegsParams {
                account_id,
                event_type: "expense".to_owned(),
                group_note: Some("original note".to_owned()),
                legs: vec![
                    TaxableEventLeg {
                        amount_minor: -1000,
                        event_date: "2026-07-01".to_owned(),
                        note: None,
                        vat_rate_bps: None,
                        vat_deductible_pct_bps: None,
                        expense_deductible_pct_bps: None,
                        prepaid_period_months: None,
                    },
                    TaxableEventLeg {
                        amount_minor: -2000,
                        event_date: "2026-07-01".to_owned(),
                        note: None,
                        vat_rate_bps: None,
                        vat_deductible_pct_bps: None,
                        expense_deductible_pct_bps: None,
                        prepaid_period_months: None,
                    },
                    TaxableEventLeg {
                        amount_minor: -3000,
                        event_date: "2026-07-01".to_owned(),
                        note: None,
                        vat_rate_bps: None,
                        vat_deductible_pct_bps: None,
                        expense_deductible_pct_bps: None,
                        prepaid_period_months: None,
                    },
                ],
            },
        )
        .unwrap();

        // Retrieve the three leg event IDs.
        let mut leg_ids: Vec<i64> = conn
            .prepare(
                "SELECT id FROM event WHERE split_group_id = ?1 AND deleted_at IS NULL ORDER BY id",
            )
            .unwrap()
            .query_map(params![split_group_id], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(leg_ids.len(), 3);

        let leg_to_remove = leg_ids.remove(2); // remove leg 3
        let leg_to_update = leg_ids[0]; // update leg 1

        // Update: remove leg 3, update leg 1, add a new leg.
        update_taxable_split_group(
            &conn,
            UpdateTaxableSplitGroupParams {
                split_group_id,
                group_note: Some("updated note".to_owned()),
                updated_legs: vec![UpdatedSplitLeg {
                    event_id: leg_to_update,
                    amount_minor: -9999,
                    event_date: "2026-07-15".to_owned(),
                    note: Some("updated leg".to_owned()),
                    vat_rate_bps: Some(500),
                    vat_deductible_pct_bps: None,
                    expense_deductible_pct_bps: None,
                    prepaid_period_months: None,
                }],
                new_legs: vec![NewSplitLeg {
                    amount_minor: -4000,
                    event_date: "2026-07-20".to_owned(),
                    note: None,
                    vat_rate_bps: None,
                    vat_deductible_pct_bps: None,
                    expense_deductible_pct_bps: None,
                    prepaid_period_months: None,
                }],
                removed_leg_ids: vec![leg_to_remove],
            },
        )
        .expect("update_taxable_split_group failed");

        // Verify: removed leg is soft-deleted.
        let removed_deleted_at: Option<String> = conn
            .query_row(
                "SELECT deleted_at FROM event WHERE id = ?1",
                params![leg_to_remove],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            removed_deleted_at.is_some(),
            "removed leg should be soft-deleted"
        );

        // Verify: updated leg has 2 event_data rows and latest reflects the update.
        let updated_data_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event_data WHERE event_id = ?1",
                params![leg_to_update],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(updated_data_count, 2, "updated leg should have 2 data rows");
        let updated_ev = get_event_by_id(&conn, leg_to_update).unwrap().unwrap();
        assert_eq!(updated_ev.amount_minor, -9999);
        assert_eq!(updated_ev.vat_rate_bps, Some(500));

        // Verify: 3 non-deleted legs total (leg 1 updated, leg 2 unchanged, 1 new).
        let active_legs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event WHERE split_group_id = ?1 AND deleted_at IS NULL",
                params![split_group_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(active_legs, 3);

        // Verify: split_group.note was updated.
        let group_note: Option<String> = conn
            .query_row(
                "SELECT note FROM split_group WHERE id = ?1",
                params![split_group_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(group_note.as_deref(), Some("updated note"));
    }

    #[test]
    fn update_taxable_split_group_rejects_when_too_few_legs_remain() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);

        // Create a split group with exactly 2 legs.
        let split_group_id = create_taxable_split_group_with_legs(
            &conn,
            CreateTaxableSplitGroupWithLegsParams {
                account_id,
                event_type: "revenue".to_owned(),
                group_note: None,
                legs: vec![
                    TaxableEventLeg {
                        amount_minor: 5000,
                        event_date: "2026-08-01".to_owned(),
                        note: None,
                        vat_rate_bps: None,
                        vat_deductible_pct_bps: None,
                        expense_deductible_pct_bps: None,
                        prepaid_period_months: None,
                    },
                    TaxableEventLeg {
                        amount_minor: 3000,
                        event_date: "2026-08-01".to_owned(),
                        note: None,
                        vat_rate_bps: None,
                        vat_deductible_pct_bps: None,
                        expense_deductible_pct_bps: None,
                        prepaid_period_months: None,
                    },
                ],
            },
        )
        .unwrap();

        let leg_ids: Vec<i64> = conn
            .prepare(
                "SELECT id FROM event WHERE split_group_id = ?1 AND deleted_at IS NULL ORDER BY id",
            )
            .unwrap()
            .query_map(params![split_group_id], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();

        // Attempt to remove 1 leg and add 0 new ones → 1 remaining → error.
        let result = update_taxable_split_group(
            &conn,
            UpdateTaxableSplitGroupParams {
                split_group_id,
                group_note: None,
                updated_legs: vec![],
                new_legs: vec![],
                removed_leg_ids: vec![leg_ids[0]],
            },
        );

        assert!(
            result.is_err(),
            "should error when fewer than 2 legs remain"
        );
        let err = result.unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("at least 2"));

        // Verify neither leg was soft-deleted (transaction was rolled back).
        let active_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event WHERE split_group_id = ?1 AND deleted_at IS NULL",
                params![split_group_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(active_count, 2, "rollback should leave both legs intact");
    }

    #[test]
    fn delete_split_group_deletes_all_legs() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn);

        let split_group_id = create_taxable_split_group_with_legs(
            &conn,
            CreateTaxableSplitGroupWithLegsParams {
                account_id,
                event_type: "expense".to_owned(),
                group_note: None,
                legs: vec![
                    TaxableEventLeg {
                        amount_minor: -1000,
                        event_date: "2026-01-01".to_owned(),
                        note: None,
                        vat_rate_bps: None,
                        vat_deductible_pct_bps: None,
                        expense_deductible_pct_bps: None,
                        prepaid_period_months: None,
                    },
                    TaxableEventLeg {
                        amount_minor: -2000,
                        event_date: "2026-01-01".to_owned(),
                        note: None,
                        vat_rate_bps: None,
                        vat_deductible_pct_bps: None,
                        expense_deductible_pct_bps: None,
                        prepaid_period_months: None,
                    },
                ],
            },
        )
        .expect("create_taxable_split_group_with_legs failed");

        delete_split_group(&conn, split_group_id).expect("delete_split_group failed");

        let deleted_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event WHERE split_group_id = ?1 AND deleted_at IS NOT NULL",
                params![split_group_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(deleted_count, 2, "both legs should be soft-deleted");

        let active_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event WHERE split_group_id = ?1 AND deleted_at IS NULL",
                params![split_group_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(active_count, 0, "no legs should remain active");
    }

    // -----------------------------------------------------------------------
    // Default taxable accounts tests
    // -----------------------------------------------------------------------

    #[test]
    fn snapshot_excludes_default_taxable_accounts() {
        use crate::features::persons::repository::{create_person, CreatePersonParams};

        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = create_person(
            &conn,
            CreatePersonParams {
                name: "Snapshot Filter Test".to_owned(),
                person_type: "physical".to_owned(),
            },
        )
        .expect("create_person failed");

        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2099-12-31T23:59:59".to_owned(),
                person_id: Some(person_id),
            },
        )
        .expect("get_accounts_snapshot failed");

        for row in &snapshot {
            assert!(
                row.account_type != "default_revenue" && row.account_type != "default_expense",
                "snapshot should not include default_revenue or default_expense accounts, found: {}",
                row.account_type
            );
        }
    }

    #[test]
    fn list_events_shows_person_name_for_default_account_events() {
        use crate::features::persons::repository::{
            create_person, list_persons, CreatePersonParams,
        };

        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = create_person(
            &conn,
            CreatePersonParams {
                name: "Invoice Person".to_owned(),
                person_type: "physical".to_owned(),
            },
        )
        .expect("create_person failed");

        let persons = list_persons(&conn).expect("list_persons failed");
        let person = persons
            .iter()
            .find(|p| p.id == person_id)
            .expect("person not found");
        let revenue_account_id = person.default_revenue_account_id;

        let _event_id = create_taxable_event(
            &conn,
            CreateTaxableEventParams {
                account_id: revenue_account_id,
                event_type: "revenue".to_owned(),
                amount_minor: 150000,
                event_date: "2026-07-01".to_owned(),
                note: Some("test invoice".to_owned()),
                ..Default::default()
            },
        )
        .expect("create_taxable_event failed");

        let result = list_events(
            &conn,
            ListEventsQuery {
                account_id: Some(revenue_account_id),
                ..Default::default()
            },
        )
        .expect("list_events failed");

        assert_eq!(result.events.len(), 1);
        assert_eq!(
            result.events[0].account_name.as_str(),
            "Invoice Person",
            "account_name for default_revenue account should be the person's name"
        );
    }
}
