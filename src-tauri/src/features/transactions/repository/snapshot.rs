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

/// Converts amounts to the consolidation currency at the snapshot date. A currency's rate is
/// looked up on first use and reused for every account, bucket link and tagged cashflow in it.
struct ConsolidationConverter<'a> {
    conn: &'a Connection,
    consolidation_id: i64,
    consolidation_minor_units: i64,
    date: &'a str,
    rates: HashMap<i64, Option<(i64, i64, bool)>>,
}

impl ConsolidationConverter<'_> {
    /// Returns the converted amount and whether no rate exists, in which case the amount is
    /// converted 1:1 (mantissa 1, exponent 0) and the caller flags it as `fx_rate_missing`.
    fn convert(
        &mut self,
        amount_minor: i64,
        currency_id: i64,
        minor_units: i64,
    ) -> rusqlite::Result<(i64, bool)> {
        if currency_id == self.consolidation_id {
            return Ok((amount_minor, false));
        }
        let rate = match self.rates.get(&currency_id) {
            Some(rate) => *rate,
            None => {
                let rate = get_fx_rate_for_conversion(
                    self.conn,
                    GetFxRateForConversionParams {
                        from_currency_id: self.consolidation_id,
                        to_currency_id: currency_id,
                        date: self.date.to_owned(),
                    },
                )?;
                self.rates.insert(currency_id, rate);
                rate
            }
        };
        Ok(match rate {
            Some((mantissa, exponent, is_direct)) => (
                convert_balance(
                    amount_minor,
                    mantissa,
                    exponent,
                    minor_units,
                    self.consolidation_minor_units,
                    is_direct,
                ),
                false,
            ),
            None => (
                convert_balance(
                    amount_minor,
                    1,
                    0,
                    minor_units,
                    self.consolidation_minor_units,
                    false,
                ),
                true,
            ),
        })
    }
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
    let mut converter = ConsolidationConverter {
        conn,
        consolidation_id: consolidation.id,
        consolidation_minor_units: consolidation.minor_units,
        date: snapshot_date,
        rates: HashMap::new(),
    };

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
        let (converted_balance_minor, fx_rate_missing) =
            converter.convert(balance_minor, currency_id, currency_minor_units)?;

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
    let all_bucket_links = list_all_latest_bucket_links(conn, selected_datetime)?;
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

        let (converted, fx_rate_missing) = converter.convert(
            result[source_idx].balance_minor,
            link.source_currency_id,
            link.source_currency_minor_units,
        )?;
        if fx_rate_missing {
            result[bucket_idx].fx_rate_missing = true;
        }

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
               AND ed.event_date > COALESCE(
                 (SELECT ed_bu.event_date
                  FROM event e_bu
                  JOIN event_data ed_bu ON ed_bu.id = e_bu.latest_data_id
                  WHERE e_bu.account_id = ed.bucket_id
                    AND e_bu.deleted_at IS NULL
                    AND e_bu.event_type = 'balance_update'
                    AND ed_bu.event_date <= ?1
                  ORDER BY ed_bu.event_date DESC, e_bu.created_at DESC
                  LIMIT 1),
                 '')
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

            let (converted, fx_rate_missing) =
                converter.convert(tagged_total, currency_id, minor_units)?;
            if fx_rate_missing {
                result[bucket_idx].fx_rate_missing = true;
            }

            // TODO: This double-counts if the same source account is also contributing via
            // bucket_event_link (Pass 2). The UI wizard should prevent this scenario.
            result[bucket_idx].converted_balance_minor += converted;
            result[bucket_idx].cashflow_tagged_minor += converted;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_in_memory;
    use crate::features::accounts::repository::{create_account, CreateAccountParams};
    use crate::features::buckets::repository::{set_bucket_event_links, SetBucketEventLinksParams};
    use crate::features::currency::repository::{set_fx_rate_manual, SetFxRateManualParams};
    use crate::features::transactions::repository::{
        create_balance_update, create_cashflow, CashflowEntry, CreateBalanceUpdateParams,
    };

    fn mk_account(conn: &Connection, account_type: &str) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: format!("Test {account_type}"),
                currency_id: 1,
                account_type: account_type.to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    #[test]
    fn bucket_links_on_a_timed_balance_update_show_on_that_day() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn, "account");
        let bucket_id = mk_account(&conn, "bucket");
        let event_id = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: bucket_id,
                amount_minor: 0,
                event_date: "2026-10-23T23:59:00".to_owned(),
                note: None,
            },
        )
        .unwrap();
        set_bucket_event_links(
            &conn,
            SetBucketEventLinksParams {
                event_id,
                account_ids: vec![source_id],
            },
        )
        .unwrap();

        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-10-23T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        let bucket = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();
        assert_eq!(bucket.bucket_links.len(), 1);
        assert_eq!(bucket.bucket_links[0].source_account_id, source_id);
        let source = snapshot.iter().find(|r| r.account_id == source_id).unwrap();
        assert!(source.is_bucket_linked);
    }

    fn currency_id(conn: &Connection, code: &str) -> i64 {
        conn.query_row(
            "SELECT id FROM currency WHERE code = ?1",
            params![code],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn mk_account_in(conn: &Connection, currency: &str, account_type: &str) -> i64 {
        create_account(
            conn,
            CreateAccountParams {
                name: format!("{currency} {account_type}"),
                currency_id: currency_id(conn, currency),
                account_type: account_type.to_owned(),
                ..Default::default()
            },
        )
        .expect("create account failed")
    }

    fn balance_update(conn: &Connection, account_id: i64, amount_minor: i64) -> i64 {
        create_balance_update(
            conn,
            CreateBalanceUpdateParams {
                account_id,
                amount_minor,
                event_date: "2026-03-01".to_owned(),
                note: None,
            },
        )
        .unwrap()
    }

    /// A cashflow dated after every balance update, tagged to `bucket_id`.
    fn tagged_cashflow(conn: &Connection, account_id: i64, bucket_id: i64, amount_minor: i64) {
        create_cashflow(
            conn,
            &CashflowEntry {
                account_id,
                amount_minor,
                event_date: "2026-03-02".to_owned(),
                note: None,
                counterpart_account_id: None,
                bucket_id: Some(bucket_id),
                original_currency_id: None,
                original_amount_minor: None,
                fx_rate_mantissa: None,
                fx_rate_exponent: None,
            },
        )
        .unwrap();
    }

    fn link_sources(conn: &Connection, bucket_id: i64, sources: &[i64]) {
        let event_id = balance_update(conn, bucket_id, 0);
        set_bucket_event_links(
            conn,
            SetBucketEventLinksParams {
                event_id,
                account_ids: sources.to_vec(),
            },
        )
        .unwrap();
    }

    fn snapshot_on_march_5th(conn: &Connection) -> Vec<SnapshotRow> {
        get_accounts_snapshot(
            conn,
            GetSnapshotParams {
                selected_datetime: "2026-03-05T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn row(snapshot: &[SnapshotRow], account_id: i64) -> &SnapshotRow {
        snapshot
            .iter()
            .find(|r| r.account_id == account_id)
            .unwrap()
    }

    #[test]
    fn several_accounts_in_one_foreign_currency_convert_at_the_same_rate() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (eur, usd) = (currency_id(&conn, "EUR"), currency_id(&conn, "USD"));
        // 1 EUR = 1.25 USD, so every USD minor unit converts at x 100 / 125.
        set_fx_rate_manual(
            &conn,
            SetFxRateManualParams {
                from_currency_id: eur,
                to_currency_id: usd,
                date: "2026-03-01".to_owned(),
                rate_mantissa: 125,
                rate_exponent: -2,
                is_direct: false,
            },
        )
        .unwrap();
        let eur_account = mk_account_in(&conn, "EUR", "account");
        let (a1, a2, a3, a4) = (
            mk_account_in(&conn, "USD", "account"),
            mk_account_in(&conn, "USD", "account"),
            mk_account_in(&conn, "USD", "account"),
            mk_account_in(&conn, "USD", "account"),
        );
        let bucket = mk_account_in(&conn, "EUR", "bucket");
        balance_update(&conn, eur_account, 7000);
        balance_update(&conn, a1, 12_500);
        balance_update(&conn, a2, 5_000);
        balance_update(&conn, a3, 333);
        // a4 has no balance update; its balance is the two tagged cashflows.
        tagged_cashflow(&conn, a4, bucket, 2_500);
        tagged_cashflow(&conn, a4, bucket, 1_000);
        link_sources(&conn, bucket, &[a1, a2, a3]);

        let snapshot = snapshot_on_march_5th(&conn);

        // Plain rows: 12500 -> 10000, 5000 -> 4000, 333 -> 266.4 -> 266, 3500 -> 2800.
        let converted = |id| {
            (
                row(&snapshot, id).converted_balance_minor,
                row(&snapshot, id).fx_rate_missing,
            )
        };
        assert_eq!(converted(eur_account), (7_000, false));
        assert_eq!(converted(a1), (10_000, false));
        assert_eq!(converted(a2), (4_000, false));
        assert_eq!(converted(a3), (266, false));
        assert_eq!(row(&snapshot, a4).balance_minor, 3_500);
        assert_eq!(converted(a4), (2_800, false));
        // Bucket: linked sources 10000 + 4000 + 266, tagged cashflows 3500 -> 2800.
        let bucket_row = row(&snapshot, bucket);
        assert_eq!(bucket_row.linked_balance_minor, 14_266);
        assert_eq!(bucket_row.cashflow_tagged_minor, 2_800);
        assert_eq!(bucket_row.converted_balance_minor, 17_066);
        assert!(!bucket_row.fx_rate_missing);
    }

    #[test]
    fn several_accounts_in_a_currency_without_a_rate_all_fall_back_to_one_to_one() {
        let conn = initialize_in_memory().expect("DB init failed");
        let (g1, g2, g3) = (
            mk_account_in(&conn, "GBP", "account"),
            mk_account_in(&conn, "GBP", "account"),
            mk_account_in(&conn, "GBP", "account"),
        );
        let bucket = mk_account_in(&conn, "EUR", "bucket");
        balance_update(&conn, g1, 1_000);
        balance_update(&conn, g2, 2_500);
        tagged_cashflow(&conn, g3, bucket, 300);
        tagged_cashflow(&conn, g3, bucket, 200);
        link_sources(&conn, bucket, &[g1, g2]);

        let snapshot = snapshot_on_march_5th(&conn);

        let converted = |id| {
            (
                row(&snapshot, id).converted_balance_minor,
                row(&snapshot, id).fx_rate_missing,
            )
        };
        assert_eq!(converted(g1), (1_000, true));
        assert_eq!(converted(g2), (2_500, true));
        assert_eq!(converted(g3), (500, true));
        let bucket_row = row(&snapshot, bucket);
        assert_eq!(bucket_row.linked_balance_minor, 3_500);
        assert_eq!(bucket_row.cashflow_tagged_minor, 500);
        assert_eq!(bucket_row.converted_balance_minor, 4_000);
        assert!(bucket_row.fx_rate_missing);
    }

    #[test]
    fn bucket_links_do_not_show_before_the_balance_update() {
        let conn = initialize_in_memory().expect("DB init failed");
        let source_id = mk_account(&conn, "account");
        let bucket_id = mk_account(&conn, "bucket");
        let event_id = create_balance_update(
            &conn,
            CreateBalanceUpdateParams {
                account_id: bucket_id,
                amount_minor: 0,
                event_date: "2026-10-23T09:00:00".to_owned(),
                note: None,
            },
        )
        .unwrap();
        set_bucket_event_links(
            &conn,
            SetBucketEventLinksParams {
                event_id,
                account_ids: vec![source_id],
            },
        )
        .unwrap();

        let snapshot = get_accounts_snapshot(
            &conn,
            GetSnapshotParams {
                selected_datetime: "2026-10-22T23:59:59".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();

        let bucket = snapshot.iter().find(|r| r.account_id == bucket_id).unwrap();
        assert!(bucket.bucket_links.is_empty());
    }
}
