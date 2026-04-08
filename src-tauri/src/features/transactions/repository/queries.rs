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
    let base = "FROM event e
        JOIN account a ON a.id = e.account_id
        JOIN currency c ON c.id = a.currency_id
        JOIN event_data ed ON ed.id = e.latest_data_id
        LEFT JOIN account counter_a ON counter_a.id = ed.counterpart_account_id
        LEFT JOIN account bucket_a ON bucket_a.id = ed.bucket_id
        LEFT JOIN currency orig_c ON orig_c.id = ed.original_currency_id
        LEFT JOIN split_group sg ON sg.id = e.split_group_id
        WHERE e.deleted_at IS NULL";

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
    let mut sql = format!(
        "SELECT
          e.id,
          e.account_id,
          CASE WHEN a.account_type IN ('default_revenue', 'default_expense')
               THEN (SELECT p.name FROM person p WHERE p.id = a.person_id)
               ELSE a.name END AS account_name,
          a.account_type,
          e.event_type,
          ed.event_date,
          ed.amount_minor,
          ed.note,
          e.created_at,
          c.code AS currency_code,
          c.minor_units AS currency_minor_units,
          e.linked_event_id,
          ed.counterpart_account_id,
          ed.bucket_id,
          ed.original_currency_id,
          ed.original_amount_minor,
          ed.fx_rate_mantissa,
          ed.fx_rate_exponent,
          counter_a.name AS counterpart_account_name,
          bucket_a.name AS bucket_name,
          orig_c.code AS original_currency_code,
          orig_c.minor_units AS original_currency_minor_units,
          e.split_group_id,
          sg.note AS split_group_note,
          ed.vat_rate_bps,
          ed.vat_deductible_pct_bps,
          ed.expense_deductible_pct_bps,
          ed.prepaid_period_months
        {}{}",
        base, where_suffix
    );
    sql.push_str(" ORDER BY ed.event_date DESC, e.created_at DESC");

    if let Some(lim) = limit {
        sql.push_str(" LIMIT ?");
        params.push(Box::new(lim));
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
        Ok(EventWithData {
            id: row.get(0)?,
            account_id: row.get(1)?,
            account_name: row.get(2)?,
            account_type: row.get(3)?,
            event_type: row.get(4)?,
            event_date: row.get(5)?,
            amount_minor: row.get(6)?,
            note: row.get(7)?,
            created_at: row.get(8)?,
            currency_code: row.get(9)?,
            currency_minor_units: row.get(10)?,
            linked_event_id: row.get(11)?,
            counterpart_account_id: row.get(12)?,
            bucket_id: row.get(13)?,
            original_currency_id: row.get(14)?,
            original_amount_minor: row.get(15)?,
            fx_rate_mantissa: row.get(16)?,
            fx_rate_exponent: row.get(17)?,
            counterpart_account_name: row.get(18)?,
            bucket_name: row.get(19)?,
            original_currency_code: row.get(20)?,
            original_currency_minor_units: row.get(21)?,
            split_group_id: row.get(22)?,
            split_group_note: row.get(23)?,
            vat_rate_bps: row.get(24)?,
            vat_deductible_pct_bps: row.get(25)?,
            expense_deductible_pct_bps: row.get(26)?,
            prepaid_period_months: row.get(27)?,
        })
    })?;

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
    conn.query_row(
        "SELECT
          e.id,
          e.account_id,
          CASE WHEN a.account_type IN ('default_revenue', 'default_expense')
               THEN (SELECT p.name FROM person p WHERE p.id = a.person_id)
               ELSE a.name END AS account_name,
          a.account_type,
          e.event_type,
          ed.event_date,
          ed.amount_minor,
          ed.note,
          e.created_at,
          c.code AS currency_code,
          c.minor_units AS currency_minor_units,
          e.linked_event_id,
          ed.counterpart_account_id,
          ed.bucket_id,
          ed.original_currency_id,
          ed.original_amount_minor,
          ed.fx_rate_mantissa,
          ed.fx_rate_exponent,
          counter_a.name AS counterpart_account_name,
          bucket_a.name AS bucket_name,
          orig_c.code AS original_currency_code,
          orig_c.minor_units AS original_currency_minor_units,
          e.split_group_id,
          sg.note AS split_group_note,
          ed.vat_rate_bps,
          ed.vat_deductible_pct_bps,
          ed.expense_deductible_pct_bps,
          ed.prepaid_period_months
        FROM event e
        JOIN account a ON a.id = e.account_id
        JOIN currency c ON c.id = a.currency_id
        JOIN event_data ed ON ed.id = e.latest_data_id
        LEFT JOIN account counter_a ON counter_a.id = ed.counterpart_account_id
        LEFT JOIN account bucket_a ON bucket_a.id = ed.bucket_id
        LEFT JOIN currency orig_c ON orig_c.id = ed.original_currency_id
        LEFT JOIN split_group sg ON sg.id = e.split_group_id
        WHERE e.deleted_at IS NULL AND e.id = ?1",
        params![event_id],
        |row| {
            Ok(EventWithData {
                id: row.get(0)?,
                account_id: row.get(1)?,
                account_name: row.get(2)?,
                account_type: row.get(3)?,
                event_type: row.get(4)?,
                event_date: row.get(5)?,
                amount_minor: row.get(6)?,
                note: row.get(7)?,
                created_at: row.get(8)?,
                currency_code: row.get(9)?,
                currency_minor_units: row.get(10)?,
                linked_event_id: row.get(11)?,
                counterpart_account_id: row.get(12)?,
                bucket_id: row.get(13)?,
                original_currency_id: row.get(14)?,
                original_amount_minor: row.get(15)?,
                fx_rate_mantissa: row.get(16)?,
                fx_rate_exponent: row.get(17)?,
                counterpart_account_name: row.get(18)?,
                bucket_name: row.get(19)?,
                original_currency_code: row.get(20)?,
                original_currency_minor_units: row.get(21)?,
                split_group_id: row.get(22)?,
                split_group_note: row.get(23)?,
                vat_rate_bps: row.get(24)?,
                vat_deductible_pct_bps: row.get(25)?,
                expense_deductible_pct_bps: row.get(26)?,
                prepaid_period_months: row.get(27)?,
            })
        },
    )
    .optional()
}
