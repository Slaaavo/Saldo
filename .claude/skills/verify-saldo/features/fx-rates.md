# FX rates

The FX Rates page lists the stored exchange rates from EUR, the consolidation currency, to every other active currency, one row per date. The dashboard converts foreign-currency balances with the latest rate on or before the selected date. Users can type a rate by hand (marked `manual`), fetch rates for one date from Open Exchange Rates, and backfill dates that have balance updates but no rate.

## Sub-features

- `fx-list` shows rates per date and currency, relative to EUR, with manual ones marked.
- `fx-manual` edits a rate cell and stores it as manual.
- `fx-fetch` fetches rates for the date in `Fetch for date` with `Refresh Rates` (needs an OXR API key).
- `fx-backfill` fetches all missing dates with `Backfill <n> missing date(s)`, shown only when some are missing.
- `fx-startup` fetches missing recent rates in the background at app start, when a key is saved.

## How to get to it (user POV)

- Sidebar → `FX Rates`.
- Settings → Integrations → `Open Exchange Rates API Key` → `Save API Key` enables fetching.
- Dashboard → yellow `Missing FX rate for …` banner, which is the symptom when no rate covers the date.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), in demo mode, which seeds manual EUR→BTC rates on the last day of each of the three previous months.
- No OXR key is saved unless the check is about `fx-fetch` or `fx-startup`. Fetching uses the network and the user's OXR quota.

- **List.** Click `AXLink "FX Rates"`. The page reads `Rates relative to EUR`, with a `BTC` column and one row per seeded month-end, each `(manual)`. On 2026-10-18 those were `2026-09-30 0.0000108981`, `2026-08-31 0.0000113379`, and `2026-07-31 0.0000121309`. Keep the screenshot.
- **Edit a rate (`fx-manual`).** Click a rate cell. It turns into an input. Type the new rate and press `return` with `app_key`. `escape` cancels. The row keeps `(manual)` and shows the new value.
- **Effect on the dashboard.** Open `AXLink "Dashboard"`, set the date to that month-end ([Dashboard snapshot](./dashboard-snapshot.md)), and compare `NET WORTH` with the screenshot from before the edit. The BTC Wallet's EUR share moves with the rate.
- **Fetch (`fx-fetch`).** With a key saved, click `AXButton "Refresh Rates"`. New non-manual rows appear for that date. Without a key, an error toast says the API key is not configured.
- **Stored rows (dev DB only).** Run `evidence.sh db <nn>-fx "select date, to_currency_id, rate_mantissa, rate_exponent, is_manual from fx_rate order by date desc limit 10"`.

## Gotchas

- Last driven end to end on 2026-10-18: `fx-list` only, in demo mode. `fx-manual`, `fx-fetch`, `fx-backfill` and `fx-startup` have not been driven yet.
- The `Fetch for date` picker has no linked label, so it can't be opened from the background. Use display-scope tools with the user's approval, or verify `fx-fetch` for today only.
- In demo mode, the OXR key is copied from the dev DB's settings, so a key saved there makes demo fetches real network calls too.
- A fresh dev DB has no foreign currencies in use, so the page shows `No FX rates stored yet.` until an account in another currency exists.
- Rates are stored as mantissa and exponent integers. Compare the displayed value, not a float you computed.
