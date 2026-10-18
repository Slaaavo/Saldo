# Dashboard snapshot by date

The dashboard shows every account, bucket and asset as of the date picked in the header, for all persons or the one picked in the person selector. A balance is the latest balance update on or before that date, plus cashflows and transfers after that update up to the date, converted to EUR with stored FX rates. The hero metrics (Total Balance, or Net Worth and Liquid once an asset exists, plus Left to Spend once a bucket exists) add those balances up.

## Sub-features

- `snap-today` opens on today's date with the latest balances.
- `snap-past` recomputes every card and metric for an earlier date.
- `snap-fx` converts a foreign-currency account with the rate stored for that date, and warns when a rate is missing.
- `snap-person` filters the dashboard to one person with the person selector.

## How to get to it (user POV)

- Sidebar → `Dashboard`, which is also the start page.
- Header → `Date` picker (Dashboard only).
- Header → person selector, left of `Date` (Dashboard and Ledger, shown only when there is more than one person).

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), in demo mode for `snap-today`, `snap-past` and `snap-fx`: Settings → `Start Demo Mode`, or `Try Demo Mode` on an empty dashboard.
- Today is after the seeded month-ends. The seed writes balance updates on the last day of each of the three months before today.
- `snap-person` runs on the dev DB, because adding a person fails in demo mode (see Gotchas).

- **Today's snapshot.** Open `AXLink "Dashboard"` and scroll to the top. `NET WORTH`, `LIQUID` and `LEFT TO SPEND` show, and the `Checking Account` card reads `4 820.00 EUR`, its latest seeded value. Keep the screenshot as `<nn>-snapshot-today`.
- **Open the date picker.** The `AXPopUpButton "Date"` doesn't open from a background click, so click the `Date` label text just left of it. A month calendar opens below. Take another screenshot if the AX count jumped but the calendar isn't painted yet.
- **Go back two months.** Click the `<` arrow at the calendar's top right twice. The caption reads two months before today's month.
- **Pick the month-end.** Click the last day of that month. The calendar closes, and the trigger shows the date, for example `31 August 2026`.
- **Read the past snapshot.** `Checking Account` reads `4 580.00 EUR`, `Credit Card` `-185.00 EUR`, `Emergency Fund` `3 095.00 EUR`, and `Holiday Savings` `200.00 EUR`, all matching that month's seed. Every metric changed. Keep it as `<nn>-snapshot-past`.
- **Back to today.** Open the calendar again and click `Today`.
- **FX conversion (`snap-fx`).** `BTC Wallet` shows its BTC amount (`0.09200000 BTC` today), and `NET WORTH`, `LIQUID` and the `Retirement Fund` bucket include it at the latest BTC rate stored on or before the picked date. On 2026-10-18: `0.092 / 0.0000108981 = 8 441.84 EUR`. To prove the rate is what's used, edit that rate on FX Rates and watch the same numbers move (see [FX rates](./fx-rates.md), `fx-manual`).
- **Missing rate (`snap-fx`).** Click `Add Account`. `Currency` is a searchable text field, not a pop-up: click it, type `USD`, and click the `USD United States Dollar` option. Name the account `Verify USD`, set Initial Balance to `100`, and click `Create`. A yellow banner under the header reads `Missing FX rate for USD. Using 1:1 rate.`, and `NET WORTH`, `LIQUID` and `LEFT TO SPEND` each rise by exactly `100.00`. The banner pushes the metrics down about 54 px.
- **Add a person (`snap-person`, dev DB).** Sidebar → `Persons` → `Add Person` → `Create Person`. Type `Verify Partner` in `Name`, keep `Type` `Physical`, and click `Create Person`. The list shows `Personal` (Default) and `Verify Partner`. On the dashboard, the header now has a person select reading `All Persons` left of `Date`, and every card gains an owner line (`Personal`).
- **Filter to the new person.** The select doesn't open from a background click. Click the blank title row (for example `(600, 88)`, between `Dashboard` and the select), then `app_key` `tab`, `down` (the list opens with `All Persons`, `Personal`, `Verify Partner`, starting on the current value), `down`, `down`, `return`. The header reads `Verify Partner`, `TOTAL BALANCE` reads `0.00 EUR`, and the page shows `No accounts yet` with `Try Demo Mode` and `Add Account`.
- **Give that person an account.** Click `Add Account`. The dialog now has a `Person` field preset to `Verify Partner`. Create `Verify Partner Cash` with `300`. The filtered dashboard shows only that card and `TOTAL BALANCE 300.00 EUR`.
- **Switch back.** Repeat the keyboard path with `up`, `up`, `return` for `All Persons`: `2 150.00 EUR` across three accounts with owner lines. For `Personal`, use one `down` from `All Persons`: `1 850.00` and `LEFT TO SPEND 1 150.00`, without the partner's account. Query `select a.name, p.name from account a join person p on p.id = a.person_id where a.account_type in ('account','bucket')` as the stored proof.

## Gotchas

- Last driven end to end on 2026-10-18. `snap-today` and `snap-past` ran in demo mode via the label click (run `20261018-212805-claude-stoic-vaughan-132554`). `snap-fx` ran in demo mode (conversion and the missing-rate banner), and `snap-person` ran on the dev DB with the keyboard path, both in run `20261018-213407-claude-keen-boyd-d370a7`. All four are verified.
- In demo mode, `Create Person` closes the dialog but adds no person, and no selector appears. An error toast probably fires, but toasts don't paint in a background window. Drive `snap-person` on the dev DB.
- The dev DB now holds a second person, so the person select and the owner lines show on every later run. Layout coordinates shift accordingly, so take a fresh screenshot first.
- While the calendar is open, the AX summary is truncated. Click days by screenshot coordinates.
- The demo banner shifts the page about 53 px. Take a screenshot after entering demo mode before you click anything.
- The selected date is end of day (`T23:59:59`), so an update dated that day counts.
- The date and the person filter live in app state, not the URL. Leaving the dashboard and coming back keeps them until the app restarts.
- The demo seed's values depend on today's date, so expected numbers are only fixed per month. Recompute them from `src-tauri/src/demo.rs` when the month changes.
