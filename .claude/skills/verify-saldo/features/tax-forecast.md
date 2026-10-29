# Tax forecast

A tax model forecasts a person's year. Its result page is a sheet of five lines (Revenue, Payroll, Depreciation, Deductible costs, Non-deductible costs), each with the year so far, the rest of the year and the full year, and each value labelled with where it came from. Below the sheet, the summary shows the corporate income tax at the rate the year's revenue picks, the minimum tax when it applies, and, for a company, the dividend from the year's profit, profit left from earlier years, the maximum dividend and its tax. The user types the accountant's year-so-far totals on **Update figures**. Saldo projects the rest of the year from them.

## Sub-features

- `tf-settings` creates a company model with brackets, reserve fund, dividend rate and minimum tax bands (Adjust Settings or Create Tax Model).
- `tf-default-month` sets a new model's actual months to the last ended month for the current year, December for a past year, and none for a future year.
- `tf-schedule` fills the Depreciation line from the asset's depreciation schedule, split into actual and projected months.
- `tf-figures` types year-so-far totals, a monthly payroll amount and earlier unpaid profit per year on Update figures. The rest of the year is projected.
- `tf-typed-rest` types over a projected rest-of-year value. The cell shows `Typed`.
- `tf-bracket` picks a company's rate by revenue, with the upper bound inside the bracket (100,000.00 at 10%, 100,000.01 at 21%).
- `tf-minimum-tax` shows the minimum tax instead of a lower computed tax.
- `tf-breakdown` lists the entries behind the sheet and dims the ones that don't count, with the reason.
- `tf-placeholders` shows Saldo's own value in each empty field on Update figures, and hides it once the draft no longer matches what the value was computed from.
- `tf-prepaid` counts a prepaid expense's yearly share once, in the month it is dated, without averaging it.

## How to get to it (user POV)

- Tax Models → `Create Tax Model` → fill the form → `Save and Calculate`.
- Tax Models → a model name → the results page.
- Results page → `Update figures`.
- Results page → `Adjust Settings` → the Minimum tax card (company only).
- Dashboard → Assets → `Add Asset` (a company's asset in its consolidation currency shows the Depreciation section).

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), on the dev DB.
- A company person: Persons → `Add Person` → Name `Verify Co`. Click the Name field, then `tab`, `down`, `down`, `return` to choose Type `Legal` → `Create Person`.
- If `Verify Co` is the first company, the Create Tax Model form presets it as Person.

- **Create the model (`tf-settings`, `tf-default-month`).** Click `AXButton "Create Tax Model"`. Type the name. In Bracket 1, type Flat Rate `10` and Upper Bound `100000`. A Bracket 2 appears; type its Flat Rate `21`. Under Legal Person Settings, type Reserve Fund Current Amount `500`, Allocation % `10`, Maximum `500` and Dividend Tax Rate `7`. Under Minimum tax, click `AXButton "Add band"`. Type Up to revenue `50000` and Minimum tax `340`. A new open band appears after each revenue limit you type, so add `250000` / `960`, then `1920` for the open band. Scroll down and click `Save and Calculate`. The results page header reads `Actual January to September, projected October to December` on 29 October 2026, and the summary shows Total Tax `340.00 EUR` with `Minimum tax applies instead of the computed tax` and `0.00 EUR` beneath (`tf-minimum-tax`).
- **Add the asset (`tf-schedule`).** Dashboard → Assets → `AXButton "Add Asset"`. Type Asset name `Verify Car`. Click Asset name, then `tab`, `down`, `down`, `down`, `return` to choose Person `Verify Co`. The Depreciation section appears. Type Purchase price `16800`, click `4 yr (48)`, and set Purchase date to 1 January 2026 (see Gotchas). Click `Create`. Back on the model, Depreciation reads `3 150.00 EUR` Schedule, `1 050.00 EUR` Schedule, `4 200.00 EUR`.
- **Type the figures (`tf-figures`).** Click `AXButton "Update figures"`. The month select already reads `September`, and every input's placeholder is Saldo's own value. Type So far: Revenue `90000`, Payroll `36000`, Deductible costs `18000`, Non-deductible costs `1500`. Type Payroll per month `4000`. Under Profit left from earlier years, click `Add year` and type `2025`, `15000`, `7`. Click `Save`. The sheet reads 90 000.00 / 30 000.00 / 120 000.00, 36 000.00 / 12 000.00 / 48 000.00, 3 150.00 / 1 050.00 / 4 200.00, 18 000.00 / 6 000.00 / 24 000.00, and 1 500.00 / 500.00 / 2 000.00, with sources `Typed`, `Average`, `Monthly amount` and `Schedule`. The summary reads Corporate income tax at 21% `9 198.00`, Net Profit `32 602.00`, Dividend from 2026 profit `32 602.00` with dividend tax `2 282.14`, Profit left from 2025 `15 000.00` with tax `1 050.00`, Maximum dividend `47 602.00`, Withholding Tax `3 332.14`, Net Dividend `44 269.86` and Monthly Tax Burden `956.68`.
- **Type over a projection (`tf-typed-rest`).** Open Update figures again. The saved values come back filled. Type `5600` in Deductible costs, Rest of year, and click `Save`. That cell reads `5 600.00 EUR` `Typed`, and the tax becomes `9 282.00`.
- **Check the bracket bound (`tf-bracket`).** Create a 2025 model for `Verify Co` with the same brackets. Its header reads `Whole year actual`, and Update figures shows `December` with the Rest of year inputs disabled. Type So far: Revenue `100000`, Payroll `40000`, Deductible costs `20000`, Non-deductible costs `1000`. Save. The summary shows Corporate income tax at 10% `4 000.00`. Change Revenue to `100000.01` and save again. It shows Corporate income tax at 21% `8 400.00`.
- **Check the not-counted reason (`tf-breakdown`).** On the 2026 model, type `1100` in Depreciation, Rest of year, and save. Expand `Event Breakdown (EUR)`. The October to December depreciation rows are dimmed with `Replaced by a typed value`, and the Depreciation total reads `3 150.00 €`.
- **Check the placeholders (`tf-placeholders`).** Open Update figures without changing anything. The empty fields show Saldo's values (for example `30000.00` in Revenue, Rest of year). Type over Revenue, So far. Every Rest of year placeholder disappears, and the Depreciation So far placeholder stays. Change the month (click blank space right of the month label, then `tab`, `down`, `up`, `return`). Every placeholder disappears. Click `Cancel`.
- **Check a prepaid share (`tf-prepaid`).** Ledger → `Add Events` → the fourth tile (`Add Prepaid Expense`). Person is preset to `Verify Co` and Date to today. Open `Prepaid until` from its label, step to January 2027 and pick 31. Type Amount `1200` and click `Add Prepaid Expense`. On the 2026 model, clear Deductible costs So far and Rest of year (triple-click, `backspace`) and save. Deductible costs reads `0.00` From entries, `808.42` Schedule and `808.42` for the year, and Event Breakdown has a `Prepaid expense share` group with that row counted.
- **Stored rows.** Run `evidence.sh db <nn>-figures "select id, name, calendar_year, actuals_through_month, payroll_monthly_minor from tax_model; select tax_model_id, line, so_far_minor, rest_minor from tax_model_typed_value order by tax_model_id, line; select * from tax_model_earlier_profit; select * from tax_model_minimum_tax_band order by tax_model_id, lower_bound_minor"`. Typed values are minor units, and a cell left empty is `NULL`.

## Gotchas

- Last driven end to end on 2026-10-29, on the dev DB: the first eight sub-features in run `20261029-110045-tax-forecast`, then `tf-placeholders` and `tf-prepaid` in run `20261029-145933-tax-forecast`. All ten are verified.
- Toasts don't paint in a background window. A figures save that fails validation (a negative amount, a four-digit year missing) stays on the page with no visible message. Check `tax_model_typed_value` if a save seems to do nothing.
- The Purchase date calendar opens from its `Purchase date` label. Fast repeated clicks on the month arrow get dropped, and the popover moves up for months with six week rows, so the arrow sits about 45 px higher. Click one month at a time and take a small screenshot after each. Page Up does nothing, because the focus starts on `Today`.
- Year inputs on the earlier-profit row show a placeholder year. Type the year anyway, or the row is dropped as blank.
- Amounts show a narrow space as the thousands separator (`9 198.00 EUR`). Match on the digits.
- The Event Breakdown table scrolls sideways at a 1200 px window, so the Non-deductible column is off screen.
