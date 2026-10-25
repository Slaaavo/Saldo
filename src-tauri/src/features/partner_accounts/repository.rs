use crate::error::AppError;
use crate::features::accounts::repository::clear_event_references_to_account;
use crate::features::partner_accounts::models::PartnerAccountRow;
use crate::shared::{is_duplicate_iban_error, validate_iban, with_savepoint_app};
use rusqlite::{params, Connection};

pub struct CreatePartnerAccountParams {
    pub name: String,
    pub iban: String,
    pub currency_id: i64,
}

pub struct UpdatePartnerAccountParams {
    pub account_id: i64,
    pub name: String,
    pub iban: String,
}

pub fn create_partner_account(
    conn: &Connection,
    params: CreatePartnerAccountParams,
) -> Result<i64, AppError> {
    let normalised_iban = validate_iban(params.iban.as_str())?;
    with_savepoint_app(conn, || {
        let next_sort_order: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM account WHERE account_type = 'partner'",
            [],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO account (name, currency_id, account_type, sort_order, iban) VALUES (?1, ?2, 'partner', ?3, ?4)",
            params![params.name.as_str(), params.currency_id, next_sort_order, normalised_iban],
        ).map_err(|e| {
            if is_duplicate_iban_error(&e) {
                AppError {
                    code: "DUPLICATE_IBAN".into(),
                    message: "This IBAN is already in use by another account or partner.".into(),
                }
            } else {
                AppError::from(e)
            }
        })?;
        Ok(conn.last_insert_rowid())
    })
}

pub fn list_partner_accounts(conn: &Connection) -> rusqlite::Result<Vec<PartnerAccountRow>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, a.iban, c.code AS currency_code, a.created_at
         FROM account a
         JOIN currency c ON c.id = a.currency_id
         WHERE a.account_type = 'partner'
         ORDER BY a.name ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(PartnerAccountRow {
            id: row.get(0)?,
            name: row.get(1)?,
            iban: row.get(2)?,
            currency_code: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn update_partner_account(
    conn: &Connection,
    params: UpdatePartnerAccountParams,
) -> Result<(), AppError> {
    let normalised_iban = validate_iban(params.iban.as_str())?;
    with_savepoint_app(conn, || {
        let affected = conn.execute(
            "UPDATE account SET name = ?1, iban = ?2 WHERE id = ?3 AND account_type = 'partner'",
            params![params.name.as_str(), normalised_iban, params.account_id],
        ).map_err(|e| {
            if is_duplicate_iban_error(&e) {
                AppError {
                    code: "DUPLICATE_IBAN".into(),
                    message: "This IBAN is already in use by another account or partner.".into(),
                }
            } else {
                AppError::from(e)
            }
        })?;
        if affected == 0 {
            return Err(AppError {
                code: "NOT_FOUND".into(),
                message: "Partner account not found.".into(),
            });
        }
        Ok(())
    })
}

pub fn delete_partner_account(conn: &Connection, account_id: i64) -> Result<(), AppError> {
    with_savepoint_app(conn, || {
        let affected = conn
            .execute(
                "DELETE FROM account WHERE id = ?1 AND account_type = 'partner'",
                params![account_id],
            )
            .map_err(AppError::from)?;
        if affected == 0 {
            return Err(AppError {
                code: "NOT_FOUND".into(),
                message: "Partner account not found.".into(),
            });
        }
        clear_event_references_to_account(conn, account_id)?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::transactions::repository::{
        create_cashflow, get_event_by_id, update_event, CashflowEntry, UpdateEventParams,
    };

    fn mk_account(conn: &Connection) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: "Checking".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn mk_partner(conn: &Connection, name: &str, iban: &str) -> i64 {
        create_partner_account(
            conn,
            CreatePartnerAccountParams {
                name: name.to_owned(),
                iban: iban.to_owned(),
                currency_id: 1,
            },
        )
        .expect("create partner failed")
    }

    fn mk_cashflow(conn: &Connection, account_id: i64, partner_id: i64, amount_minor: i64) -> i64 {
        create_cashflow(
            conn,
            &CashflowEntry {
                account_id,
                amount_minor,
                event_date: "2026-03-01T10:00:00".to_owned(),
                note: None,
                counterpart_account_id: Some(partner_id),
                bucket_id: None,
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .expect("create cashflow failed")
    }

    fn stored_counterparts(conn: &Connection, event_id: i64) -> Vec<Option<i64>> {
        let mut stmt = conn
            .prepare(
                "SELECT counterpart_account_id FROM event_data WHERE event_id = ?1 ORDER BY id",
            )
            .unwrap();
        stmt.query_map(params![event_id], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn delete_partner_account_clears_it_from_every_entry_that_pointed_at_it() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account = mk_account(&conn);
        let landlord = mk_partner(&conn, "Landlord", "SK3112000000198742637541");
        let employer = mk_partner(&conn, "Employer", "DE89370400440532013000");
        let rent = mk_cashflow(&conn, account, landlord, -50_000);
        update_event(
            &conn,
            UpdateEventParams {
                event_id: rent,
                amount_minor: -55_000,
                event_date: "2026-03-02T10:00:00".to_owned(),
                note: Some("March rent".to_owned()),
            },
        )
        .unwrap();
        let salary = mk_cashflow(&conn, account, employer, 200_000);

        delete_partner_account(&conn, landlord).expect("delete failed");

        assert_eq!(stored_counterparts(&conn, rent), [None, None]);
        let rent = get_event_by_id(&conn, rent).unwrap().unwrap();
        assert_eq!(rent.amount_minor, -55_000);
        assert_eq!(rent.event_date, "2026-03-02T10:00:00");
        assert_eq!(rent.note.as_deref(), Some("March rent"));
        assert_eq!(rent.counterpart_account_id, None);
        assert_eq!(rent.counterpart_account_name, None);
        let salary = get_event_by_id(&conn, salary).unwrap().unwrap();
        assert_eq!(salary.counterpart_account_id, Some(employer));
        assert_eq!(salary.counterpart_account_name.as_deref(), Some("Employer"));
    }
}
