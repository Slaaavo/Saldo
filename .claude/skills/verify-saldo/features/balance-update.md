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

- **Create the account.** Click `AXButton "Add Account"`. A `Create Account` dialog opens. Click `AXTextField "Account Name"`, type `Verify Checking`. Click `AXTextField "Initial Balance (optional)"`, type `1000`. Click `AXButton "Create"`. The dialog closes, and the dashboard shows the card `Verify Checking` with `1 000.00 EUR` and `TOTAL BALANCE 1 000.00 EUR`.
- **Open Update Balance from the card.** Click the card's `AXButton "UPDATE BALANCE"`. The `Update Balance` dialog opens with Account preset to `Verify Checking` and Date set to today. Leave both pop-ups alone.
- **Enter the new balance.** Click `AXTextField "Amount"`, type `1250.50`. Click `AXTextField "Note"`, type `verify-saldo`. Save `app_screenshot` (`scale: 1`) as `<nn>-update-form` with `evidence.sh keep`.
- **Save.** Click `AXButton "Save"`. The dialog closes. The card and `TOTAL BALANCE` both read `1 250.50 EUR`. Keep that screenshot as `<nn>-dashboard-after`.
- **Second view.** Click `AXLink "Ledger"`. Under today's date header there are two `Verify Checking` rows: `1 250.50 EUR` with note `verify-saldo`, and `1 000.00 EUR`. Keep it as `<nn>-ledger-after`.
- **Stored rows.** Run `evidence.sh db <nn>-balance-rows "select a.name, e.event_type, d.amount_minor, d.event_date, d.note from account a join event e on e.account_id = a.id join event_data d on d.id = e.latest_data_id where a.name = 'Verify Checking' and e.deleted_at is null order by e.id"`. It shows two `balance_update` rows, `100000` and `125050`, with today's `event_date`.
- **Bulk entry (`bu-bulk`).** Click `AXButton "Add Events"`, then the `Bulk Balance Update` option. The `Update All Balances` dialog lists one `New balance` input per account. Click the input for `Verify Checking`, type a value, and click `AXButton "Update"`. The card shows that value.
- **Edit and delete (`bu-edit`, `bu-delete`).** On the Ledger page, the pencil and trash buttons at the right end of a row have no accessible name, so click them by screenshot coordinates. The pencil opens `Edit Balance Update`. The trash opens `Confirm` with "Are you sure you want to delete this event?".

## Gotchas

- Last driven end to end on 2026-10-18: `bu-initial` and `bu-card`, on the dev DB. `bu-bulk`, `bu-edit` and `bu-delete` have not been driven with this harness yet.
- `0` in Initial Balance and the empty Amount field are placeholders. Typing doesn't need a clear first.
- The Account select and the Date picker in `Update Balance` are pop-ups. Open the dialog from the card so the account is preset. For another date, click the dialog's `Date` label, which opens the calendar.
- The bulk dialog's date picker has a time component (default 23:59), and its calendar stays open after you pick a day. Close it with `escape`.
- Two updates on the same date both show. The dashboard uses the one created last.
- Amounts render with a narrow space as the thousands separator (`1 250.50 EUR`). Match on the digits.
- In demo mode, the rows vanish on `Exit Demo`, and `evidence.sh db` can't see them.
