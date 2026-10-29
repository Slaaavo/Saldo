use rusqlite::Connection;
use std::path::{Path, PathBuf};

use crate::config::{self, DbLocation};
use crate::error::AppError;
use crate::migrations::{self, SchemaMismatch};

/// The database one tool call works on. It is opened for that call and dropped after it, so a
/// changed location, a newly installed Saldo or a restored file shows on the next call, and an
/// idle server holds no lock and no snapshot.
pub(super) struct SaldoDb {
    pub conn: Connection,
    pub path: PathBuf,
}

/// Why the database was not opened. Each one tells the user what to do.
#[derive(Debug)]
pub(super) enum OpenRefusal {
    NoDatabase { expected: PathBuf },
    CustomFolderMissing { folder: PathBuf },
    Schema(SchemaMismatch),
    Db(AppError),
}

/// Opens the same `saldo.db` the app opens, but never creates, migrates or falls back: a batch
/// must not land in a file the user does not see in Saldo.
pub(super) fn open_saldo_db(app_data_dir: &Path) -> Result<SaldoDb, OpenRefusal> {
    let path = match config::resolve_db_location(app_data_dir) {
        DbLocation::CustomMissing { folder, .. } => {
            return Err(OpenRefusal::CustomFolderMissing { folder })
        }
        DbLocation::Default(path) | DbLocation::Custom(path) => path,
    };
    if !path.exists() {
        return Err(OpenRefusal::NoDatabase { expected: path });
    }
    let conn = crate::db::open_existing(&path).map_err(OpenRefusal::Db)?;
    migrations::check_schema(&conn)
        .map_err(OpenRefusal::Db)?
        .map_err(OpenRefusal::Schema)?;
    Ok(SaldoDb { conn, path })
}

impl OpenRefusal {
    pub fn code(&self) -> &'static str {
        match self {
            OpenRefusal::NoDatabase { .. } => "NO_DATABASE",
            OpenRefusal::CustomFolderMissing { .. } => "DATABASE_FOLDER_MISSING",
            OpenRefusal::Schema(SchemaMismatch::NotInitialized) => "NOT_A_SALDO_DATABASE",
            OpenRefusal::Schema(SchemaMismatch::Behind { .. }) => "DATABASE_BEHIND",
            OpenRefusal::Schema(SchemaMismatch::Ahead { .. }) => "DATABASE_AHEAD",
            OpenRefusal::Db(_) => "DATABASE_ERROR",
        }
    }

    pub fn message(&self) -> String {
        match self {
            OpenRefusal::NoDatabase { expected } => format!(
                "Saldo has no database at {} yet. Open Saldo once so it creates one, then try again.",
                expected.display()
            ),
            OpenRefusal::CustomFolderMissing { folder } => format!(
                "Saldo keeps its database in {}, but saldo.db is not there right now (the drive \
                 may be disconnected or the cloud file not downloaded). Make it available, open \
                 Saldo to check its database location, then try again.",
                folder.display()
            ),
            OpenRefusal::Schema(SchemaMismatch::NotInitialized) => {
                "This file is not a Saldo database Saldo has opened. Open Saldo once, then try again."
                    .into()
            }
            OpenRefusal::Schema(SchemaMismatch::Behind { missing }) => format!(
                "Saldo's database is older than this Saldo build ({} updates missing). Open \
                 Saldo once so it updates the database, then try again.",
                missing.len()
            ),
            OpenRefusal::Schema(SchemaMismatch::Ahead { unknown }) => format!(
                "Saldo's database was updated by a newer Saldo build ({} updates this build does \
                 not know). Install the matching Saldo version, then try again.",
                unknown.len()
            ),
            OpenRefusal::Db(e) => format!("Saldo could not open its database: {}", e.message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_data_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn opens_the_migrated_database_in_the_app_data_folder() {
        let dir = app_data_dir();
        let path = dir.path().join(config::DB_FILENAME);
        drop(crate::db::initialize_db(&path).unwrap());

        let db = open_saldo_db(dir.path()).unwrap();

        assert_eq!(db.path, path);
    }

    #[test]
    fn opens_the_database_in_a_configured_custom_folder() {
        let dir = app_data_dir();
        let folder = dir.path().join("cloud");
        std::fs::create_dir_all(&folder).unwrap();
        drop(crate::db::initialize_db(&folder.join(config::DB_FILENAME)).unwrap());
        config::write_db_location(dir.path(), &folder).unwrap();

        let db = open_saldo_db(dir.path()).unwrap();

        assert_eq!(db.path, folder.join(config::DB_FILENAME));
    }

    #[test]
    fn refuses_instead_of_falling_back_when_the_custom_folder_has_no_database() {
        let dir = app_data_dir();
        drop(crate::db::initialize_db(&dir.path().join(config::DB_FILENAME)).unwrap());
        let folder = dir.path().join("evicted");
        config::write_db_location(dir.path(), &folder).unwrap();

        let refusal = open_saldo_db(dir.path()).err().unwrap();

        assert!(
            matches!(&refusal, OpenRefusal::CustomFolderMissing { folder: f } if *f == folder),
            "{refusal:?}"
        );
        assert_eq!(refusal.code(), "DATABASE_FOLDER_MISSING");
    }

    #[test]
    fn refuses_and_creates_nothing_when_there_is_no_database() {
        let dir = app_data_dir();

        let refusal = open_saldo_db(dir.path()).err().unwrap();

        assert_eq!(refusal.code(), "NO_DATABASE");
        assert!(!dir.path().join(config::DB_FILENAME).exists());
    }

    #[test]
    fn refuses_a_database_behind_this_build_without_migrating_it() {
        let dir = app_data_dir();
        let path = dir.path().join(config::DB_FILENAME);
        let conn = crate::db::initialize_db(&path).unwrap();
        let last: String = conn
            .query_row("SELECT MAX(filename) FROM _migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        conn.execute("DELETE FROM _migrations WHERE filename = ?1", [&last])
            .unwrap();
        drop(conn);

        let refusal = open_saldo_db(dir.path()).err().unwrap();

        assert_eq!(refusal.code(), "DATABASE_BEHIND");
        let applied: i64 = Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM _migrations WHERE filename = ?1",
                [&last],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(applied, 0, "MCP mode never migrates");
    }

    #[test]
    fn refuses_a_database_a_newer_build_migrated() {
        let dir = app_data_dir();
        let path = dir.path().join(config::DB_FILENAME);
        let conn = crate::db::initialize_db(&path).unwrap();
        conn.execute(
            "INSERT INTO _migrations (filename) VALUES ('999_from_the_future.sql')",
            [],
        )
        .unwrap();
        drop(conn);

        let refusal = open_saldo_db(dir.path()).err().unwrap();

        assert_eq!(refusal.code(), "DATABASE_AHEAD");
    }
}
