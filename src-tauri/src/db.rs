use rusqlite::{Connection, Result as SqlResult};
use std::path::Path;

use crate::migrations;

pub fn initialize_db(db_path: &Path) -> SqlResult<Connection> {
    let conn = Connection::open(db_path)?;
    set_pragmas(&conn)?;
    migrations::run_pending(&conn)?;
    Ok(conn)
}

pub fn set_pragmas(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;",
    )?;
    Ok(())
}

#[cfg(test)]
pub fn initialize_in_memory() -> SqlResult<Connection> {
    let conn = Connection::open_in_memory()?;
    set_pragmas(&conn)?;
    crate::migrations::run_pending(&conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_cleanly() {
        let conn = initialize_in_memory().expect("DB init failed");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .expect("query failed");
        assert_eq!(count, 2, "Expected 2 applied migrations");
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = initialize_in_memory().expect("DB init failed");
        // Run migrations again — should be a no-op
        crate::migrations::run_pending(&conn).expect("Second migration run failed");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .expect("query failed");
        assert_eq!(count, 2, "Expected 2 applied migrations after double run");
    }

    #[test]
    fn seed_data_present() {
        let conn = initialize_in_memory().expect("DB init failed");

        let currency_code: String = conn
            .query_row("SELECT code FROM currency WHERE id = 1", [], |row| {
                row.get(0)
            })
            .expect("currency query failed");
        assert_eq!(currency_code, "EUR");

        let account_name: String = conn
            .query_row("SELECT name FROM account WHERE id = 1", [], |row| {
                row.get(0)
            })
            .expect("account query failed");
        assert_eq!(account_name, "Main Account");
    }
}
