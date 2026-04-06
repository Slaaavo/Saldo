use crate::error::AppError;
use crate::features::persons::models::PersonRow;
use crate::shared::local_now;
use rusqlite::{params, Connection};

pub struct CreatePersonParams {
    pub name: String,
    pub person_type: String,
}

pub struct UpdatePersonParams {
    pub person_id: i64,
    pub name: String,
    pub person_type: String,
}

pub fn create_person(conn: &Connection, params: CreatePersonParams) -> Result<i64, AppError> {
    let now = local_now();
    conn.execute(
        "INSERT INTO person (name, person_type, is_default, created_at) VALUES (?1, ?2, 0, ?3)",
        params![params.name.as_str(), params.person_type.as_str(), now],
    )
    .map_err(AppError::from)?;
    Ok(conn.last_insert_rowid())
}

pub fn list_persons(conn: &Connection) -> rusqlite::Result<Vec<PersonRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, person_type, is_default, created_at
         FROM person
         ORDER BY is_default DESC, name ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let is_default_int: i64 = row.get(3)?;
        Ok(PersonRow {
            id: row.get(0)?,
            name: row.get(1)?,
            person_type: row.get(2)?,
            is_default: is_default_int != 0,
            created_at: row.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn update_person(conn: &Connection, params: UpdatePersonParams) -> Result<(), AppError> {
    let affected = conn
        .execute(
            "UPDATE person SET name = ?1, person_type = ?2 WHERE id = ?3",
            params![
                params.name.as_str(),
                params.person_type.as_str(),
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
            },
        )
        .expect("create_person failed");

        update_person(
            &conn,
            UpdatePersonParams {
                person_id: id,
                name: "New Name".to_owned(),
                person_type: "physical".to_owned(),
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
}
