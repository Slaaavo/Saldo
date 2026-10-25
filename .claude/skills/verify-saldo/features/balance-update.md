# Balance update

A balance update records what an account, bucket, or asset holds on a date. The card on the dashboard shows the latest balance on or before the selected date, the hero metrics add it up, and the Ledger lists every update with its note.

## Sub-features

- `bu-card` sets one account's balance from its dashboard card (`UPDATE BALANCE`).
- `bu-initial` stores an initial balance when an account is created with one.
- `bu-bulk` sets several balances at once (Add Events → Bulk Balance Update).
- `bu-edit` changes an existing update from its ledger row (pencil).
- `bu-delete` removes an update from its ledger row (trash) after a confirmation.

## How to get to it (user POV)

- Dashboard → an account card → `UPDATE BALANCE`.
- Dashboard → Accounts → `Add Account` with an Initial Balance.
- Dashboard or Ledger → `Add Events` → `Bulk Balance Update`.
- Dashboard or Ledger → an event row → pencil (edit) or trash (delete).

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), on the dev DB.
- No account named `Verify Checking` yet. Otherwise use a new name, or run `reset-dev-db.sh` while the app is down.
- For `bu-bulk`, a second account helps show that every account gets its own input. Create `Verify Savings` with Initial Balance `500` the same way as `Verify Checking`.

- **Create the account.** Click `AXButton "Add Account"`. A `Create Account` dialog opens. Click `AXTextField "Account Name"`, type `Verify Checking`. Click `AXTextField "Initial Balance (optional)"`, type `1000`. Click `AXButton "Create"`. The dialog closes, and the dashboard shows the card `Verify Checking` with `1 000.00 EUR` and `TOTAL BALANCE 1 000.00 EUR`.
- **Open Update Balance from the card.** Click the card's `AXButton "UPDATE BALANCE"`. The `Update Balance` dialog opens with Account preset to `Verify Checking` and Date set to today. Leave both pop-ups alone.
- **Enter the new balance.** Click `AXTextField "Amount"`, type `1250.50`. Click `AXTextField "Note"`, type `verify-saldo`. Save `app_screenshot` (`scale: 1`) as `<nn>-update-form` with `evidence.sh keep`.
- **Save.** Click `AXButton "Save"`. The dialog closes. The card and `TOTAL BALANCE` both read `1 250.50 EUR`. Keep that screenshot as `<nn>-dashboard-after`.
- **Second view.** Click `AXLink "Ledger"`. Under today's date header there are two `Verify Checking` rows: `1 250.50 EUR` with note `verify-saldo`, and `1 000.00 EUR`. Keep it as `<nn>-ledger-after`.
- **Stored rows.** Run `evidence.sh db <nn>-balance-rows "select e.id, a.name, e.event_type, d.amount_minor, d.event_date, d.note from account a join event e on e.account_id = a.id join event_data d on d.id = e.latest_data_id where a.name like 'Verify %' and e.deleted_at is null order by e.id"`. It shows two `Verify Checking` `balance_update` rows, `100000` and `125050`, with today's `event_date`.
- **Bulk entry (`bu-bulk`).** On the Ledger page, click `AXButton "Add Events"`. An `Add Events` dialog opens with four tiles: `Bulk Balance Update`, `Revenue`, `Expense`, and `Add Prepaid Expense`. Their AX titles are withheld, so click the first tile by coordinate. `Update All Balances` opens with `Date` set to today, an `ACCOUNTS` section, and `Note`. Each account row shows the name with its current balance underneath, and an unnamed `AXTextField` to the right whose placeholder is `New balance`. Click the `Verify Checking` input and type `1300`. Click the `Verify Savings` input and type `650`. Click `AXTextField "Note"` and type `verify-saldo bulk`. Keep `<nn>-bulk-form`, then click `AXButton "Update"`. The dialog closes, and the Ledger gets one new row per account with the note. On the dashboard, the cards read `1300.00` and `650.00`, and `TOTAL BALANCE` reads `1 950.00 EUR`. The stored-rows query shows `130000` and `65000`.
- **Edit (`bu-edit`).** The pencil and trash buttons at a row's right end on the Ledger have no accessible name, so click them by coordinate. In a 1200-px-wide window they sit at about x 1057 and x 1093 on the row's center line. Click the pencil of `Verify Checking 1 300.00 EUR`. `Edit Balance Update` opens: `Account` is greyed out and read-only, `Amount` holds `1300.00`, and `Date` and `Note` hold their saved values. Triple-click `Amount` (`count: 3`) and type `1350`. Triple-click `Note` and type `verify-saldo edited`. Keep `<nn>-edit-form`, then click `AXButton "Save"`. The row reads `1 350.00 EUR` with the new note. In the DB, the event keeps its id, and `latest_data_id` points at a new `event_data` row, so that event now has two `event_data` versions.
- **Delete (`bu-delete`).** Click the trash of `Verify Savings 650.00 EUR`. `Confirm` opens with "Are you sure you want to delete this event?", plus `Cancel` and `Confirm` buttons. Keep `<nn>-delete-confirm`, then click `AXButton "Confirm"`. The row disappears, and the card falls back to that account's previous update (`500.00 EUR`). The event stays in the DB with `deleted_at` set, so query it by id without the `deleted_at` filter.

## Gotchas

- Last driven end to end on 2026-10-18, on the dev DB: `bu-initial` and `bu-card` in run `20261018-212805-claude-stoic-vaughan-132554`, then `bu-bulk`, `bu-edit` and `bu-delete` in run `20261018-213407-claude-keen-boyd-d370a7`. All five are verified.
- `0` in Initial Balance and the empty Amount field are placeholders, so typing doesn't need a clear first. Edit dialogs come prefilled. Overwrite a field with a triple-click and then type. `app_type` with `mode: "replace"` appends to the old text instead.
- The Account select and the Date picker in `Update Balance` are pop-ups. Open the dialog from the card so the account is preset. For another date, click the dialog's `Date` label, which opens the calendar.
- The bulk dialog's date picker has a time component (default 23:59). The trigger shows only the date. Its calendar opens from a click on the dialog's `Date` label and stays open after you pick a day, so close it with `escape`. The balance under each name follows the picked date (run `20261025-172732-checkable-numbers`).
- Two updates on the same date both show. The dashboard uses the one created last.
- Ledger rows and hero metrics use a narrow space as the thousands separator (`1 350.00 EUR`). Match on the digits.
- Cashflow rows on the Ledger have only the trash button, no pencil.
- In demo mode, the rows vanish on `Exit Demo`, and `evidence.sh db` can't see them.
