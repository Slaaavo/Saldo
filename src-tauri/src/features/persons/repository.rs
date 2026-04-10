use crate::error::AppError;
use crate::features::persons::models::PersonRow;
use crate::shared::{local_now, with_savepoint_app};
use rusqlite::{params, Connection, OptionalExtension};

pub struct CreatePersonParams {
    pub name: String,
    pub person_type: String,
    pub vat_payer: bool,
}

pub struct UpdatePersonParams {
    pub person_id: i64,
    pub name: String,
    pub person_type: String,
    pub vat_payer: bool,
}

/// Returns the consolidation currency ID, falling back to the first currency in the table.
fn get_consolidation_currency_id(conn: &Connection) -> Result<i64, AppError> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT c.id FROM currency c
             JOIN app_setting s ON s.value = c.code
             WHERE s.key = 'consolidation_currency_code'
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(AppError::from)?;
    if let Some(id) = id {
        return Ok(id);
    }
    conn.query_row("SELECT id FROM currency ORDER BY id LIMIT 1", [], |row| {
        row.get(0)
    })
    .map_err(AppError::from)
}

/// Creates the two hidden default taxable accounts for a person and returns (revenue_id, expense_id).
pub fn create_default_taxable_accounts(
    conn: &Connection,
    person_id: i64,
) -> Result<(i64, i64), AppError> {
    let currency_id = get_consolidation_currency_id(conn)?;

    conn.execute(
        "INSERT INTO account (name, currency_id, account_type, sort_order, person_id)
         VALUES ('__default_revenue', ?1, 'default_revenue', 0, ?2)",
        params![currency_id, person_id],
    )
    .map_err(AppError::from)?;
    let revenue_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO account (name, currency_id, account_type, sort_order, person_id)
         VALUES ('__default_expense', ?1, 'default_expense', 0, ?2)",
        params![currency_id, person_id],
    )
    .map_err(AppError::from)?;
    let expense_id = conn.last_insert_rowid();

    Ok((revenue_id, expense_id))
}

pub fn create_person(conn: &Connection, params: CreatePersonParams) -> Result<i64, AppError> {
    let now = local_now();
    with_savepoint_app(conn, || {
        conn.execute(
            "INSERT INTO person (name, person_type, is_default, created_at, vat_payer) VALUES (?1, ?2, 0, ?3, ?4)",
            params![params.name.as_str(), params.person_type.as_str(), now, params.vat_payer as i64],
        )
        .map_err(AppError::from)?;
        let person_id = conn.last_insert_rowid();

        let (revenue_id, expense_id) = create_default_taxable_accounts(conn, person_id)?;

        conn.execute(
            "UPDATE person SET default_revenue_account_id = ?1, default_expense_account_id = ?2 WHERE id = ?3",
            params![revenue_id, expense_id, person_id],
        )
        .map_err(AppError::from)?;

        Ok(person_id)
    })
}

pub fn list_persons(conn: &Connection) -> rusqlite::Result<Vec<PersonRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, person_type, is_default, created_at,
                default_revenue_account_id, default_expense_account_id, vat_payer
         FROM person
         ORDER BY is_default DESC, name ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let is_default_int: i64 = row.get(3)?;
        let vat_payer_int: i64 = row.get(7)?;
        Ok(PersonRow {
            id: row.get(0)?,
            name: row.get(1)?,
            person_type: row.get(2)?,
            is_default: is_default_int != 0,
            created_at: row.get(4)?,
            default_revenue_account_id: row.get::<_, i64>(5)?,
            default_expense_account_id: row.get::<_, i64>(6)?,
            vat_payer: vat_payer_int != 0,
        })
    })?;
    rows.collect()
}

/// Resolves the correct hidden default account ID for a person and event type.
/// Returns NOT_FOUND if the person does not exist, DOMAIN_ERROR if the account FK is NULL.
pub fn resolve_default_taxable_account(
    conn: &Connection,
    person_id: i64,
    event_type: &str,
) -> Result<i64, AppError> {
    let row: Option<(Option<i64>, Option<i64>)> = conn
        .query_row(
            "SELECT default_revenue_account_id, default_expense_account_id FROM person WHERE id = ?1",
            params![person_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(AppError::from)?;

    let (rev_id, exp_id) = row.ok_or_else(|| AppError {
        code: "NOT_FOUND".into(),
        message: "Person not found".into(),
    })?;

    match event_type {
        "revenue" => rev_id.ok_or_else(|| AppError {
            code: "DOMAIN_ERROR".into(),
            message: "Person has no default revenue account".into(),
        }),
        "expense" => exp_id.ok_or_else(|| AppError {
            code: "DOMAIN_ERROR".into(),
            message: "Person has no default expense account".into(),
        }),
        _ => Err(AppError {
            code: "VALIDATION".into(),
            message: "event_type must be 'revenue' or 'expense'".into(),
        }),
    }
}

pub fn update_person(conn: &Connection, params: UpdatePersonParams) -> Result<(), AppError> {
    let affected = conn
        .execute(
            "UPDATE person SET name = ?1, person_type = ?2, vat_payer = ?3 WHERE id = ?4",
            params![
                params.name.as_str(),
                params.person_type.as_str(),
                params.vat_payer as i64,
                params.person_id
            ],
        )
        .map_err(AppError::from)?;
    if affected == 0 {
        return Err(AppError {
            code: "NOT_FOUND".into(),
            message: "Person not found.".into(),
        });
    }
    Ok(())
}

/// Returns the default expense account ID for the given person.
/// Returns NOT_FOUND if the person does not exist, DOMAIN_ERROR if the FK is NULL.
pub fn get_default_expense_account_id(conn: &Connection, person_id: i64) -> Result<i64, AppError> {
    resolve_default_taxable_account(conn, person_id, "expense")
}

/// Returns the vat_payer flag for the given person.
/// Returns NOT_FOUND if the person does not exist.
pub fn get_person_vat_payer(conn: &Connection, person_id: i64) -> Result<bool, AppError> {
    let row: Option<i64> = conn
        .query_row(
            "SELECT vat_payer FROM person WHERE id = ?1",
            params![person_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(AppError::from)?;

    row.map(|v| v != 0).ok_or_else(|| AppError {
        code: "NOT_FOUND".into(),
        message: "Person not found".into(),
    })
}

pub fn get_person_accounts(conn: &Connection, person_id: i64) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT id FROM account WHERE person_id = ?1")?;
    let rows = stmt.query_map(params![person_id], |row| row.get(0))?;
    rows.collect()
}

pub fn delete_person_row(conn: &Connection, person_id: i64) -> Result<(), AppError> {
    let is_default: i64 = conn
        .query_row(
            "SELECT is_default FROM person WHERE id = ?1",
            params![person_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError {
                code: "NOT_FOUND".into(),
                message: "Person not found.".into(),
            },
            other => AppError::from(other),
        })?;

    if is_default != 0 {
        return Err(AppError {
            code: "DOMAIN_ERROR".into(),
            message: "Cannot delete the default person.".into(),
        });
    }

    let affected = conn
        .execute("DELETE FROM person WHERE id = ?1", params![person_id])
        .map_err(AppError::from)?;
    if affected == 0 {
        return Err(AppError {
            code: "NOT_FOUND".into(),
            message: "Person not found.".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;

    fn get_default_person_id(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM person WHERE is_default = 1", [], |row| {
            row.get(0)
        })
        .expect("default person not found")
    }

    #[test]
    fn create_and_list_returns_new_person() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_person(
            &conn,
            CreatePersonParams {
                name: "Alice Corp".to_owned(),
                person_type: "legal".to_owned(),
                vat_payer: false,
            },
        )
        .expect("create_person failed");

        let persons = list_persons(&conn).expect("list_persons failed");
        let created = persons
            .iter()
            .find(|p| p.id == id)
            .expect("new person not in list");
        assert_eq!(created.name, "Alice Corp");
        assert_eq!(created.person_type, "legal");
        assert!(!created.is_default);
    }

    #[test]
    fn create_person_auto_creates_default_taxable_accounts() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_person(
            &conn,
            CreatePersonParams {
                name: "Tax Tester Ltd".to_owned(),
                person_type: "legal".to_owned(),
                vat_payer: false,
            },
        )
        .expect("create_person failed");

        let persons = list_persons(&conn).expect("list_persons failed");
        let person = persons
            .iter()
            .find(|p| p.id == id)
            .expect("person not found");

        assert!(
            person.default_revenue_account_id > 0,
            "revenue account ID should be set"
        );
        assert!(
            person.default_expense_account_id > 0,
            "expense account ID should be set"
        );
        assert_ne!(
            person.default_revenue_account_id, person.default_expense_account_id,
            "revenue and expense accounts should be distinct"
        );

        let revenue_type: String = conn
            .query_row(
                "SELECT account_type FROM account WHERE id = ?1",
                [person.default_revenue_account_id],
                |row| row.get(0),
            )
            .expect("revenue account not found");
        assert_eq!(revenue_type, "default_revenue");

        let expense_type: String = conn
            .query_row(
                "SELECT account_type FROM account WHERE id = ?1",
                [person.default_expense_account_id],
                |row| row.get(0),
            )
            .expect("expense account not found");
        assert_eq!(expense_type, "default_expense");
    }

    #[test]
    fn default_person_has_default_accounts_after_migration() {
        let conn = initialize_in_memory().expect("DB init failed");

        let persons = list_persons(&conn).expect("list_persons failed");
        let default_person = persons
            .iter()
            .find(|p| p.is_default)
            .expect("default person not found");

        assert!(
            default_person.default_revenue_account_id > 0,
            "default person should have revenue account after migration 024 backfill"
        );
        assert!(
            default_person.default_expense_account_id > 0,
            "default person should have expense account after migration 024 backfill"
        );
    }

    #[test]
    fn default_person_cannot_be_deleted() {
        let conn = initialize_in_memory().expect("DB init failed");
        let default_id = get_default_person_id(&conn);
        let result = delete_person_row(&conn, default_id);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "DOMAIN_ERROR");
    }

    #[test]
    fn update_person_changes_name() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_person(
            &conn,
            CreatePersonParams {
                name: "Old Name".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .expect("create_person failed");

        update_person(
            &conn,
            UpdatePersonParams {
                person_id: id,
                name: "New Name".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .expect("update_person failed");

        let persons = list_persons(&conn).expect("list_persons failed");
        let updated = persons
            .iter()
            .find(|p| p.id == id)
            .expect("person not found");
        assert_eq!(updated.name, "New Name");
    }

    #[test]
    fn resolve_default_taxable_account_returns_not_found_for_missing_person() {
        let conn = initialize_in_memory().expect("DB init failed");
        let result = resolve_default_taxable_account(&conn, 999_999, "revenue");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "NOT_FOUND");
    }

    #[test]
    fn resolve_default_taxable_account_returns_correct_ids() {
        let conn = initialize_in_memory().expect("DB init failed");
        let id = create_person(
            &conn,
            CreatePersonParams {
                name: "Resolver Test".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .expect("create_person failed");

        let persons = list_persons(&conn).expect("list_persons failed");
        let person = persons
            .iter()
            .find(|p| p.id == id)
            .expect("person not found");

        let rev =
            resolve_default_taxable_account(&conn, id, "revenue").expect("revenue resolve failed");
        assert_eq!(rev, person.default_revenue_account_id);

        let exp =
            resolve_default_taxable_account(&conn, id, "expense").expect("expense resolve failed");
        assert_eq!(exp, person.default_expense_account_id);
    }
}
