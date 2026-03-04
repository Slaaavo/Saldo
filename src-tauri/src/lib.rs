use std::sync::Mutex;
use tauri::Manager;

mod commands;
mod db;
mod error;
mod migrations;
mod models;
mod repository;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_dir).expect("failed to create app data dir");
            let db_path = app_dir.join("our-finances.db");

            let conn = db::initialize_db(&db_path).expect("failed to initialize database");

            app.manage(AppState {
                db: Mutex::new(conn),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_balance_update,
            commands::get_accounts_snapshot,
            commands::list_events,
            commands::create_account,
            commands::update_account,
            commands::delete_account,
            commands::update_event,
            commands::delete_event,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
