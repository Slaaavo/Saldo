# Income budget

An income budget is a bucket for one person whose value Saldo computes instead of storing. The user types a budget without VAT and a VAT rate for each month in a horizontal month table. For every budgeted month that has started, Saldo takes the money that arrived that month on the person's accounts in the budget's currency, removes the month's VAT, and subtracts the budget. The bucket's value is the sum of those differences, and Left to Spend subtracts it like any bucket, so a missing payment raises Left to Spend above the cash and a payment above budget lowers it.

## Sub-features

- `income-budget-create` adds an income budget from `Add Bucket` with the `Income budget` kind, and refuses one without a person.
- `income-budget-card` shows the computed value on a bucket card with `EDIT BUDGET` instead of `UPDATE BALANCE`, and the bucket switch.
- `income-budget-table` opens a wide dialog with one column per month (`Nov 2026`), rows Budget, VAT, Received and Difference, and the dashboard's moment marked `TODAY` or `AS OF 15 OCT`.
- `income-budget-type` types budgets and VAT rates, and shows Received, Difference and the value as you type.
- `income-budget-fill` copies a cell into neighbouring months by dragging its corner square.
- `income-budget-years` opens on the years that hold values, and `+ 2025` / `+ 2027` add a year at either end.
- `income-budget-save` stores only typed months. Cancel stores nothing, and an invalid cell blocks Save.
- `income-budget-example` reproduces the agreed example: `-6 315.00 EUR` on 2026-11-08.
- `income-budget-left-to-spend` shows the value in Left to Spend and its breakdown as `Received minus budget`.
- `income-budget-counted-accounts` names the accounts whose money counts, under the table.
- `income-budget-past` shows the value and the table at a time-machine date.

## How to get to it (user POV)

- Dashboard → Buckets → `Add Bucket` → Kind `Income budget`.
- Dashboard → an income budget card → `EDIT BUDGET`.
- Dashboard → hero metrics → `LEFT TO SPEND` → the income budget's line.
- Dashboard → `Date` → a past day, then `EDIT BUDGET`.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), on the dev DB.
- Load the fixture while the app runs, with the sandbox disabled: `sqlite3 "$HOME/Library/Application Support/com.teamsumbal.saldo.dev/saldo.db" < .claude/skills/verify-saldo/fixtures/income-budget-inflows.sql`. It adds the person `Verify Consulting`, the EUR account `Verify Income`, and four cashflows on it: 2026-07-09 3 505.50, 2026-08-14 6 626.01, 2026-09-10 6 741.63 and 2026-10-12 7 204.11. Running it again adds nothing. The CSV import would be the user path for these rows, but it needs display-scope tools, and the rows are only the setup here. The open window shows `Updated with changes made outside Saldo` and reloads.
- No income budget exists yet for that person. A person can have only one.

- **Create (`income-budget-create`).** Click `AXButton "Add Bucket"`. `Create Bucket` opens with a `Kind` group of two toggles, `Normal` (pressed) and `Income budget`. Click `Income budget`. `Initial Balance` disappears and `Person` reads `Pick a person`. Type `Verify Consulting Budget` into `Bucket Name` and click `Create`. The dialog stays and shows "Pick the person whose income this budgets." under Person. Pick the person by keyboard: click the visible `Person` label, `app_key` `down` opens the list (`Personal`, …, `Verify Consulting`), `down` moves, `return` picks. Count the presses from the list's order. Click `Create`. A card `Verify Consulting Budget`, `0.00 EUR`, `EDIT BUDGET` and a switch appears under Buckets.
- **Open the table (`income-budget-table`, `income-budget-counted-accounts`).** Click the card's `EDIT BUDGET`. The dialog title is the bucket name with a person chip. The summary reads `0.00 EUR`, "This bucket on 8 November 2026, with the values below" and "Left to Spend equals your cash." The table shows `May 2026` to `Dec 2026`, with `Nov 2026` marked `TODAY` and its Received `0.00 so far`. Received shows the fixture's months greyed, because they have no budget yet: `3 505.50`, `6 626.01`, `6 741.63`, `7 204.11`. Under the table: "Counts money arriving on Verify Income."
- **Type (`income-budget-type`).** Click the July Budget cell and type `2850`, then July VAT `23`, August Budget `5640` and November Budget `6120`. Click a cell before typing into it. July Received turns `2 850.00` with Difference `0.00`. August shows `6 626.01` and `+986.01`, because its empty VAT counts as 0 %. November shows `-6 120.00 so far`.
- **Fill (`income-budget-fill`).** Click the August Budget cell. Its corner square, `Fill budget from August 2026`, shows only on the focused cell. `app_drag` from the square (the cell's bottom-right corner) along a `path` with intermediate points to the October column, for example `[[651,382],[700,372],[780,368],[805,366]]`. September and October read `5640`, and the line under the table says `Copied "5640" into 2 more months.` Drag the July VAT square to December and the November Budget square to December the same way.
- **The example (`income-budget-example`).** With July 2 850, August to October 5 640, November and December 6 120, and VAT 23 everywhere, Received reads `2 850.00`, `5 387.00`, `5 481.00`, `5 857.00`, `0.00 so far`. Difference reads `0.00`, `-253.00`, `-159.00`, `+217.00`, `-6 120.00 so far`. The value is `-6 315.00 EUR`, with "Left to Spend shows 6 315.00 EUR more than your cash. November's budget counts from the 1st, and its payment has not arrived yet."
- **Years (`income-budget-years`).** Click `+ 2027`. The table scrolls to the end, the button becomes `+ 2028`, and the line reads "2027 added. It is kept only if you type a value into it and save." Click `+ 2025`. The table scrolls to `Jan 2025`.
- **Save (`income-budget-save`).** Click `Save`. The dialog closes, the card reads `-6 315.00 EUR` in red, and `LEFT TO SPEND` rises by 6 315.00 (on 2026-11-08: `25 170.25 → 31 485.25`). Run `evidence.sh db <nn>-months "select a.name, a.bucket_kind, m.month, m.budget_minor, m.vat_bps from income_budget_month m join account a on a.id = m.bucket_id order by m.month"`. It lists only `2026-07` to `2026-12`, with `285000`, `564000`, `612000` and `2300`. The added years are not stored. Reopen `EDIT BUDGET`. Only 2026 shows, with the saved values.
- **Invalid cell.** Type `1,234.50` into an empty Budget cell. It turns red, and the value does not change. `Save` keeps the dialog open with the toast "Budget, May 2026: type an amount without a sign, like 5640 or 5640.50." Click `Cancel`. Nothing is stored.
- **Left to Spend (`income-budget-left-to-spend`).** Open `LEFT TO SPEND` by keyboard (blank click above the metrics, three `tab`s, `return`). The income budget is a counted line `Verify Consulting Budget`, `Received minus budget`, `-6 315.00 EUR`, `+6 315.00 EUR`.
- **A past date (`income-budget-past`).** Click the `Date` label, go back one month with the calendar arrow, and click `15`. `LEFT TO SPEND` becomes `LIQUID + 195.00`. `EDIT BUDGET` shows `-195.00 EUR`, "This bucket on 15 October 2026", `Oct 2026` marked `AS OF 15 OCT` with Received `5 857.00 so far`, and empty Received for November and December.

## Gotchas

- Last driven end to end on 2026-11-08, on the dev DB in run `20261108-195757-feat-income-budget-bucket`: all sub-features, in the background, plus one foreground check with display-scope tools for `income-budget-years`. The fixture rows were inserted by SQL, and the income budget's person in that run was `Verify Co`.
- **The fill drag works with `app_drag`** when the `path` has intermediate points. A two-point drag may skip the move events the fill reads. The square takes pointer events only while its cell's input has focus, so click the cell first.
- **A horizontal scroll can paint at a stale offset.** Before the fix in this branch, `+ 2025` painted `May 2025` first while the scrollbar sat at the start, in the background and in the foreground. If the table looks shifted or blank after a year is added, check once in the foreground before calling it a paint artifact.
- **The AX tree truncates** in this dialog (over 200 elements), and the table's input names show as `(title withheld)`. Use screenshot coordinates for cells. Each column is 92 px wide.
- The income budget card has no `UPDATE BALANCE`. That the other entry points (Bulk Balance Update, the CSV import's account and bucket lists, the ledger's portfolio filter) leave it out is covered by Vitest and the storage refusals by cargo tests. None of them was driven in the app.
- The dev DB already holds `Verify Consulting Budget` for `Verify Co` and the `Verify Income` account from the 2026-11-08 run, so the fixture adds only the `Verify Consulting` person there. Run `reset-dev-db.sh` first for a clean create.
