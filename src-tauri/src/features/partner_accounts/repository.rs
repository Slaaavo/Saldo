use crate::error::AppError;
use crate::features::partner_accounts::models::PartnerAccountRow;
use crate::shared::{is_duplicate_iban_error, validate_iban, with_savepoint_app};
use rusqlite::{params, Connection};

pub fn create_partner_account(
    conn: &Connection,
    name: &str,
    iban: &str,
    currency_id: i64,
) -> Result<i64, AppError> {
    let normalised_iban = validate_iban(iban)?;
    with_savepoint_app(conn, || {
        let next_sort_order: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM account WHERE account_type = 'partner'",
            [],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO account (name, currency_id, account_type, sort_order, iban) VALUES (?1, ?2, 'partner', ?3, ?4)",
            params![name, currency_id, next_sort_order, normalised_iban],
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
    account_id: i64,
    name: &str,
    iban: &str,
) -> Result<(), AppError> {
    let normalised_iban = validate_iban(iban)?;
    with_savepoint_app(conn, || {
        let affected = conn.execute(
            "UPDATE account SET name = ?1, iban = ?2 WHERE id = ?3 AND account_type = 'partner'",
            params![name, normalised_iban, account_id],
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
        Ok(())
    })
}
