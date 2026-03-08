use serde::{Deserialize, Serialize};
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

/// Writes the custom DB folder path to the config file.
pub fn write_db_location(app_data_dir: &Path, folder: &Path) -> Result<(), AppError> {
    let config = DbLocationConfig {
        db_folder: Some(folder.to_string_lossy().to_string()),
    };
    let json = serde_json::to_string_pretty(&config).map_err(|e| AppError {
        code: "SERIALIZATION_ERROR".into(),
        message: e.to_string(),
    })?;
    let config_path = app_data_dir.join(CONFIG_FILENAME);
    std::fs::write(&config_path, json)?;
    Ok(())
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
