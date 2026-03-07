const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_initial.sql",
        include_str!("../../migrations/001_initial.sql"),
    ),
    (
        "002_seed.sql",
        include_str!("../../migrations/002_seed.sql"),
    ),
    (
        "003_add_account_type.sql",
        include_str!("../../migrations/003_add_account_type.sql"),
    ),
    (
        "004_seed_currencies.sql",
        include_str!("../../migrations/004_seed_currencies.sql"),
    ),
    (
        "005_fx_rate_and_settings.sql",
        include_str!("../../migrations/005_fx_rate_and_settings.sql"),
    ),
    (
        "006_bucket_allocation.sql",
        include_str!("../../migrations/006_bucket_allocation.sql"),
    ),
    (
        "007_add_sort_order.sql",
        include_str!("../../migrations/007_add_sort_order.sql"),
    ),
];

pub fn run_pending(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           filename TEXT NOT NULL UNIQUE,
           applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f','now'))
         );",
    )?;

    for (filename, sql) in MIGRATIONS {
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
