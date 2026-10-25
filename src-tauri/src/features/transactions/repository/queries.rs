use crate::error::AppError;
use crate::features::transactions::models::{EventWithData, ListEventsResult};
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Default)]
pub struct ListEventsQuery {
    pub account_id: Option<i64>,
    pub account_ids: Option<Vec<i64>>,
    pub before_date: Option<String>,
    pub from_date: Option<String>,
    pub event_types: Option<Vec<String>>,
    /// Maximum number of cards, newest first. A split group is one card and arrives with all
    /// the legs that match the other filters; any other event is one card.
    pub limit: Option<i64>,
    pub bucket_ids: Option<Vec<i64>>,
    pub person_id: Option<i64>,
    pub unmatched_only: bool,
}

pub fn list_events(
    conn: &Connection,
    query: ListEventsQuery,
) -> rusqlite::Result<ListEventsResult> {
    let account_id = query.account_id;
    let account_ids = query.account_ids.as_deref();
    let before_date = query.before_date.as_deref();
    let from_date = query.from_date.as_deref();
    let event_types = query.event_types.as_deref();
    let limit = query.limit;
    let bucket_ids = query.bucket_ids.as_deref();
    let person_id = query.person_id;
    let unmatched_only = query.unmatched_only;
    let base = format!(
        "FROM event e {} WHERE e.deleted_at IS NULL",
        super::EVENT_JOINS
    );

    let mut where_suffix = String::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    // account_ids multi-select filter: non-empty slice takes precedence over account_id
    let use_multi = account_ids.is_some_and(|ids| !ids.is_empty());
    let use_bucket_ids = bucket_ids.is_some_and(|ids| !ids.is_empty());
    if use_multi && use_bucket_ids {
        // Both filters present: match either account or tagged bucket
        let acct_ids = account_ids.unwrap();
        let bkt_ids = bucket_ids.unwrap();
        let acct_placeholders = std::iter::repeat_n("?", acct_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let linked_placeholders = acct_placeholders.clone();
        let bkt_placeholders = std::iter::repeat_n("?", bkt_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        where_suffix.push_str(&format!(
            " AND (e.account_id IN ({acct_placeholders}) OR e.linked_asset_id IN ({linked_placeholders}) OR ed.bucket_id IN ({bkt_placeholders}))"
        ));
        for &id in acct_ids {
            params.push(Box::new(id));
        }
        for &id in acct_ids {
            params.push(Box::new(id));
        }
        for &id in bkt_ids {
            params.push(Box::new(id));
        }
    } else if use_multi {
        let ids = account_ids.unwrap();
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let linked_placeholders = placeholders.clone();
        where_suffix.push_str(&format!(" AND (e.account_id IN ({placeholders}) OR e.linked_asset_id IN ({linked_placeholders}))"));
        for &id in ids {
            params.push(Box::new(id));
        }
        for &id in ids {
            params.push(Box::new(id));
        }
    } else if use_bucket_ids {
        let ids = bucket_ids.unwrap();
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        where_suffix.push_str(&format!(" AND ed.bucket_id IN ({placeholders})"));
        for &id in ids {
            params.push(Box::new(id));
        }
    } else if let Some(id) = account_id {
        where_suffix.push_str(" AND (e.account_id = ? OR e.linked_asset_id = ?)");
        params.push(Box::new(id));
        params.push(Box::new(id));
    }

    // Events are stored either date-only or with a time, which do not order consistently against
    // a bound of the other shape, so the range is compared by calendar day.
    if let Some(bd) = before_date {
        where_suffix.push_str(" AND substr(ed.event_date, 1, 10) <= substr(?, 1, 10)");
        params.push(Box::new(bd.to_owned()));
    }

    if let Some(fd) = from_date {
        where_suffix.push_str(" AND substr(ed.event_date, 1, 10) >= substr(?, 1, 10)");
        params.push(Box::new(fd.to_owned()));
    }

    if let Some(types) = event_types {
        if !types.is_empty() {
            let placeholders = std::iter::repeat_n("?", types.len())
                .collect::<Vec<_>>()
                .join(", ");
            where_suffix.push_str(&format!(" AND e.event_type IN ({placeholders})"));
            for t in types {
                params.push(Box::new(t.to_owned()));
            }
        }
    }

    if let Some(pid) = person_id {
        where_suffix.push_str(" AND a.person_id = ?");
        params.push(Box::new(pid));
    }

    if unmatched_only {
        where_suffix.push_str(
            " AND e.event_type = 'cashflow' \
             AND NOT EXISTS ( \
               SELECT 1 FROM taxable_cashflow_link tcl \
               JOIN event te ON te.id = tcl.taxable_event_id AND te.deleted_at IS NULL \
               WHERE tcl.cashflow_event_id = e.id \
             )",
        );
    }

    // Count query: same conditions, no ORDER BY or LIMIT
    let count_sql = format!("SELECT COUNT(*) {}{}", base, where_suffix);
    let total_count: i64 = conn.query_row(
        &count_sql,
        rusqlite::params_from_iter(params.iter()),
        |row| row.get(0),
    )?;

    let sql = match limit {
        Some(lim) => {
            params.push(Box::new(lim));
            format!(
                "WITH matched AS (
                   SELECT e.id AS event_id,
                          CASE WHEN e.split_group_id IS NULL
                               THEN 'e' || e.id ELSE 'g' || e.split_group_id END AS card,
                          ed.event_date AS event_date,
                          e.created_at AS created_at
                   {base}{where_suffix}
                 ),
                 newest_cards AS (
                   SELECT card FROM matched
                   GROUP BY card
                   ORDER BY MAX(event_date) DESC, MAX(created_at) DESC
                   LIMIT ?
                 )
                 SELECT {select} FROM event e {joins}
                 WHERE e.id IN (SELECT event_id FROM matched
                                WHERE card IN (SELECT card FROM newest_cards))
                 ORDER BY ed.event_date DESC, e.created_at DESC",
                select = super::EVENT_SELECT,
                joins = super::EVENT_JOINS,
            )
        }
        None => format!(
            "SELECT {} {}{} ORDER BY ed.event_date DESC, e.created_at DESC",
            super::EVENT_SELECT,
            base,
            where_suffix
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(params.iter()),
        super::map_event_row,
    )?;

    let events = rows.collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(ListEventsResult {
        events,
        total_count,
    })
}

pub fn get_event_by_id(
    conn: &Connection,
    event_id: i64,
) -> rusqlite::Result<Option<EventWithData>> {
    let sql = format!(
        "SELECT {} FROM event e {} WHERE e.deleted_at IS NULL AND e.id = ?1",
        super::EVENT_SELECT,
        super::EVENT_JOINS
    );
    conn.query_row(&sql, params![event_id], super::map_event_row)
        .optional()
}

/// Returns a VALIDATION error if the event with `event_id` has `is_system_generated = 1`.
pub fn check_event_not_system_generated(conn: &Connection, event_id: i64) -> Result<(), AppError> {
    let is_system_generated: Option<bool> = conn
        .query_row(
            "SELECT is_system_generated FROM event WHERE id = ?1",
            params![event_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()
        .map_err(AppError::from)?;

    match is_system_generated {
        None => Err(AppError {
            code: "NOT_FOUND".into(),
            message: "Event not found".into(),
        }),
        Some(true) => Err(AppError {
            code: "VALIDATION".into(),
            message: "System-generated events cannot be modified".into(),
        }),
        Some(false) => Ok(()),
    }
}

/// Returns a VALIDATION error if any non-deleted leg of `split_group_id` is system-generated.
pub fn check_split_group_not_system_generated(
    conn: &Connection,
    split_group_id: i64,
) -> Result<(), AppError> {
    let has_system_generated: bool = conn
        .query_row(
            "SELECT EXISTS(\
               SELECT 1 FROM event \
               WHERE split_group_id = ?1 \
               AND deleted_at IS NULL \
               AND is_system_generated = 1\
             )",
            params![split_group_id],
            |row| row.get(0),
        )
        .map_err(AppError::from)?;

    if has_system_generated {
        return Err(AppError {
            code: "VALIDATION".into(),
            message: "System-generated events cannot be modified".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::persons::repository::{create_person, CreatePersonParams};
    use crate::features::transactions::models::SplitGroupEntry;
    use crate::features::transactions::repository::{
        create_balance_update, create_split_group_with_legs, CreateBalanceUpdateParams,
        CreateSplitGroupWithLegsParams,
    };

    /// Seeds one event per stored date format around 2026-10-23 and returns their dates, oldest first.
    fn seed_events(conn: &Connection) -> Vec<&'static str> {
        let account_id = create_account(
            conn,
            CreateAccountParams {
                name: "Test Account".to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed");
        let dates = vec![
            "2026-10-22T23:59:59",
            "2026-10-23",
            "2026-10-23T10:30:00",
            "2026-10-24",
        ];
        for date in &dates {
            create_balance_update(
                conn,
                CreateBalanceUpdateParams {
                    account_id,
                    amount_minor: 100,
                    event_date: (*date).to_owned(),
                    note: None,
                },
            )
            .unwrap();
        }
        dates
    }

    fn listed_dates(conn: &Connection, query: ListEventsQuery) -> Vec<String> {
        let mut dates: Vec<String> = list_events(conn, query)
            .unwrap()
            .events
            .into_iter()
            .map(|e| e.event_date)
            .collect();
        dates.sort();
        dates
    }

    #[test]
    fn from_bound_includes_date_only_and_timed_events_on_that_day() {
        let conn = initialize_in_memory().expect("DB init failed");
        seed_events(&conn);

        for from_date in ["2026-10-23T00:00:00", "2026-10-23"] {
            let dates = listed_dates(
                &conn,
                ListEventsQuery {
                    from_date: Some(from_date.to_owned()),
                    ..Default::default()
                },
            );
            assert_eq!(
                dates,
                ["2026-10-23", "2026-10-23T10:30:00", "2026-10-24"],
                "from_date {from_date}"
            );
        }
    }

    #[test]
    fn to_bound_includes_date_only_and_timed_events_on_that_day() {
        let conn = initialize_in_memory().expect("DB init failed");
        seed_events(&conn);

        for before_date in ["2026-10-23T23:59:59", "2026-10-23"] {
            let dates = listed_dates(
                &conn,
                ListEventsQuery {
                    before_date: Some(before_date.to_owned()),
                    ..Default::default()
                },
            );
            assert_eq!(
                dates,
                ["2026-10-22T23:59:59", "2026-10-23", "2026-10-23T10:30:00"],
                "before_date {before_date}"
            );
        }
    }

    #[test]
    fn single_day_range_returns_both_stored_formats_of_that_day() {
        let conn = initialize_in_memory().expect("DB init failed");
        seed_events(&conn);

        let dates = listed_dates(
            &conn,
            ListEventsQuery {
                from_date: Some("2026-10-23T00:00:00".to_owned()),
                before_date: Some("2026-10-23T23:59:59".to_owned()),
                ..Default::default()
            },
        );

        assert_eq!(dates, ["2026-10-23", "2026-10-23T10:30:00"]);
    }

    fn mk_account(conn: &Connection, name: &str, person_id: Option<i64>) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: name.to_owned(),
                currency_id: 1,
                account_type: "account".to_owned(),
                person_id,
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn balance_update_on(conn: &Connection, account_id: i64, date: &str) {
        create_balance_update(
            conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor: 100,
                event_date: date.to_owned(),
                note: None,
            },
        )
        .unwrap();
    }

    fn three_leg_split_group(conn: &Connection, account_id: i64, date: &str) -> i64 {
        let leg = |amount_minor: i64| SplitGroupEntry {
            amount_minor,
            event_date: date.to_owned(),
            note: None,
            counterpart_account_id: None,
            bucket_id: None,
            original_currency_id: None,
            original_amount_minor: None,
            fx_rate_mantissa: None,
            fx_rate_exponent: None,
        };
        create_split_group_with_legs(
            conn,
            CreateSplitGroupWithLegsParams {
                account_id,
                group_note: None,
                legs: vec![leg(-100), leg(-200), leg(-300)],
            },
        )
        .expect("create split group failed")
    }

    fn april(day: u32) -> String {
        format!("2026-04-{day:02}")
    }

    #[test]
    fn limit_counts_a_split_group_as_one_card_and_returns_all_its_legs() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn, "Test Account", None);
        for day in 1..=5 {
            balance_update_on(&conn, account_id, &format!("2026-01-{day:02}"));
        }
        let group_id = three_leg_split_group(&conn, account_id, "2026-03-01");
        for day in 1..=19 {
            balance_update_on(&conn, account_id, &april(day));
        }

        let result = list_events(
            &conn,
            ListEventsQuery {
                limit: Some(20),
                ..Default::default()
            },
        )
        .unwrap();

        let group_legs = result
            .events
            .iter()
            .filter(|e| e.split_group_id == Some(group_id))
            .count();
        assert_eq!(group_legs, 3);
        assert_eq!(result.events.len(), 19 + 3);
        assert!(result
            .events
            .iter()
            .all(|e| e.event_date.as_str() >= "2026-03-01"));
        assert_eq!(result.total_count, 5 + 3 + 19);
    }

    #[test]
    fn a_split_group_past_the_limit_is_left_out_whole() {
        let conn = initialize_in_memory().expect("DB init failed");
        let account_id = mk_account(&conn, "Test Account", None);
        three_leg_split_group(&conn, account_id, "2026-03-01");
        for day in 1..=19 {
            balance_update_on(&conn, account_id, &april(day));
        }

        let result = list_events(
            &conn,
            ListEventsQuery {
                limit: Some(19),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(result.events.len(), 19);
        assert!(result.events.iter().all(|e| e.split_group_id.is_none()));
        assert_eq!(result.total_count, 19 + 3);
    }

    #[test]
    fn limit_applies_to_the_selected_persons_events_only() {
        let conn = initialize_in_memory().expect("DB init failed");
        let other_person = create_person(
            &conn,
            CreatePersonParams {
                name: "Other".to_owned(),
                person_type: "physical".to_owned(),
                vat_payer: false,
            },
        )
        .unwrap();
        let mine = mk_account(&conn, "Mine", Some(1));
        let theirs = mk_account(&conn, "Theirs", Some(other_person));
        balance_update_on(&conn, mine, "2026-02-01");
        let group_id = three_leg_split_group(&conn, mine, "2026-03-01");
        for day in 1..=5 {
            balance_update_on(&conn, theirs, &april(day));
        }

        let result = list_events(
            &conn,
            ListEventsQuery {
                limit: Some(2),
                person_id: Some(1),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(result.events.iter().all(|e| e.account_id == mine));
        let dates: Vec<&str> = result
            .events
            .iter()
            .map(|e| e.event_date.as_str())
            .collect();
        assert_eq!(dates.iter().filter(|d| **d == "2026-03-01").count(), 3);
        assert_eq!(dates.iter().filter(|d| **d == "2026-02-01").count(), 1);
        assert!(result
            .events
            .iter()
            .filter(|e| e.event_date == "2026-03-01")
            .all(|e| e.split_group_id == Some(group_id)));
        assert_eq!(result.total_count, 1 + 3);
    }
}
