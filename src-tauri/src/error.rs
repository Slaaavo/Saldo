use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(failure, Some(message)) = &e {
            if failure.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER {
                return AppError {
                    code: "VALIDATION".into(),
                    message: message.clone(),
                };
            }
        }
        AppError {
            code: "DB_ERROR".into(),
            message: e.to_string(),
        }
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError {
            code: "APP_ERROR".into(),
            message: s,
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError {
            code: "IO_ERROR".into(),
            message: e.to_string(),
        }
    }
}
