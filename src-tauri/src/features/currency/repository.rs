use crate::shared::with_savepoint;
use rusqlite::{params, Connection, OptionalExtension};

use super::models::{Currency, FxRateRow};

pub fn list_currencies(
    conn: &Connection,
    include_custom: Option<bool>,
) -> rusqlite::Result<Vec<Currency>> {
    let sql = if include_custom.unwrap_or(true) {
        "SELECT id, code, name, minor_units, is_custom FROM currency ORDER BY code"
    } else {
        "SELECT id, code, name, minor_units, is_custom FROM currency WHERE is_custom = 0 ORDER BY code"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(Currency {
            id: row.get(0)?,
            code: row.get(1)?,
            name: row.get(2)?,
            minor_units: row.get(3)?,
            is_custom: row.get::<_, i64>(4)? != 0,
        })
    })?;
    rows.collect()
}

pub fn get_consolidation_currency(conn: &Connection) -> rusqlite::Result<Currency> {
    conn.query_row(
        "SELECT c.id, c.code, c.name, c.minor_units, c.is_custom
         FROM currency c
         JOIN app_setting s ON s.value = c.code
         WHERE s.key = 'consolidation_currency_code'",
        [],
        |row| {
            Ok(Currency {
                id: row.get(0)?,
                code: row.get(1)?,
                name: row.get(2)?,
                minor_units: row.get(3)?,
                is_custom: row.get::<_, i64>(4)? != 0,
            })
        },
    )
}

pub fn set_consolidation_currency(conn: &Connection, currency_id: i64) -> rusqlite::Result<()> {
    with_savepoint(conn, || {
        conn.execute(
            "UPDATE app_setting
             SET value = (SELECT code FROM currency WHERE id = ?1)
             WHERE key = 'consolidation_currency_code'",
            params![currency_id],
        )?;
        // Invalidate all auto-fetched rates — they were computed from the old consolidation.
        conn.execute("DELETE FROM fx_rate WHERE is_manual = 0", [])?;
        Ok(())
    })
}

pub fn set_fx_rate_manual(
    conn: &Connection,
    from_currency_id: i64,
    to_currency_id: i64,
    date: &str,
    rate_mantissa: i64,
    rate_exponent: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO fx_rate (date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent, is_manual)
         VALUES (?1, ?2, ?3, ?4, ?5, 1)
         ON CONFLICT (date, from_currency_id, to_currency_id)
         DO UPDATE SET
           rate_mantissa = excluded.rate_mantissa,
           rate_exponent = excluded.rate_exponent,
           is_manual = 1,
           fetched_at = strftime('%Y-%m-%dT%H:%M:%f','now')",
        params![date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent],
    )?;
    Ok(())
}

pub fn list_fx_rates(
    conn: &Connection,
    date_filter: Option<&str>,
) -> rusqlite::Result<Vec<FxRateRow>> {
    let mut stmt = conn.prepare(
        "SELECT
           fx.id,
           fx.date,
           cf.code AS from_currency_code,
           ct.code AS to_currency_code,
           fx.rate_mantissa,
           fx.rate_exponent,
           fx.is_manual,
           fx.fetched_at
         FROM fx_rate fx
         JOIN currency cf ON cf.id = fx.from_currency_id
         JOIN currency ct ON ct.id = fx.to_currency_id
         WHERE (?1 IS NULL OR fx.date = ?1)
         ORDER BY fx.date DESC, cf.code, ct.code",
    )?;
    let rows = stmt.query_map(params![date_filter], |row| {
        Ok(FxRateRow {
            id: row.get(0)?,
            date: row.get(1)?,
            from_currency_code: row.get(2)?,
            to_currency_code: row.get(3)?,
            rate_mantissa: row.get(4)?,
            rate_exponent: row.get(5)?,
            is_manual: row.get::<_, i64>(6)? != 0,
            fetched_at: row.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn get_fx_rate_for_conversion(
    conn: &Connection,
    from_currency_id: i64,
    to_currency_id: i64,
    date: &str,
) -> rusqlite::Result<Option<(i64, i64)>> {
    conn.query_row(
        "SELECT rate_mantissa, rate_exponent
         FROM fx_rate
         WHERE from_currency_id = ?1
           AND to_currency_id = ?2
           AND date <= ?3
         ORDER BY date DESC
         LIMIT 1",
        params![from_currency_id, to_currency_id, date],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )
    .optional()
}

/// Batch-upsert auto-fetched FX rates.
/// Skips rows where `is_manual = 1` (manual rates are preserved and not overwritten).
pub fn upsert_fx_rates(
    conn: &Connection,
    rates: &[(String, i64, i64, i64, i64)], // (date, from_currency_id, to_currency_id, mantissa, exponent)
) -> rusqlite::Result<()> {
    with_savepoint(conn, || {
        for (date, from_id, to_id, mantissa, exponent) in rates {
            conn.execute(
                "INSERT INTO fx_rate
                   (date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent, is_manual)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0)
                 ON CONFLICT (date, from_currency_id, to_currency_id) DO UPDATE SET
                   rate_mantissa = excluded.rate_mantissa,
                   rate_exponent = excluded.rate_exponent,
                   fetched_at    = strftime('%Y-%m-%dT%H:%M:%f','now')
                 WHERE is_manual = 0",
                params![date, from_id, to_id, mantissa, exponent],
            )?;
        }
        Ok(())
    })
}

/// Return (code, id) for each distinct currency used by accounts, excluding the
/// consolidation currency. These are the currencies for which cross rates are needed.
pub fn get_active_foreign_currencies(
    conn: &Connection,
    consolidation_id: i64,
) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT c.code, c.id
         FROM account a
         JOIN currency c ON c.id = a.currency_id
         WHERE a.currency_id != ?1
           AND c.is_custom = 0",
    )?;
    let rows = stmt.query_map(params![consolidation_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    rows.collect()
}

/// Return the most recently stored fx_rate date for a given (from, to) currency pair.
/// Returns None if no rates have been stored for that pair.
pub fn get_latest_fx_rate_date(
    conn: &Connection,
    from_currency_id: i64,
    to_currency_id: i64,
) -> rusqlite::Result<Option<String>> {
    // MAX() on an empty set returns NULL — query_row always gets one row.
    conn.query_row(
        "SELECT MAX(date) FROM fx_rate WHERE from_currency_id = ?1 AND to_currency_id = ?2",
        params![from_currency_id, to_currency_id],
        |row| row.get::<_, Option<String>>(0),
    )
}

/// Return dates that have balance-update events for non-consolidation-currency accounts
/// but lack a corresponding fx_rate row.  Used for ledger-driven backfill.
pub fn get_dates_needing_fx_rates(
    conn: &Connection,
    consolidation_id: i64,
) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT substr(ed.event_date, 1, 10) AS d
         FROM event e
         JOIN event_data ed ON ed.id = e.latest_data_id
         JOIN account a     ON a.id  = e.account_id
         WHERE a.currency_id != ?1
           AND e.deleted_at IS NULL
           AND e.latest_data_id IS NOT NULL
           AND NOT EXISTS (
             SELECT 1 FROM fx_rate fx
             WHERE fx.date             = substr(ed.event_date, 1, 10)
               AND fx.from_currency_id = ?1
               AND fx.to_currency_id   = a.currency_id
           )
         ORDER BY d",
    )?;
    let rows = stmt.query_map(params![consolidation_id], |row| row.get(0))?;
    rows.collect()
}

/// Return `true` if every currency in `active_currencies` has a stored fx_rate row for the
/// given `date` and `consolidation_id`. Returns `true` immediately if the slice is empty.
pub fn has_all_fx_rates_for_date(
    conn: &Connection,
    consolidation_id: i64,
    active_currencies: &[(String, i64)],
    date: &str,
) -> rusqlite::Result<bool> {
    if active_currencies.is_empty() {
        return Ok(true);
    }
    for (_, currency_id) in active_currencies {
        let exists: i64 = conn.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM fx_rate
               WHERE date = ?1
                 AND from_currency_id = ?2
                 AND to_currency_id = ?3
             )",
            params![date, consolidation_id, currency_id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;

    #[test]
    fn get_consolidation_currency_returns_eur() {
        let conn = initialize_in_memory().expect("DB init failed");
        let c = get_consolidation_currency(&conn).unwrap();
        assert_eq!(c.code, "EUR");
        assert_eq!(c.minor_units, 2);
    }

    #[test]
    fn set_consolidation_currency_updates_and_clears_auto_rates() {
        let conn = initialize_in_memory().expect("DB init failed");
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();

        // Insert an auto-fetched rate and a manual rate
        conn.execute(
            "INSERT INTO fx_rate (date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent, is_manual) VALUES ('2026-03-01', ?1, ?2, 10842, -4, 0)",
            params![eur, usd],
        ).unwrap();
        conn.execute(
            "INSERT INTO fx_rate (date, from_currency_id, to_currency_id, rate_mantissa, rate_exponent, is_manual) VALUES ('2026-03-01', ?1, ?2, 10842, -4, 1)",
            params![usd, eur],
        ).unwrap();

        set_consolidation_currency(&conn, usd).unwrap();

        let c = get_consolidation_currency(&conn).unwrap();
        assert_eq!(c.code, "USD");

        // Auto-fetched rate deleted, manual rate preserved
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM fx_rate WHERE is_manual = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);

        let manual_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM fx_rate WHERE is_manual = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(manual_count, 1);
    }

    #[test]
    fn get_fx_rate_for_conversion_returns_most_recent_on_or_before_date() {
        let conn = initialize_in_memory().expect("DB init failed");
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();

        set_fx_rate_manual(&conn, eur, usd, "2026-01-01", 10800, -4).unwrap();
        set_fx_rate_manual(&conn, eur, usd, "2026-02-01", 10842, -4).unwrap();
        set_fx_rate_manual(&conn, eur, usd, "2026-03-15", 10900, -4).unwrap();

        // On 2026-03-01, most recent on-or-before is 2026-02-01
        let rate = get_fx_rate_for_conversion(&conn, eur, usd, "2026-03-01").unwrap();
        assert_eq!(rate, Some((10842, -4)));

        // Before any rates → None
        let none = get_fx_rate_for_conversion(&conn, eur, usd, "2025-12-31").unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn upsert_fx_rates_creates_new_rows() {
        let conn = initialize_in_memory().expect("DB init failed");
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();

        let rates = vec![("2026-03-01".to_string(), eur, usd, 10842_i64, -4_i64)];
        upsert_fx_rates(&conn, &rates).unwrap();

        let rate = get_fx_rate_for_conversion(&conn, eur, usd, "2026-03-01").unwrap();
        assert_eq!(rate, Some((10842, -4)));
    }

    #[test]
    fn upsert_fx_rates_overwrites_existing_auto_rows() {
        let conn = initialize_in_memory().expect("DB init failed");
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();

        let initial = vec![("2026-03-01".to_string(), eur, usd, 10842_i64, -4_i64)];
        upsert_fx_rates(&conn, &initial).unwrap();

        let updated = vec![("2026-03-01".to_string(), eur, usd, 10900_i64, -4_i64)];
        upsert_fx_rates(&conn, &updated).unwrap();

        let rate = get_fx_rate_for_conversion(&conn, eur, usd, "2026-03-01").unwrap();
        assert_eq!(rate, Some((10900, -4)));
    }

    #[test]
    fn upsert_fx_rates_preserves_manual_rows() {
        let conn = initialize_in_memory().expect("DB init failed");
        let eur = conn
            .query_row("SELECT id FROM currency WHERE code = 'EUR'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let usd = conn
            .query_row("SELECT id FROM currency WHERE code = 'USD'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();

        // Insert a manual rate
        set_fx_rate_manual(&conn, eur, usd, "2026-03-01", 10842, -4).unwrap();

        // Attempt to overwrite with an auto-fetched rate
        let rates = vec![("2026-03-01".to_string(), eur, usd, 99999_i64, -4_i64)];
        upsert_fx_rates(&conn, &rates).unwrap();

        // Manual rate should be unchanged
        let rate = get_fx_rate_for_conversion(&conn, eur, usd, "2026-03-01").unwrap();
        assert_eq!(rate, Some((10842, -4)));

        let is_manual: i64 = conn
            .query_row(
                "SELECT is_manual FROM fx_rate WHERE from_currency_id = ?1 AND to_currency_id = ?2 AND date = '2026-03-01'",
                params![eur, usd],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(is_manual, 1);
    }
}
