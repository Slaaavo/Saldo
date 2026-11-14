//! The one rule for an account's balance, written once as SQL so every reader splices the same text.
//!
//! An account's balance is its latest visible `balance_update` (by `event_date`, then `created_at`)
//! plus the visible rows of `BALANCE_FLOW_TYPES` dated strictly after that anchor. Which rows are
//! visible is the caller's predicate, built over the aliases of each sub-select: the dashboard
//! snapshot sees rows dated up to its moment, a bank check sees rows that existed when it was made
//! or are dated before its day.

/// The event types that move an account's balance after its latest balance update.
pub(crate) const BALANCE_FLOW_TYPES: &str = "'cashflow', 'transfer', 'unexplained_difference'";

/// The account's latest visible balance update, selecting `column` of its `event_data` row.
/// A sub-select that is NULL when no balance update is visible.
pub(crate) fn latest_balance_update_sql(
    account: &str,
    visible: &impl Fn(&str, &str) -> String,
    column: &str,
) -> String {
    anchor_sql(account, visible, column, "e", "ed")
}

/// The account's balance under the predicate `visible(event_alias, event_data_alias)`.
pub(crate) fn balance_sql(account: &str, visible: impl Fn(&str, &str) -> String) -> String {
    let anchor_amount = latest_balance_update_sql(account, &visible, "amount_minor");
    let anchor_date = anchor_sql(account, &visible, "event_date", "e3", "ed3");
    let flow_visible = visible("e2", "ed2");
    format!(
        "COALESCE(
           {anchor_amount},
           0
         ) + COALESCE(
           (SELECT SUM(ed2.amount_minor)
            FROM event e2
            JOIN event_data ed2 ON ed2.id = e2.latest_data_id
            WHERE e2.account_id = {account}
              AND e2.deleted_at IS NULL
              AND e2.event_type IN ({BALANCE_FLOW_TYPES})
              AND ({flow_visible})
              AND ed2.event_date > COALESCE(
                {anchor_date},
                ''
              )),
           0
         )"
    )
}

fn anchor_sql(
    account: &str,
    visible: &impl Fn(&str, &str) -> String,
    column: &str,
    e: &str,
    ed: &str,
) -> String {
    let anchor_visible = visible(e, ed);
    format!(
        "(SELECT {ed}.{column}
          FROM event {e}
          JOIN event_data {ed} ON {ed}.id = {e}.latest_data_id
          WHERE {e}.account_id = {account}
            AND {e}.deleted_at IS NULL
            AND {e}.event_type = 'balance_update'
            AND ({anchor_visible})
          ORDER BY {ed}.event_date DESC, {e}.created_at DESC
          LIMIT 1)"
    )
}
