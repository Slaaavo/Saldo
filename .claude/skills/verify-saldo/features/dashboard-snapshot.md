# Dashboard snapshot by date

The dashboard shows every account, bucket and asset as of the date picked in the header. A balance is the latest balance update on or before that date, plus cashflows and transfers after that update up to the date, converted to EUR with stored FX rates. The hero metrics (Total Balance, or Net Worth and Liquid once an asset exists, plus Left to Spend once a bucket exists) add those balances up.

## Sub-features

- `snap-today` opens on today's date with the latest balances.
- `snap-past` recomputes every card and metric for an earlier date.
- `snap-fx` converts a foreign-currency account with the rate stored for that date, and warns when a rate is missing.
- `snap-person` filters the dashboard to one person with the person selector.

## How to get to it (user POV)

- Sidebar → `Dashboard`, which is also the start page.
- Header → `Date` picker (Dashboard only).
- Header → person selector (Dashboard and Ledger, shown when there is more than one person).

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), in demo mode: Settings → `Start Demo Mode`, or `Try Demo Mode` on an empty dashboard.
- Today is after the seeded month-ends. The seed writes balance updates on the last day of each of the three months before today.

- **Today's snapshot.** Open `AXLink "Dashboard"` and scroll to the top. `NET WORTH`, `LIQUID` and `LEFT TO SPEND` show, and the `Checking Account` card reads `4 820.00 EUR`, its latest seeded value. Keep the screenshot as `<nn>-snapshot-today`.
- **Open the date picker.** The `AXPopUpButton "Date"` refuses background clicks, so click the `Date` label text just left of it. A month calendar opens below. Take another screenshot if the AX count jumped but the calendar isn't painted yet.
- **Go back two months.** Click the `<` arrow at the calendar's top right twice. The caption reads two months before today's month.
- **Pick the month-end.** Click the last day of that month. The calendar closes, and the trigger shows the date, for example `31 August 2026`.
- **Read the past snapshot.** `Checking Account` reads `4 580.00 EUR`, `Credit Card` `-185.00 EUR`, `Emergency Fund` `3 095.00 EUR`, and `Holiday Savings` `200.00 EUR`, all matching that month's seed. Every metric changed. Keep it as `<nn>-snapshot-past`.
- **FX conversion.** `BTC Wallet` shows its BTC amount. `Net Worth` and `Liquid` include it at the latest BTC rate stored on or before the picked date (see the FX Rates page). The yellow banner `Missing FX rate for BTC. Using 1:1 rate.` appears only when no BTC rate exists on or before that date.
- **Back to today.** Open the calendar again and click `Today`.

## Gotchas

- Last driven end to end on 2026-10-18: `snap-today` and `snap-past`, in demo mode, via the label click. `snap-fx` and `snap-person` have not been driven yet.
- While the calendar is open, the AX summary is truncated. Click days by screenshot coordinates.
- The demo banner shifts the page about 53 px. Take a screenshot after entering demo mode before you click anything.
- The selected date is end of day (`T23:59:59`), so an update dated that day counts.
- The date lives in app state, not the URL. Leaving the dashboard and coming back keeps it until the app restarts.
- The demo seed's values depend on today's date, so expected numbers are only fixed per month. Recompute them from `src-tauri/src/demo.rs` when the month changes.
