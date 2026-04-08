use crate::error::AppError;
use crate::shared::with_savepoint_app;
use rusqlite::{params, Connection, OptionalExtension};

use crate::features::transactions::models::EventWithData;

// ---------------------------------------------------------------------------
// Params structs
// ---------------------------------------------------------------------------

pub struct LinkCashflowsParams {
    pub taxable_event_id: i64,
    pub cashflow_event_ids: Vec<i64>,
}

#[derive(Default)]
pub struct EligibleCashflowsParams {
    pub person_id: i64,
    pub amount_minor: Option<i64>,
    pub exclude_already_linked: bool,
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Link one or more cashflow events to a taxable event.
/// Validates that the taxable event and each cashflow event exist, are not deleted,
/// have the correct event types, and belong to the same person.
pub fn link_cashflows_to_taxable(
    conn: &Connection,
    params: LinkCashflowsParams,
) -> Result<(), AppError> {
    if params.cashflow_event_ids.is_empty() {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "At least one cashflow_event_id is required".into(),
        });
    }

    with_savepoint_app(conn, || {
        // Validate taxable event: exists, not deleted, event_type in (revenue, expense), get person_id
        let taxable_row: Option<(String, Option<String>, i64)> = conn
            .query_row(
                "SELECT e.event_type, e.deleted_at, a.person_id
                 FROM event e
                 JOIN account a ON a.id = e.account_id
                 WHERE e.id = ?1",
                params![params.taxable_event_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let (taxable_type, taxable_deleted_at, taxable_person_id) =
            taxable_row.ok_or_else(|| AppError {
                code: "NOT_FOUND".into(),
                message: "Taxable event not found".into(),
            })?;

        if taxable_deleted_at.is_some() {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Cannot link to a deleted taxable event".into(),
            });
        }

        if taxable_type != "revenue" && taxable_type != "expense" {
            return Err(AppError {
                code: "VALIDATION".into(),
                message: "Target event must be a revenue or expense event".into(),
            });
        }

        // Validate each cashflow event
        for &cashflow_id in &params.cashflow_event_ids {
            let cashflow_row: Option<(String, Option<String>, i64)> = conn
                .query_row(
                    "SELECT e.event_type, e.deleted_at, a.person_id
                     FROM event e
                     JOIN account a ON a.id = e.account_id
                     WHERE e.id = ?1",
                    params![cashflow_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;

            let (cf_type, cf_deleted_at, cf_person_id) = cashflow_row.ok_or_else(|| AppError {
                code: "NOT_FOUND".into(),
                message: format!("Cashflow event {} not found", cashflow_id),
            })?;

            if cf_deleted_at.is_some() {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: format!("Cashflow event {} is deleted", cashflow_id),
                });
            }

            if cf_type != "cashflow" {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: format!(
                        "Event {} is not a cashflow event (type: {})",
                        cashflow_id, cf_type
                    ),
                });
            }

            if cf_person_id != taxable_person_id {
                return Err(AppError {
                    code: "VALIDATION".into(),
                    message: format!(
                        "Cashflow event {} belongs to a different person",
                        cashflow_id
                    ),
                });
            }

            // Insert link row — detect UNIQUE violation
            let result = conn.execute(
                "INSERT INTO taxable_cashflow_link (taxable_event_id, cashflow_event_id)
                 VALUES (?1, ?2)",
                params![params.taxable_event_id, cashflow_id],
            );

            if let Err(e) = result {
                if let rusqlite::Error::SqliteFailure(sqlite_err, _) = &e {
                    if sqlite_err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE {
                        return Err(AppError {
                            code: "ALREADY_LINKED".into(),
                            message: format!(
                                "Cashflow event {} is already linked to another taxable event",
                                cashflow_id
                            ),
                        });
                    }
                }
                return Err(AppError::from(e));
            }
        }

        Ok(())
    })
}

/// Remove a specific cashflow-to-taxable link.
pub fn unlink_cashflow_from_taxable(
    conn: &Connection,
    taxable_event_id: i64,
    cashflow_event_id: i64,
) -> Result<(), AppError> {
    let rows_deleted = conn.execute(
        "DELETE FROM taxable_cashflow_link
         WHERE taxable_event_id = ?1 AND cashflow_event_id = ?2",
        params![taxable_event_id, cashflow_event_id],
    )?;

    if rows_deleted == 0 {
        return Err(AppError {
            code: "NOT_FOUND".into(),
            message: "Link not found".into(),
        });
    }

    Ok(())
}

/// Return all non-deleted cashflow events linked to a given taxable event.
pub fn list_linked_cashflows(
    conn: &Connection,
    taxable_event_id: i64,
) -> rusqlite::Result<Vec<EventWithData>> {
    let sql = format!(
        "SELECT {}
         FROM event e
         {}
         JOIN taxable_cashflow_link tcl ON tcl.cashflow_event_id = e.id
         WHERE e.deleted_at IS NULL AND tcl.taxable_event_id = ?1
         ORDER BY ed.event_date DESC, e.created_at DESC",
        super::EVENT_SELECT,
        super::EVENT_JOINS
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![taxable_event_id], super::map_event_row)?;
    rows.collect()
}

/// Return cashflow events eligible to be linked to a taxable event.
/// Filtered by person, optionally by amount, optionally excluding already-linked events.
pub fn list_eligible_cashflows(
    conn: &Connection,
    params: EligibleCashflowsParams,
) -> rusqlite::Result<Vec<EventWithData>> {
    let mut where_clauses = vec![
        "e.deleted_at IS NULL".to_string(),
        "e.event_type = 'cashflow'".to_string(),
        "a.person_id = ?1".to_string(),
    ];
    let mut bind_params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(params.person_id)];

    if let Some(amount) = params.amount_minor {
        where_clauses.push(format!("ed.amount_minor = ?{}", bind_params.len() + 1));
        bind_params.push(Box::new(amount));
    }

    if params.exclude_already_linked {
        where_clauses.push(
            "NOT EXISTS (
                SELECT 1 FROM taxable_cashflow_link tcl
                JOIN event te ON te.id = tcl.taxable_event_id AND te.deleted_at IS NULL
                WHERE tcl.cashflow_event_id = e.id
            )"
            .to_string(),
        );
    }

    let where_sql = where_clauses.join(" AND ");

    let sql = format!(
        "SELECT {}
         FROM event e
         {}
         WHERE {}
         ORDER BY ed.event_date DESC, e.created_at DESC",
        super::EVENT_SELECT,
        super::EVENT_JOINS,
        where_sql
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(bind_params.iter()),
        super::map_event_row,
    )?;
    rows.collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::persons::repository::{create_person, CreatePersonParams};
    use crate::features::transactions::repository::{
        create_cashflow, create_taxable_event, CashflowEntry, CreateTaxableEventParams,
    };

    fn mk_person(conn: &Connection, name: &str) -> i64 {
        create_person(
            conn,
            CreatePersonParams {
                name: name.to_owned(),
                person_type: "physical".to_owned(),
            },
        )
        .expect("create person failed")
    }

    fn mk_account(conn: &Connection, person_id: i64) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: "Test Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                person_id: Some(person_id),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn mk_cashflow(conn: &Connection, account_id: i64, amount: i64) -> i64 {
        create_cashflow(
            conn,
            &CashflowEntry {
                account_id,
                amount_minor: amount,
                event_date: "2024-01-15T00:00:00".to_owned(),
                note: None,
                counterpart_account_id: None,
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .expect("create cashflow failed")
    }

    fn resolve_default_expense_account(conn: &Connection, person_id: i64) -> i64 {
        conn.query_row(
            "SELECT default_expense_account_id FROM person WHERE id = ?1",
            params![person_id],
            |row| row.get(0),
        )
        .expect("resolve default expense account failed")
    }

    fn mk_expense(conn: &Connection, person_id: i64, amount: i64) -> i64 {
        let account_id = resolve_default_expense_account(conn, person_id);
        create_taxable_event(
            conn,
            CreateTaxableEventParams {
                account_id,
                event_type: "expense".to_owned(),
                amount_minor: amount,
                event_date: "2024-01-20T00:00:00".to_owned(),
                ..Default::default()
            },
        )
        .expect("create expense failed")
    }

    #[test]
    fn link_cashflow_to_taxable_event() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = mk_person(&conn, "Alice");
        let account_id = mk_account(&conn, person_id);
        let cashflow_id = mk_cashflow(&conn, account_id, 5000);
        let taxable_id = mk_expense(&conn, person_id, 5000);

        link_cashflows_to_taxable(
            &conn,
            LinkCashflowsParams {
                taxable_event_id: taxable_id,
                cashflow_event_ids: vec![cashflow_id],
            },
        )
        .expect("link should succeed");

        let linked = list_linked_cashflows(&conn, taxable_id).expect("list linked cashflows");
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].id, cashflow_id);
    }

    #[test]
    fn link_same_cashflow_to_two_taxable_events_is_rejected() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = mk_person(&conn, "Alice");
        let account_id = mk_account(&conn, person_id);
        let cashflow_id = mk_cashflow(&conn, account_id, 5000);
        let taxable_id1 = mk_expense(&conn, person_id, 5000);
        let taxable_id2 = mk_expense(&conn, person_id, 5000);

        link_cashflows_to_taxable(
            &conn,
            LinkCashflowsParams {
                taxable_event_id: taxable_id1,
                cashflow_event_ids: vec![cashflow_id],
            },
        )
        .expect("first link should succeed");

        let result = link_cashflows_to_taxable(
            &conn,
            LinkCashflowsParams {
                taxable_event_id: taxable_id2,
                cashflow_event_ids: vec![cashflow_id],
            },
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "ALREADY_LINKED");
    }

    #[test]
    fn link_cashflow_from_different_person_is_rejected() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_a = mk_person(&conn, "Alice");
        let person_b = mk_person(&conn, "Bob");
        let account_a = mk_account(&conn, person_a);
        let cashflow_id = mk_cashflow(&conn, account_a, 5000);
        let taxable_id = mk_expense(&conn, person_b, 5000);

        let result = link_cashflows_to_taxable(
            &conn,
            LinkCashflowsParams {
                taxable_event_id: taxable_id,
                cashflow_event_ids: vec![cashflow_id],
            },
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("different person"));
    }

    #[test]
    fn link_non_cashflow_event_is_rejected() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = mk_person(&conn, "Alice");
        let taxable_id1 = mk_expense(&conn, person_id, 5000);
        let taxable_id2 = mk_expense(&conn, person_id, 3000);

        let result = link_cashflows_to_taxable(
            &conn,
            LinkCashflowsParams {
                taxable_event_id: taxable_id1,
                cashflow_event_ids: vec![taxable_id2],
            },
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "VALIDATION");
        assert!(err.message.contains("not a cashflow event"));
    }

    #[test]
    fn list_eligible_cashflows_with_amount_filter() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = mk_person(&conn, "Alice");
        let account_id = mk_account(&conn, person_id);
        let cf_5000 = mk_cashflow(&conn, account_id, 5000);
        let _cf_3000 = mk_cashflow(&conn, account_id, 3000);
        let taxable_id = mk_expense(&conn, person_id, 5000);

        link_cashflows_to_taxable(
            &conn,
            LinkCashflowsParams {
                taxable_event_id: taxable_id,
                cashflow_event_ids: vec![cf_5000],
            },
        )
        .expect("link should succeed");

        // Without filters – both cashflows, exclude already linked
        let eligible = list_eligible_cashflows(
            &conn,
            EligibleCashflowsParams {
                person_id,
                amount_minor: None,
                exclude_already_linked: true,
            },
        )
        .expect("list eligible cashflows");
        assert_eq!(eligible.len(), 1, "only 3000 cashflow should remain");
        assert_eq!(eligible[0].amount_minor, 3000);

        // With amount filter matching the unlinked one
        let eligible_filtered = list_eligible_cashflows(
            &conn,
            EligibleCashflowsParams {
                person_id,
                amount_minor: Some(3000),
                exclude_already_linked: true,
            },
        )
        .expect("list eligible cashflows");
        assert_eq!(eligible_filtered.len(), 1);
    }

    #[test]
    fn unlink_cashflow_removes_link() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = mk_person(&conn, "Alice");
        let account_id = mk_account(&conn, person_id);
        let cashflow_id = mk_cashflow(&conn, account_id, 5000);
        let taxable_id = mk_expense(&conn, person_id, 5000);

        link_cashflows_to_taxable(
            &conn,
            LinkCashflowsParams {
                taxable_event_id: taxable_id,
                cashflow_event_ids: vec![cashflow_id],
            },
        )
        .expect("link should succeed");

        unlink_cashflow_from_taxable(&conn, taxable_id, cashflow_id)
            .expect("unlink should succeed");

        let linked = list_linked_cashflows(&conn, taxable_id).expect("list linked");
        assert!(linked.is_empty(), "no linked cashflows after unlinking");
    }

    #[test]
    fn unlink_nonexistent_link_returns_error() {
        let conn = initialize_in_memory().expect("DB init failed");
        let result = unlink_cashflow_from_taxable(&conn, 9999, 8888);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "NOT_FOUND");
    }

    #[test]
    fn soft_deleted_taxable_event_excluded_from_eligible() {
        let conn = initialize_in_memory().expect("DB init failed");
        let person_id = mk_person(&conn, "Alice");
        let account_id = mk_account(&conn, person_id);
        let cashflow_id = mk_cashflow(&conn, account_id, 5000);
        let taxable_id = mk_expense(&conn, person_id, 5000);

        link_cashflows_to_taxable(
            &conn,
            LinkCashflowsParams {
                taxable_event_id: taxable_id,
                cashflow_event_ids: vec![cashflow_id],
            },
        )
        .expect("link should succeed");

        // Soft-delete the taxable event
        conn.execute(
            "UPDATE event SET deleted_at = '2024-02-01T12:00:00' WHERE id = ?1",
            params![taxable_id],
        )
        .expect("soft delete failed");

        // The cashflow should now appear as eligible again (linked-to taxable is deleted)
        let eligible = list_eligible_cashflows(
            &conn,
            EligibleCashflowsParams {
                person_id,
                amount_minor: None,
                exclude_already_linked: true,
            },
        )
        .expect("list eligible cashflows");

        assert_eq!(
            eligible.len(),
            1,
            "cashflow should be eligible since linked taxable is deleted"
        );
    }
}
