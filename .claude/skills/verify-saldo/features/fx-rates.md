# FX rates

The FX Rates page lists the stored exchange rates from EUR, the consolidation currency, to every other currency that has stored rates, one row per date. The dashboard converts foreign-currency balances with the newest stored rate when live, and with the latest rate on or before the picked date in the time machine. Users can type a rate by hand (marked `manual`), fetch rates for one date from Open Exchange Rates, and backfill dates that have balance updates but no rate.

## Sub-features

- `fx-list` shows rates per date and currency, relative to EUR, with manual ones marked.
- `fx-manual` edits a rate cell and stores it as manual.
- `fx-fetch` fetches rates for the date in `Fetch for date` with `Refresh Rates` (needs an OXR API key).
- `fx-backfill` fetches all missing dates with `Backfill <n> missing date(s)`, shown only when some are missing.
- `fx-startup` fetches missing recent rates in the background at app start, when a key is saved.

## How to get to it (user POV)

- Sidebar → `FX Rates`.
- Settings → Integrations → `Open Exchange Rates API Key` → `Save API Key` enables fetching.
- Dashboard → amber rate warning panel (`… has no USD rate, so it is not counted in the totals.` or `… uses a … rate from <date>, N days old.`), which is the symptom when no rate covers the moment or the rate is old.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), in demo mode, which seeds manual EUR→BTC rates on the last day of each of the three previous months and one dated today (equal to the latest month-end's).
- No OXR key is saved unless the check is about `fx-fetch` or `fx-startup`. Fetching uses the network and the user's OXR quota. Only the user may type their key into Settings, because the agent never enters API keys. Check with `sqlite3 "file:<dev DB>?mode=ro" "select key from app_setting"`: a fresh dev DB lists only `consolidation_currency_code`.

- **List (`fx-list`).** Click `AXLink "FX Rates"`. The page reads `Rates relative to EUR`, then `Fetch for date` with today's date and `Refresh Rates`. The table has a `BTC` column with a row for today and one per seeded month-end, each `(manual)`. On 2026-10-18 the month-ends were `2026-09-30 0.0000108981`, `2026-08-31 0.0000113379`, and `2026-07-31 0.0000121309`. Keep the screenshot.
- **Before the edit.** On the live dashboard, note `NET WORTH`, `LIQUID`, and the `Retirement Fund` bucket, which mirrors `BTC Wallet`. On 2026-10-25 they were `215 851.84`, `12 851.84`, and `8 441.84 EUR` (0.092 BTC at `0.0000108981`).
- **Edit a rate (`fx-manual`).** On FX Rates, click today's BTC cell, because live uses the newest rate. To edit a month-end instead, prove it in the time machine on a day before today's rate. It turns into a small `AXTextField` (about 97×22) holding the current rate. Triple-click it, type `0.00001`, keep `<nn>-fx-edit-input`, and press `app_key` `return`. The cell reads `0.00001 (manual)`. `escape` cancels, and a blur saves.
- **Effect on the dashboard.** Open `AXLink "Dashboard"` (live). `Retirement Fund` reads `9 200.00 EUR` (0.092 / 0.00001), `LIQUID` reads `13 610.00`, and `NET WORTH` reads `216 610.00`, each +758.16. `LEFT TO SPEND` stays `1 100.00`, because the bucket rises by the same amount.
- **Fetch without a key (`fx-fetch`, negative path).** With no key saved, click `AXButton "Refresh Rates"`. No network call happens. An inline red box under the actions (not a toast) reads `API key not configured. Set oxr_app_id in settings.`
- **Fetch with a key (`fx-fetch`).** Only with a key the user saved: click `Refresh Rates`. New non-manual rows should appear for that date.
- **Stored rows (dev DB only).** Run `evidence.sh db <nn>-fx "select date, to_currency_id, rate_mantissa, rate_exponent, is_manual from fx_rate order by date desc limit 10"`.

## Gotchas

- Last driven end to end on 2026-10-18, in demo mode: `fx-list` (both runs) and `fx-manual` (run `20261018-213407-claude-keen-boyd-d370a7`), plus `fx-fetch`'s no-key error. Not verified: a successful `fx-fetch`, `fx-backfill`, and `fx-startup`. All three need the user's own OXR key, saved by the user, and their quota.
- Saving a balance update on a foreign-currency card with no rate for that day opens a `Confirm` prompt to fetch rates. Without an OXR key, `Confirm` shows an error toast that points to the key in Settings or to entering the rate here. The toast only paints with display-scope tools; in the background it is a collapsed `AXStaticText` at the bottom edge. Verified on the dev DB in run `20261025-081534-worktree-open-review-fixes`.
- `Backfill N missing date(s)` counts dates of every entry type on standard foreign-currency accounts, never custom units. To check it, give a custom-unit asset a quantity change on a day without a price (`Update Value` with the price unchanged and an earlier date): the count must not grow. Verified in run `20261025-081534-worktree-open-review-fixes`.
- The table has a column only for currencies that already have a stored rate. An account in a new currency (for example USD) adds no column, so there's no cell to type a first manual rate into. It only adds `Backfill 1 missing date(s)`, which needs a key.
- The `Fetch for date` picker has no linked label, so it can't be opened from the background. Use display-scope tools with the user's approval, or verify `fx-fetch` for today only.
- In demo mode, the OXR key is copied from the dev DB's settings, so a key saved there makes demo fetches real network calls too.
- A fresh dev DB has no foreign currencies in use, so the page shows `No FX rates stored yet.` until a rate is stored.
- Rates are stored as mantissa and exponent integers. Compare the displayed value, not a float you computed.
