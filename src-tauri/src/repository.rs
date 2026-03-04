use rusqlite::{params, Connection};
use crate::models::*;

pub fn create_account(
    conn: &Connection,
    name: &str,
    currency_id: i64,
    initial_balance_minor: Option<i64>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO account (name, currency_id) VALUES (?1, ?2)",
        params![name, currency_id],
    )?;
    let account_id = conn.last_insert_rowid();

    if let Some(amount) = initial_balance_minor {
        let now = local_now();
        create_balance_update(conn, account_id, amount, &now, None)?;
    }

    Ok(account_id)
}

pub fn update_account(conn: &Connection, account_id: i64, name: &str) -> rusqlite::Result<()> {
    let rows = conn.execute(
        "UPDATE account SET name = ?1 WHERE id = ?2",
        params![name, account_id],
    )?;
    if rows == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn delete_account(conn: &Connection, account_id: i64) -> Result<(), String> {
    let event_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM event WHERE account_id = ?1 AND deleted_at IS NULL",
            params![account_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if event_count > 0 {
        return Err(format!(
            "Cannot delete account: it has {} active event(s). Delete them first.",
            event_count
        ));
    }

    let rows = conn
        .execute("DELETE FROM account WHERE id = ?1", params![account_id])
        .map_err(|e| e.to_string())?;
    if rows == 0 {
        return Err("Account not found".to_string());
    }
    Ok(())
}

pub fn create_balance_update(
    conn: &Connection,
    account_id: i64,
    amount_minor: i64,
    event_date: &str,
    note: Option<&str>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO event (account_id, event_type) VALUES (?1, 'balance_update')",
        params![account_id],
    )?;
    let event_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO event_data (event_id, amount_minor, event_date, note) VALUES (?1, ?2, ?3, ?4)",
        params![event_id, amount_minor, event_date, note],
    )?;

    Ok(event_id)
}

pub fn update_event(
    conn: &Connection,
    event_id: i64,
    amount_minor: i64,
    event_date: &str,
    note: Option<&str>,
) -> rusqlite::Result<()> {
    let deleted_at: Option<String> = conn.query_row(
        "SELECT deleted_at FROM event WHERE id = ?1",
        params![event_id],
        |row| row.get(0),
    )?;
    if deleted_at.is_some() {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }

    conn.execute(
        "INSERT INTO event_data (event_id, amount_minor, event_date, note) VALUES (?1, ?2, ?3, ?4)",
        params![event_id, amount_minor, event_date, note],
    )?;
    Ok(())
}

pub fn delete_event(conn: &Connection, event_id: i64) -> rusqlite::Result<()> {
    let now = local_now();
    let rows = conn.execute(
        "UPDATE event SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, event_id],
    )?;
    if rows == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn get_accounts_snapshot(
    conn: &Connection,
    selected_datetime: &str,
) -> rusqlite::Result<Vec<SnapshotRow>> {
    let mut stmt = conn.prepare(
        "SELECT
           a.id AS account_id,
           a.name AS account_name,
           COALESCE((
             SELECT ed.amount_minor
             FROM event e
             JOIN event_data ed ON ed.id = e.latest_data_id
             WHERE e.account_id = a.id
               AND e.deleted_at IS NULL
               AND ed.event_date <= ?1
             ORDER BY ed.event_date DESC, e.created_at DESC
             LIMIT 1
           ), 0) AS balance_minor
         FROM account a
         ORDER BY a.name",
    )?;

    let rows = stmt.query_map(params![selected_datetime], |row| {
        Ok(SnapshotRow {
            account_id: row.get(0)?,
            account_name: row.get(1)?,
            balance_minor: row.get(2)?,
        })
    })?;

    rows.collect()
}

pub fn list_events(
    conn: &Connection,
    account_id: Option<i64>,
) -> rusqlite::Result<Vec<EventWithData>> {
    let sql = "
        SELECT
          e.id,
          e.account_id,
          a.name AS account_name,
          e.event_type,
          ed.event_date,
          ed.amount_minor,
          ed.note,
          e.created_at
        FROM event e
        JOIN account a ON a.id = e.account_id
        JOIN event_data ed ON ed.id = e.latest_data_id
        WHERE e.deleted_at IS NULL
          AND (?1 IS NULL OR e.account_id = ?1)
        ORDER BY ed.event_date DESC, e.created_at DESC
    ";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![account_id], |row| {
        Ok(EventWithData {
            id: row.get(0)?,
            account_id: row.get(1)?,
            account_name: row.get(2)?,
            event_type: row.get(3)?,
            event_date: row.get(4)?,
            amount_minor: row.get(5)?,
            note: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;

    rows.collect()
}

fn local_now() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;

    #[test]
    fn snapshot_with_no_events_returns_zero() {
        let conn = initialize_in_memory().expect("DB init failed");
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].account_name, "Main Account");
        assert_eq!(snapshot[0].balance_minor, 0);
    }

    #[test]
    fn snapshot_reflects_balance_update() {
        let conn = initialize_in_memory().expect("DB init failed");
        create_balance_update(&conn, 1, 5000, "2026-03-01T10:00:00", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 5000);
    }

    #[test]
    fn snapshot_ignores_future_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        create_balance_update(&conn, 1, 5000, "2026-06-01T10:00:00", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 0);
    }

    #[test]
    fn snapshot_uses_latest_event_by_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        create_balance_update(&conn, 1, 3000, "2026-01-01T10:00:00", None).unwrap();
        create_balance_update(&conn, 1, 7000, "2026-02-01T10:00:00", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 7000);
    }

    #[test]
    fn snapshot_ignores_soft_deleted_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        let event_id =
            create_balance_update(&conn, 1, 5000, "2026-03-01T10:00:00", None).unwrap();
        delete_event(&conn, event_id).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 0);
    }

    #[test]
    fn update_event_creates_new_data_row() {
        let conn = initialize_in_memory().expect("DB init failed");
        let event_id =
            create_balance_update(&conn, 1, 5000, "2026-03-01T10:00:00", None).unwrap();
        update_event(&conn, event_id, 9999, "2026-03-01T10:00:00", None).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2026-03-01T23:59:59").unwrap();
        assert_eq!(snapshot[0].balance_minor, 9999);
    }

    #[test]
    fn delete_account_blocked_with_active_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        create_balance_update(&conn, 1, 5000, "2026-03-01T10:00:00", None).unwrap();
        let result = delete_account(&conn, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("active event"));
    }

    #[test]
    fn delete_account_succeeds_when_no_active_events() {
        let conn = initialize_in_memory().expect("DB init failed");
        // Main Account (id=1) has no events after migrations
        let result = delete_account(&conn, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn create_account_with_initial_balance() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = create_account(&conn, "Savings", 1, Some(10000)).unwrap();
        let snapshot = get_accounts_snapshot(&conn, "2099-12-31T23:59:59").unwrap();
        let row = snapshot.iter().find(|r| r.account_id == account_id).unwrap();
        assert_eq!(row.balance_minor, 10000);
    }

    #[test]
    fn list_events_returns_all_non_deleted() {
        let conn = initialize_in_memory().expect("DB init failed");
        create_balance_update(&conn, 1, 1000, "2026-01-01T10:00:00", None).unwrap();
        create_balance_update(&conn, 1, 2000, "2026-02-01T10:00:00", None).unwrap();
        let events = list_events(&conn, None).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn list_events_filters_by_account() {
        let conn = initialize_in_memory().expect("DB init failed");
        let acc2 = create_account(&conn, "Second", 1, None).unwrap();
        create_balance_update(&conn, 1, 1000, "2026-01-01T10:00:00", None).unwrap();
        create_balance_update(&conn, acc2, 2000, "2026-02-01T10:00:00", None).unwrap();

        let events_acc1 = list_events(&conn, Some(1)).unwrap();
        assert_eq!(events_acc1.len(), 1);
        assert_eq!(events_acc1[0].account_id, 1);

        let events_acc2 = list_events(&conn, Some(acc2)).unwrap();
        assert_eq!(events_acc2.len(), 1);
        assert_eq!(events_acc2[0].account_id, acc2);
    }
}
