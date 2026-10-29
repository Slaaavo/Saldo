use crate::error::AppError;
use crate::features::transactions::models::RecordedSource;
use crate::features::transactions::SourceRef;
use rusqlite::{params, Connection, OptionalExtension};

/// Whether a document is already in Saldo: a live entry or an asset account carries its
/// reference. The eKasa import and `record_batch` both ask this one question.
pub fn find_recorded_source(
    conn: &Connection,
    source_ref: &SourceRef,
) -> Result<Option<RecordedSource>, AppError> {
    let entry = conn
        .query_row(
            "SELECT e.id, e.batch_id, ed.event_date
             FROM event e
             JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.source_ref = ?1 AND e.deleted_at IS NULL
             ORDER BY e.id
             LIMIT 1",
            params![source_ref.as_str()],
            |row| {
                Ok(RecordedSource {
                    event_id: Some(row.get(0)?),
                    asset_account_id: None,
                    batch_id: row.get(1)?,
                    date: row.get(2)?,
                })
            },
        )
        .optional()?;
    if entry.is_some() {
        return Ok(entry);
    }
    Ok(conn
        .query_row(
            "SELECT id, batch_id, purchase_date FROM account
             WHERE source_ref = ?1
             ORDER BY id
             LIMIT 1",
            params![source_ref.as_str()],
            |row| {
                Ok(RecordedSource {
                    event_id: None,
                    asset_account_id: Some(row.get(0)?),
                    batch_id: row.get(1)?,
                    date: row.get(2)?,
                })
            },
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::persons::repository::get_default_expense_account_id;
    use crate::features::transactions::repository::{
        create_taxable_event, create_taxable_split_group_with_legs, delete_event,
        delete_split_group, update_taxable_event, CreateTaxableEventParams,
        CreateTaxableSplitGroupWithLegsParams, TaxableEventLeg, UpdateTaxableEventParams,
    };

    fn receipt() -> SourceRef {
        SourceRef::ekasa_online("O-7DBCDA8A56EE426DBCDA8A56EE426D1A")
    }

    fn get_default_person_id(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM person WHERE is_default = 1", [], |row| {
            row.get(0)
        })
        .expect("default person not found")
    }

    fn import_expense(conn: &Connection, source_ref: Option<SourceRef>) -> i64 {
        let account_id = get_default_expense_account_id(conn, get_default_person_id(conn)).unwrap();
        create_taxable_event(
            conn,
            CreateTaxableEventParams {
                account_id,
                event_type: "expense".to_owned(),
                amount_minor: 1_250,
                event_date: "2026-05-01".to_owned(),
                source_ref,
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn leg(amount_minor: i64) -> TaxableEventLeg {
        TaxableEventLeg {
            amount_minor,
            event_date: "2026-05-01".to_owned(),
            note: None,
            vat_rate_bps: None,
            vat_reclaimable_pct_bps: None,
            expense_deductible_pct_bps: None,
            prepaid_until: None,
            reclaimed_vat: None,
        }
    }

    fn import_split_group(conn: &Connection) -> i64 {
        let account_id = get_default_expense_account_id(conn, get_default_person_id(conn)).unwrap();
        create_taxable_split_group_with_legs(
            conn,
            CreateTaxableSplitGroupWithLegsParams {
                account_id,
                event_type: "expense".to_owned(),
                legs: vec![leg(1_000), leg(250)],
                source_ref: Some(receipt()),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn live_leg_ids(conn: &Connection, split_group_id: i64) -> Vec<i64> {
        conn.prepare(
            "SELECT id FROM event WHERE split_group_id = ?1 AND deleted_at IS NULL ORDER BY id",
        )
        .unwrap()
        .query_map(params![split_group_id], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
    }

    fn is_recorded(conn: &Connection) -> bool {
        find_recorded_source(conn, &receipt()).unwrap().is_some()
    }

    #[test]
    fn a_receipt_that_was_never_imported_is_not_flagged() {
        let conn = initialize_in_memory().expect("DB init failed");
        import_expense(&conn, None);
        import_expense(
            &conn,
            Some(SourceRef::ekasa_online(
                "O-00000000000000000000000000000000",
            )),
        );

        assert!(!is_recorded(&conn));
    }

    #[test]
    fn an_imported_expense_flags_its_receipt() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = import_expense(&conn, Some(receipt()));

        let recorded = find_recorded_source(&conn, &receipt()).unwrap().unwrap();
        assert_eq!(recorded.event_id, Some(id));
        assert_eq!(recorded.date.as_deref(), Some("2026-05-01"));
    }

    #[test]
    fn a_deleted_expense_no_longer_flags_its_receipt() {
        let conn = initialize_in_memory().expect("DB init failed");
        let first = import_expense(&conn, Some(receipt()));
        let second = import_expense(&conn, Some(receipt()));

        delete_event(&conn, first).unwrap();
        assert!(is_recorded(&conn), "the second import is still live");

        delete_event(&conn, second).unwrap();
        assert!(!is_recorded(&conn));
    }

    #[test]
    fn an_imported_split_group_flags_its_receipt_until_every_leg_is_deleted() {
        let conn = initialize_in_memory().expect("DB init failed");
        let group_id = import_split_group(&conn);
        let legs = live_leg_ids(&conn, group_id);
        assert!(is_recorded(&conn));

        delete_event(&conn, legs[0]).unwrap();
        assert!(is_recorded(&conn), "one leg is still live");

        delete_split_group(&conn, group_id).unwrap();
        assert!(!is_recorded(&conn));
    }

    #[test]
    fn editing_an_imported_expense_keeps_flagging_its_receipt() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = import_expense(&conn, Some(receipt()));

        update_taxable_event(
            &conn,
            UpdateTaxableEventParams {
                event_id: id,
                event_type: "expense".to_owned(),
                amount_minor: 2_000,
                event_date: "2026-05-02".to_owned(),
                note: Some("edited".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(is_recorded(&conn));
    }

    #[test]
    fn an_asset_account_flags_its_document() {
        let conn = initialize_in_memory().expect("DB init failed");
        let invoice = SourceRef::try_from("invoice:36562939/FA20261187".to_owned()).unwrap();
        conn.execute(
            "INSERT INTO account (name, currency_id, account_type, person_id, purchase_date, source_ref)
             VALUES ('MacBook', 1, 'asset', ?1, '2026-10-12', ?2)",
            params![get_default_person_id(&conn), invoice.as_str()],
        )
        .unwrap();
        let account_id = conn.last_insert_rowid();

        let recorded = find_recorded_source(&conn, &invoice).unwrap().unwrap();

        assert_eq!(recorded.asset_account_id, Some(account_id));
        assert_eq!(recorded.event_id, None);
        assert_eq!(recorded.date.as_deref(), Some("2026-10-12"));
    }
}
