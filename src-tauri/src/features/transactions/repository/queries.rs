use crate::features::transactions::models::{EventWithData, ListEventsResult};
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Default)]
pub struct ListEventsQuery {
    pub account_id: Option<i64>,
    pub account_ids: Option<Vec<i64>>,
    pub before_date: Option<String>,
    pub from_date: Option<String>,
    pub event_types: Option<Vec<String>>,
    pub limit: Option<i64>,
    pub bucket_ids: Option<Vec<i64>>,
    pub person_id: Option<i64>,
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
        let bkt_placeholders = std::iter::repeat_n("?", bkt_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        where_suffix.push_str(&format!(
            " AND (e.account_id IN ({acct_placeholders}) OR ed.bucket_id IN ({bkt_placeholders}))"
        ));
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
        where_suffix.push_str(&format!(" AND e.account_id IN ({placeholders})"));
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
        where_suffix.push_str(" AND e.account_id = ?");
        params.push(Box::new(id));
    }

    if let Some(bd) = before_date {
        where_suffix.push_str(" AND ed.event_date <= ?");
        params.push(Box::new(bd.to_owned()));
    }

    if let Some(fd) = from_date {
        where_suffix.push_str(" AND ed.event_date >= ?");
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

    // Count query: same conditions, no ORDER BY or LIMIT
    let count_sql = format!("SELECT COUNT(*) {}{}", base, where_suffix);
    let total_count: i64 = conn.query_row(
        &count_sql,
        rusqlite::params_from_iter(params.iter()),
        |row| row.get(0),
    )?;

    // Events query: add ORDER BY and optional LIMIT
    let mut sql = format!("SELECT {} {}{}", super::EVENT_SELECT, base, where_suffix);
    sql.push_str(" ORDER BY ed.event_date DESC, e.created_at DESC");

    if let Some(lim) = limit {
        sql.push_str(" LIMIT ?");
        params.push(Box::new(lim));
    }

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
