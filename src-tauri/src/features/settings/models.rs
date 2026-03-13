use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbLocationInfo {
    pub current_path: String,
    pub is_default: bool,
    pub is_demo_mode: bool,
    pub fallback_warning: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickDbFolderResult {
    pub folder: String,
    pub db_exists: bool,
}
