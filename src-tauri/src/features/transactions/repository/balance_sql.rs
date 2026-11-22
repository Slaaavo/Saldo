//! The one rule for an account's balance, written once as SQL so every reader splices the same text.
//!
//! An account's balance is its latest visible `balance_update` (by `event_date`, `created_at`, `id`)
//! plus the visible rows of `BALANCE_FLOW_TYPES` dated strictly after that anchor. An unexplained
//! difference dated at the anchor's own moment also counts when it was entered after the anchor:
//! booking dates it at a balance update that is dated later than now, and a balance update entered
//! after the booking becomes the anchor and replaces it. Which rows are visible is the caller's
//! predicate, built over the aliases of each sub-select: the dashboard snapshot sees rows dated up
//! to its moment, a bank check sees rows that existed when it was made or are dated before its day.
//! How a visible flow row counts is the caller's `FlowCount`: the snapshot counts the rows that are
//! not deleted at their current amount, a bank check counts a card hold as it was at the check.

/// The event types that move an account's balance after its latest balance update.
pub(crate) const BALANCE_FLOW_TYPES: &str = "'cashflow', 'transfer', 'unexplained_difference'";

/// How a visible flow row counts, as SQL over its `event` and `event_data` aliases.
pub(crate) struct FlowCount {
    /// Whether the row is there at all.
    pub live: fn(&str, &str) -> String,
    /// What the row adds to the balance.
    pub amount: fn(&str, &str) -> String,
}

/// A flow row counts while it is not deleted, at its current amount.
pub(crate) const AS_STORED: FlowCount = FlowCount {
    live: not_deleted,
    amount: current_amount,
};

fn not_deleted(e: &str, _ed: &str) -> String {
    format!("{e}.deleted_at IS NULL")
}

fn current_amount(_e: &str, ed: &str) -> String {
    format!("{ed}.amount_minor")
}

/// The account's latest visible balance update, selecting `column` of its `event_data` row.
/// A sub-select that is NULL when no balance update is visible.
pub(crate) fn latest_balance_update_sql(
    account: &str,
    visible: &impl Fn(&str, &str) -> String,
    column: &str,
) -> String {
    anchor_sql(account, visible, &format!("ed.{column}"), "e", "ed")
}

/// The account's balance under the predicate `visible(event_alias, event_data_alias)`, each
/// counted flow row adding what `flow` says.
pub(crate) fn balance_sql(
    account: &str,
    visible: impl Fn(&str, &str) -> String,
    flow: &FlowCount,
) -> String {
    let anchor_amount = latest_balance_update_sql(account, &visible, "amount_minor");
    let counted = counted_flow_sql(account, &visible, flow, "e2", "ed2");
    let amount = (flow.amount)("e2", "ed2");
    format!(
        "COALESCE(
           {anchor_amount},
           0
         ) + COALESCE(
           (SELECT SUM({amount})
            FROM event e2
            JOIN event_data ed2 ON ed2.id = e2.latest_data_id
            WHERE {counted}),
           0
         )"
    )
}

pub(crate) fn counted_flow_sql(
    account: &str,
    visible: &impl Fn(&str, &str) -> String,
    flow: &FlowCount,
    e: &str,
    ed: &str,
) -> String {
    let anchor_date = anchor_sql(account, visible, "ed3.event_date", "e3", "ed3");
    let anchor_moment = anchor_sql(
        account,
        visible,
        "ed3.event_date, e3.created_at, e3.id",
        "e3",
        "ed3",
    );
    let flow_visible = visible(e, ed);
    let flow_live = (flow.live)(e, ed);
    format!(
        "{e}.account_id = {account}
              AND ({flow_live})
              AND {e}.event_type IN ({BALANCE_FLOW_TYPES})
              AND ({flow_visible})
              AND ({ed}.event_date > COALESCE(
                     {anchor_date},
                     ''
                   )
                   OR ({e}.event_type = 'unexplained_difference'
                       AND ({ed}.event_date, {e}.created_at, {e}.id) > {anchor_moment}))"
    )
}

fn anchor_sql(
    account: &str,
    visible: &impl Fn(&str, &str) -> String,
    select: &str,
    e: &str,
    ed: &str,
) -> String {
    let anchor_visible = visible(e, ed);
    format!(
        "(SELECT {select}
          FROM event {e}
          JOIN event_data {ed} ON {ed}.id = {e}.latest_data_id
          WHERE {e}.account_id = {account}
            AND {e}.deleted_at IS NULL
            AND {e}.event_type = 'balance_update'
            AND ({anchor_visible})
          ORDER BY {ed}.event_date DESC, {e}.created_at DESC, {e}.id DESC
          LIMIT 1)"
    )
}
