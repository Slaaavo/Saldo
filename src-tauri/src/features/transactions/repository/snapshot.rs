use crate::features::assets::repository::get_all_account_asset_link_ids;
use crate::features::buckets::repository::list_all_latest_bucket_links;
use crate::features::currency::repository::{
    get_consolidation_currency, get_fx_rate_for_conversion, GetFxRateForConversionParams,
};
use crate::features::transactions::models::SnapshotRow;
use crate::shared::convert_balance;
use rusqlite::{params, Connection};
use std::collections::HashMap;

type SnapshotRawRow = (
    i64,
    String,
    String,
    Option<String>,
    i64,
    String,
    i64,
    i64,
    i64,
    Option<i64>,
    Option<i64>,
    Option<String>,
    Option<i64>,
);

#[derive(Default)]
pub struct GetSnapshotParams {
    pub selected_datetime: String,
    pub person_id: Option<i64>,
}

pub fn get_accounts_snapshot(
    conn: &Connection,
    params: GetSnapshotParams,
) -> rusqlite::Result<Vec<SnapshotRow>> {
    let selected_datetime = params.selected_datetime.as_str();
    let person_id = params.person_id;
    let consolidation = get_consolidation_currency(conn)?;
    // Extract YYYY-MM-DD from datetime string for fx_rate date comparison.
    let snapshot_date = &selected_datetime[..10.min(selected_datetime.len())];

    let mut stmt = conn.prepare(
        "SELECT
           a.id AS account_id,
           a.name AS account_name,
           a.account_type,
           a.iban,
           c.id AS currency_id,
           c.code AS currency_code,
           c.minor_units AS currency_minor_units,
           c.is_custom AS currency_is_custom,
           COALESCE(
             (SELECT ed.amount_minor
              FROM event e
              JOIN event_data ed ON ed.id = e.latest_data_id
              WHERE e.account_id = a.id
                AND e.deleted_at IS NULL
                AND e.event_type = 'balance_update'
                AND ed.event_date <= ?1
              ORDER BY ed.event_date DESC, e.created_at DESC
              LIMIT 1),
             0
           ) + COALESCE(
             (SELECT SUM(ed2.amount_minor)
              FROM event e2
              JOIN event_data ed2 ON ed2.id = e2.latest_data_id
              WHERE e2.account_id = a.id
                AND e2.deleted_at IS NULL
                AND e2.event_type IN ('cashflow', 'transfer')
                AND ed2.event_date <= ?1
                AND ed2.event_date > COALESCE(
                  (SELECT ed3.event_date
                   FROM event e3
                   JOIN event_data ed3 ON ed3.id = e3.latest_data_id
                   WHERE e3.account_id = a.id
                     AND e3.deleted_at IS NULL
                     AND e3.event_type = 'balance_update'
                     AND ed3.event_date <= ?1
                   ORDER BY ed3.event_date DESC, e3.created_at DESC
                   LIMIT 1),
                  ''
                )),
             0
           ) AS balance_minor,
           a.person_id,
           a.purchase_price_minor,
           a.purchase_date,
           a.depreciation_period_months
         FROM account a
         JOIN currency c ON c.id = a.currency_id
         WHERE a.account_type IN ('account', 'bucket', 'asset')
           AND (?2 IS NULL OR a.person_id = ?2)
         ORDER BY a.account_type, a.sort_order, a.id",
    )?;

    let row_data: Vec<SnapshotRawRow> = stmt
        .query_map(params![selected_datetime, person_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut result = Vec::with_capacity(row_data.len());
    for (
        account_id,
        account_name,
        account_type,
        iban,
        currency_id,
        currency_code,
        currency_minor_units,
        currency_is_custom,
        balance_minor,
        person_id_row,
        purchase_price_minor,
        purchase_date,
        depreciation_period_months,
    ) in row_data
    {
        let (converted_balance_minor, fx_rate_missing) = if currency_id == consolidation.id {
            (balance_minor, false)
        } else {
            match get_fx_rate_for_conversion(
                conn,
                GetFxRateForConversionParams {
                    from_currency_id: consolidation.id,
                    to_currency_id: currency_id,
                    date: snapshot_date.to_owned(),
                },
            )? {
                Some((mantissa, exponent, is_direct)) => {
                    let converted = convert_balance(
                        balance_minor,
                        mantissa,
                        exponent,
                        currency_minor_units,
                        consolidation.minor_units,
                        is_direct,
                    );
                    (converted, false)
                }
                None => {
                    // 1:1 fallback: mantissa=1, exponent=0
                    let converted = convert_balance(
                        balance_minor,
                        1,
                        0,
                        currency_minor_units,
                        consolidation.minor_units,
                        false,
                    );
                    (converted, true)
                }
            }
        };

        result.push(SnapshotRow {
            account_id,
            account_name,
            account_type,
            iban,
            balance_minor,
            currency_code,
            currency_minor_units,
            is_custom: currency_is_custom != 0,
            converted_balance_minor,
            fx_rate_missing,
            is_linked_to_asset: false,
            linked_asset_ids: vec![],
            is_bucket_linked: false,
            bucket_links: vec![],
            linked_balance_minor: 0,
            cashflow_tagged_minor: 0,
            person_id: person_id_row,
            purchase_price_minor,
            purchase_date,
            depreciation_period_months,
        });
    }

    // Populate asset-link fields using a single bulk query to avoid N+1.
    let (linked_account_ids_set, account_to_assets, asset_to_accounts) =
        get_all_account_asset_link_ids(conn)?;
    for row in &mut result {
        if row.account_type == "account" {
            if linked_account_ids_set.contains(&row.account_id) {
                row.is_linked_to_asset = true;
                if let Some(asset_ids) = account_to_assets.get(&row.account_id) {
                    row.linked_asset_ids = asset_ids.clone();
                }
            }
        } else if row.account_type == "asset" {
            if let Some(account_ids) = asset_to_accounts.get(&row.account_id) {
                row.linked_asset_ids = account_ids.clone();
            }
        }
    }

    // Pass 2: Event-bound bucket links
    let all_bucket_links = list_all_latest_bucket_links(conn, snapshot_date)?;
    let account_index_map: HashMap<i64, usize> = result
        .iter()
        .enumerate()
        .map(|(idx, row)| (row.account_id, idx))
        .collect();

    for (bucket_id, link) in &all_bucket_links {
        let bucket_idx = match account_index_map.get(bucket_id) {
            Some(&idx) => idx,
            None => continue,
        };
        let source_idx = match account_index_map.get(&link.source_account_id) {
            Some(&idx) => idx,
            None => continue,
        };

        let source_balance = result[source_idx].balance_minor;
        let source_currency_id = link.source_currency_id;
        let source_minor_units = link.source_currency_minor_units;

        let converted = if source_currency_id == consolidation.id {
            source_balance
        } else {
            match get_fx_rate_for_conversion(
                conn,
                GetFxRateForConversionParams {
                    from_currency_id: consolidation.id,
                    to_currency_id: source_currency_id,
                    date: snapshot_date.to_owned(),
                },
            )? {
                Some((mantissa, exponent, is_direct)) => convert_balance(
                    source_balance,
                    mantissa,
                    exponent,
                    source_minor_units,
                    consolidation.minor_units,
                    is_direct,
                ),
                None => {
                    result[bucket_idx].fx_rate_missing = true;
                    convert_balance(
                        source_balance,
                        1,
                        0,
                        source_minor_units,
                        consolidation.minor_units,
                        false,
                    )
                }
            }
        };

        result[bucket_idx].bucket_links.push(link.clone());
        result[bucket_idx].linked_balance_minor += converted;
        result[bucket_idx].converted_balance_minor += converted;
        result[source_idx].is_bucket_linked = true;
    }

    // Pass 3: Cashflow-tagged amounts per bucket.
    // Aggregate cashflows where event_data.bucket_id is set, grouped by bucket and source
    // currency. This lets the bucket card show how much of its balance comes from tagged
    // cashflow entries (e.g. CSV-imported rows).
    {
        let mut stmt = conn.prepare(
            "SELECT
               ed.bucket_id AS bucket_account_id,
               a.currency_id,
               c.minor_units,
               SUM(ed.amount_minor) AS tagged_total
             FROM event e
             JOIN event_data ed ON ed.id = e.latest_data_id
             JOIN account a ON a.id = e.account_id
             JOIN currency c ON c.id = a.currency_id
             WHERE e.deleted_at IS NULL
               AND ed.bucket_id IS NOT NULL
               AND ed.event_date <= ?1
             GROUP BY ed.bucket_id, a.currency_id",
        )?;

        let tagged_rows: Vec<(i64, i64, i64, i64)> = stmt
            .query_map(params![selected_datetime], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for (bucket_account_id, currency_id, minor_units, tagged_total) in tagged_rows {
            let bucket_idx = match account_index_map.get(&bucket_account_id) {
                Some(&idx) => idx,
                None => continue,
            };

            let converted = if currency_id == consolidation.id {
                tagged_total
            } else {
                match get_fx_rate_for_conversion(
                    conn,
                    GetFxRateForConversionParams {
                        from_currency_id: consolidation.id,
                        to_currency_id: currency_id,
                        date: snapshot_date.to_owned(),
                    },
                )? {
                    Some((mantissa, exponent, is_direct)) => convert_balance(
                        tagged_total,
                        mantissa,
                        exponent,
                        minor_units,
                        consolidation.minor_units,
                        is_direct,
                    ),
                    None => {
                        result[bucket_idx].fx_rate_missing = true;
                        convert_balance(
                            tagged_total,
                            1,
                            0,
                            minor_units,
                            consolidation.minor_units,
                            false,
                        )
                    }
                }
            };

            // TODO: This double-counts if the same source account is also contributing via
            // bucket_event_link (Pass 2). The UI wizard should prevent this scenario.
            result[bucket_idx].converted_balance_minor += converted;
            result[bucket_idx].cashflow_tagged_minor += converted;
        }
    }

    Ok(result)
}
