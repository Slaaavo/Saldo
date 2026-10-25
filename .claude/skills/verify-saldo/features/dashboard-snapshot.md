# Dashboard snapshot, time machine and number breakdowns

The dashboard opens live. It shows every account, bucket and asset with everything Saldo knows, including rows dated after today, for all persons or the one picked in the person selector. Picking a date in the header enters the time machine, which shows balances as of the end of that day under an amber banner with `Back to live`. A balance is the latest balance update on or before the moment, plus cashflows and transfers after it, converted to EUR with the latest stored rate on or before the moment. Each hero metric (Total Balance, or Net Worth and Liquid once an asset exists, plus Left to Spend once a bucket exists) opens a breakdown whose counted lines add up to it. An amount whose currency has no rate stays out of every total, and the rate warning panel names each account with no rate or with an old rate.

## Sub-features

- `snap-live` opens live: the header `Date` reads `Live`, and a row dated tomorrow counts in the cards, the metrics and the Ledger section.
- `snap-time-machine` picks a date: an amber banner names it, rows and rates dated later are left out, and `Back to live` or the `×` in the date trigger returns to live.
- `snap-breakdown` opens a headline number: counted lines with amount, rate, rate date and signed value add up to the total, and not-counted lines give the reason.
- `snap-fx` converts with the latest stored rate on or before the moment. An account with no rate is left out of the totals and named in the warning panel. A rate older than 7 days (31 for custom units such as VGLD) is named with its date and age and still counts.
- `snap-bulk-date` shows each balance in Bulk Balance Update as of that dialog's own date.
- `snap-person` filters the dashboard to one person with the person selector.

## How to get to it (user POV)

- Sidebar → `Dashboard`, which is also the start page.
- Header → `Date` picker (Dashboard only). Its calendar has no `Today` button.
- Amber time-machine banner → `Back to live`.
- Click a hero metric (`NET WORTH`, `LIQUID`, `TOTAL BALANCE`, `LEFT TO SPEND`).
- Ledger section → `Add Events` → `Bulk Balance Update`.
- Header → person selector, left of `Date` (Dashboard and Ledger, shown only when there is more than one person).

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md). `snap-live`, `snap-time-machine` and `snap-bulk-date` run on the dev DB, which needs at least one account with a balance. `snap-fx` currency staleness and the demo breakdowns run in demo mode: Settings → `Start Demo Mode`, or `Try Demo Mode` on an empty dashboard.
- The demo seed writes balance updates and EUR→BTC rates on the last day of each of the three months before today, plus one BTC rate dated today.
- `snap-person` runs on the dev DB, because adding a person fails in demo mode (see Gotchas).

- **Live by default (`snap-live`).** Open `AXLink "Dashboard"`. The header trigger `AXPopUpButton "Date"` reads `Live`, and there is no amber banner. In live demo mode there is no rate warning panel. Keep the screenshot.
- **A row dated tomorrow (`snap-live`, `snap-bulk-date`).** Click `Add Events`, then `Bulk Balance Update`. The dialog opens at today 23:59 and lists each balance under its name. Click the dialog's `Date` label to open its calendar, click an earlier day, and press `escape`: the listed balances change to that day's. Open the calendar again, click tomorrow, press `escape`, click a balance field, type a new amount, and click `Update`. The live dashboard card, `NET WORTH` and `LIQUID` include it. Prove it with `evidence.sh db` on `event_data.event_date` for that account. Scroll down: the Ledger section lists tomorrow's date above today's rows.
- **Enter the time machine (`snap-time-machine`).** The `Date` pop-up doesn't open from a background click, so click the `Date` label text just left of it. Click today's day. An amber strip reads `Time machine: balances as of <date>. Entries and rates dated later are left out.` with `AXButton "Back to live"`, and the row dated tomorrow no longer counts.
- **Back to live.** Click `Back to live` (a plain button, clicks normally). The trigger reads `Live` again and the metrics include tomorrow's row. See Gotchas about cards that keep their old paint.
- **Open a breakdown (`snap-breakdown`).** The metric cards are `AXPopUpButton "NET WORTH …"` and refuse a background click. Click blank space just above the metrics, or the warning text when the panel shows, then `app_key` `tab` once for the first metric, twice for the second, three times for the third, and `return`. A dialog titled with the metric and its total lists `COUNTED` lines (account, amount in its own currency, rate such as `1 EUR = 0.0000108981 BTC` with its date and a `manual` tag, value with sign) and a `Total` row, then `NOT COUNTED` lines with the reason. `escape` closes it. In live demo, `LIQUID` is Checking Account + Credit Card + BTC Wallet, with Mortgage (linked to an asset), House (asset) and the three buckets not counted. `LEFT TO SPEND` subtracts each bucket part, listed as `Additional balance` or `via <account>`.
- **No rate (`snap-fx`).** Click `Add Account`. `Currency` is a searchable text field, not a pop-up: click it, type `USD`, and click the `USD United States Dollar` option. Name the account `Verify USD`, set Initial Balance to `100`, and click `Create`. The warning panel under the header reads `Verify USD (100.00 USD) has no USD rate, so it is not counted in the totals.`, the metrics don't change, and every breakdown lists `Verify USD` under `NOT COUNTED` with `No USD rate, so it cannot be converted.` An account whose balance is 0 raises no warning.
- **Old currency rate (`snap-fx`, demo).** Enter the time machine 8 days after the latest seeded month-end. The panel reads `BTC Wallet (0.09200000 BTC) uses a BTC rate from <month-end>, 8 days old.` Pick 7 days after it: no panel. Both need a picked day before today.
- **Old custom-unit price (`snap-fx`, dev DB).** On an asset in a custom unit, click `UPDATE VALUE`, type a new `Price per unit`, open the date calendar (click the price field, `app_key` `tab`, `return`), go back a month or more, click a day, and `Save`. In the time machine 32 or more days after that date the panel names the asset with the price date and its age, and the old price still counts. Within 31 days the breakdown shows the price with no age tag.
- **Add a person (`snap-person`, dev DB).** Sidebar → `Persons` → `Add Person` → `Create Person`. Type `Verify Partner` in `Name`, keep `Type` `Physical`, and click `Create Person`. On the dashboard, the header now has a person select reading `All Persons` left of `Date`, and every card gains an owner line.
- **Filter to the new person.** The select doesn't open from a background click. Click the blank title row (for example `(600, 88)`, between `Dashboard` and the select), then `app_key` `tab`, `down` (the list opens with `All Persons`, `Personal`, `Verify Partner`, starting on the current value), `down`, `down`, `return`. The header reads `Verify Partner`, and the page shows only that person's accounts. Query `select a.name, p.name from account a join person p on p.id = a.person_id where a.account_type in ('account','bucket')` as the stored proof.

## Gotchas

- Last driven end to end on 2026-10-25 in run `20261025-172732-checkable-numbers`: `snap-live`, `snap-time-machine`, `snap-breakdown`, `snap-fx` (no rate, the 7/8-day BTC boundary in demo, the 31-day VGLD price on the dev DB) and `snap-bulk-date` are verified. `snap-person` was last driven on 2026-10-18 in run `20261018-213407-claude-keen-boyd-d370a7` and its path did not change.
- After a date change or `Back to live`, the horizontally scrolling account row can keep painting the previous balances in a background window while the metrics already show the new ones. Scrolling doesn't fix it. Opening and closing a breakdown repaints it. Trust the metrics and the breakdown, and repaint before you keep a card screenshot.
- `UPDATE VALUE` stores a balance update only when the quantity changes. With an unchanged quantity it stores just the price under the picked date.
- The demo seed's values depend on today's date, so expected numbers are only fixed per month. Recompute them from `src-tauri/src/demo.rs` when the month changes.
- In demo mode, `Create Person` closes the dialog but adds no person, and no selector appears. Drive `snap-person` on the dev DB.
- The dev DB holds a second person, so the person select and the owner lines show on every later run. Take a fresh screenshot before any coordinate click.
- While a calendar is open, the AX summary is truncated. Click days by screenshot coordinates. The amber time-machine banner and the demo banner each push the page down about 53 px.
- A time-machine date is end of day (`T23:59:59`), so an update dated that day counts.
- The Ledger section under the cards shows the 20 newest cards, where a split group counts as one card and always shows all its legs. Live, it has no upper date bound.
- The time machine and the person filter live in app state, not the URL. Leaving the dashboard and coming back keeps them until the app restarts or the window reloads.
