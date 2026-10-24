use rusqlite::Connection;

use crate::error::AppError;

include!(concat!(env!("OUT_DIR"), "/migrations_list.rs"));

pub fn run_pending(conn: &Connection) -> Result<(), AppError> {
    apply_pending(conn, MIGRATIONS)
}

fn apply_pending(conn: &Connection, migrations: &[(&str, &str)]) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           filename TEXT NOT NULL UNIQUE,
           applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now'))
         );",
    )?;

    for (filename, sql) in migrations {
        let already_applied: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM _migrations WHERE filename = ?1)",
            [filename],
            |row| row.get(0),
        )?;

        if !already_applied {
            conn.execute_batch(sql)?;
            conn.execute("INSERT INTO _migrations (filename) VALUES (?1)", [filename])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fk_enabled_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [name],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn applied(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT filename FROM _migrations ORDER BY id")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn a_failing_migration_leaves_no_partial_schema() {
        let conn = fk_enabled_conn();
        let migrations = [
            (
                "001_first.sql",
                "CREATE TABLE first (id INTEGER PRIMARY KEY);",
            ),
            (
                "002_broken.sql",
                "CREATE TABLE second (id INTEGER PRIMARY KEY);
                 INSERT INTO missing VALUES (1);",
            ),
        ];

        let err = apply_pending(&conn, &migrations).unwrap_err();

        assert!(table_exists(&conn, "first"));
        assert!(!table_exists(&conn, "second"));
        assert_eq!(applied(&conn), ["001_first.sql"]);
        assert!(conn.is_autocommit());
        assert!(err.message.contains("002_broken.sql"), "{}", err.message);
    }

    #[test]
    fn a_migration_that_leaves_an_orphan_row_fails() {
        let conn = fk_enabled_conn();
        let migrations = [
            (
                "001_tables.sql",
                "CREATE TABLE parent (id INTEGER PRIMARY KEY);
                 CREATE TABLE child (
                   id INTEGER PRIMARY KEY,
                   parent_id INTEGER NOT NULL REFERENCES parent (id)
                 );",
            ),
            (
                "002_orphan.sql",
                "INSERT INTO child (id, parent_id) VALUES (1, 99);",
            ),
        ];

        let err = apply_pending(&conn, &migrations).unwrap_err();

        assert!(err.message.contains("002_orphan.sql"), "{}", err.message);
        let children: i64 = conn
            .query_row("SELECT COUNT(*) FROM child", [], |row| row.get(0))
            .unwrap();
        assert_eq!(children, 0);
        assert_eq!(applied(&conn), ["001_tables.sql"]);
        let foreign_keys: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
    }
}
