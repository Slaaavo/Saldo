use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::AppError;

pub const DB_FILENAME: &str = "saldo.db";
const CONFIG_FILENAME: &str = "db-location.json";

#[derive(Debug, Serialize, Deserialize)]
struct DbLocationConfig {
    db_folder: Option<String>,
}

/// Reads the DB location config and returns the custom folder path if valid.
/// Returns `None` on any error (file missing, malformed JSON, or empty path).
pub fn read_db_location(app_data_dir: &Path) -> Option<PathBuf> {
    let config_path = app_data_dir.join(CONFIG_FILENAME);
    let contents = std::fs::read_to_string(&config_path).ok()?;
    let config: DbLocationConfig = serde_json::from_str(&contents).ok()?;
    let folder = config.db_folder?;
    if folder.is_empty() {
        return None;
    }
    Some(PathBuf::from(folder))
}

/// Resolves the database file to open at startup, plus whether to warn that the configured
/// custom location was unavailable.
///
/// The custom location is used only when its `saldo.db` is present: opening a missing file would
/// silently create an empty database there (e.g. a cloud-sync file that was evicted or not yet
/// downloaded). Otherwise the default path is used with the warning flag set.
pub fn resolve_db_path(app_data_dir: &Path) -> (PathBuf, bool) {
    let default_path = app_data_dir.join(DB_FILENAME);
    match read_db_location(app_data_dir) {
        Some(folder) => {
            let custom_path = folder.join(DB_FILENAME);
            if custom_path.exists() {
                (custom_path, false)
            } else {
                (default_path, true)
            }
        }
        None => (default_path, false),
    }
}

/// Writes the custom DB folder path to the config file. The file is written to a temporary name
/// and renamed into place, so a crash or failed write never leaves a truncated config.
pub fn write_db_location(app_data_dir: &Path, folder: &Path) -> Result<(), AppError> {
    let config = DbLocationConfig {
        db_folder: Some(folder.to_string_lossy().to_string()),
    };
    let json = serde_json::to_string_pretty(&config).map_err(|e| AppError {
        code: "SERIALIZATION_ERROR".into(),
        message: e.to_string(),
    })?;
    let config_path = app_data_dir.join(CONFIG_FILENAME);
    let tmp_path = app_data_dir.join(format!("{CONFIG_FILENAME}.tmp"));
    let written = write_synced(&tmp_path, json.as_bytes())
        .and_then(|()| std::fs::rename(&tmp_path, &config_path));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    Ok(written?)
}

fn write_synced(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

/// Deletes the config file, reverting to the default DB location.
pub fn clear_db_location(app_data_dir: &Path) -> Result<(), AppError> {
    let config_path = app_data_dir.join(CONFIG_FILENAME);
    if config_path.exists() {
        std::fs::remove_file(&config_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("failed to create temp dir")
    }

    #[test]
    fn roundtrip_write_and_read() {
        let dir = make_temp_dir();
        let folder = dir.path().join("custom-db");
        fs::create_dir_all(&folder).unwrap();

        write_db_location(dir.path(), &folder).expect("write failed");
        let result = read_db_location(dir.path());
        assert_eq!(result, Some(folder));
    }

    #[test]
    fn missing_config_returns_none() {
        let dir = make_temp_dir();
        assert_eq!(read_db_location(dir.path()), None);
    }

    #[test]
    fn malformed_json_returns_none() {
        let dir = make_temp_dir();
        let config_path = dir.path().join("db-location.json");
        fs::write(&config_path, b"not valid json").unwrap();
        assert_eq!(read_db_location(dir.path()), None);
    }

    #[test]
    fn empty_folder_returns_none() {
        let dir = make_temp_dir();
        let config_path = dir.path().join("db-location.json");
        fs::write(&config_path, r#"{"db_folder": ""}"#).unwrap();
        assert_eq!(read_db_location(dir.path()), None);
    }

    #[test]
    fn null_folder_returns_none() {
        let dir = make_temp_dir();
        let config_path = dir.path().join("db-location.json");
        fs::write(&config_path, r#"{"db_folder": null}"#).unwrap();
        assert_eq!(read_db_location(dir.path()), None);
    }

    #[test]
    fn resolve_without_config_uses_default_path() {
        let dir = make_temp_dir();
        assert_eq!(
            resolve_db_path(dir.path()),
            (dir.path().join(DB_FILENAME), false)
        );
    }

    #[test]
    fn resolve_uses_custom_folder_when_db_file_exists() {
        let dir = make_temp_dir();
        let folder = dir.path().join("custom-db");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(DB_FILENAME), b"").unwrap();
        write_db_location(dir.path(), &folder).unwrap();

        assert_eq!(
            resolve_db_path(dir.path()),
            (folder.join(DB_FILENAME), false)
        );
    }

    #[test]
    fn resolve_falls_back_when_custom_folder_is_missing() {
        let dir = make_temp_dir();
        write_db_location(dir.path(), &dir.path().join("gone")).unwrap();

        assert_eq!(
            resolve_db_path(dir.path()),
            (dir.path().join(DB_FILENAME), true)
        );
    }

    #[test]
    fn resolve_falls_back_when_custom_folder_has_no_db_file() {
        let dir = make_temp_dir();
        let folder = dir.path().join("custom-db");
        fs::create_dir_all(&folder).unwrap();
        write_db_location(dir.path(), &folder).unwrap();

        assert_eq!(
            resolve_db_path(dir.path()),
            (dir.path().join(DB_FILENAME), true)
        );
    }

    #[test]
    fn resolve_finds_db_file_created_when_the_location_was_set() {
        // The "fresh" location flow opens the database at switch time, which must leave a file
        // behind for the next launch to find.
        let dir = make_temp_dir();
        let folder = dir.path().join("custom-db");
        fs::create_dir_all(&folder).unwrap();
        write_db_location(dir.path(), &folder).unwrap();

        crate::db::initialize_db(&folder.join(DB_FILENAME)).expect("DB init failed");

        assert_eq!(
            resolve_db_path(dir.path()),
            (folder.join(DB_FILENAME), false)
        );
    }

    #[test]
    fn clear_removes_config() {
        let dir = make_temp_dir();
        let folder = dir.path().join("some-folder");

        write_db_location(dir.path(), &folder).unwrap();
        assert!(dir.path().join("db-location.json").exists());

        clear_db_location(dir.path()).expect("clear failed");
        assert!(!dir.path().join("db-location.json").exists());

        // idempotent: clearing again should not error
        clear_db_location(dir.path()).expect("second clear failed");
    }
}
