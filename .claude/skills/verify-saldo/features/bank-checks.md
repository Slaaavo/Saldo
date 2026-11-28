# Checks against the bank app

An own account's dashboard card can be checked against the user's bank app. "Matches bank" records in one click that Saldo's balance matched the bank app at that moment. "Find the difference" asks for the bank app's balance, shows how far Saldo is off, lists the likely causes with a fix for each, and books the gap as one "Unexplained difference" entry. The card then shows "Verified <day>", or "Verified <day> · now differs by X" when a later change broke the check. Buckets and assets have no check, and time-machine mode hides it.

## Sub-features

- `BC-MATCH`: "Matches bank" records a check at Saldo's live balance; the card shows "Verified <day>".
- `BC-UNDO`: the toast's Undo deletes that check.
- `BC-FIND`: "Find the difference" shows Saldo's balance, the typed bank balance and "Saldo is X lower/higher than the bank".
- `BC-BOOK`: "Book as unexplained difference" adds one entry and a matching check; Net worth and Liquid move by the difference.
- `BC-LATER-UPDATE`: after a booking on a day that already has a balance update, a balance update typed later that day replaces the booked entry: the card shows exactly the typed balance.
- `BC-DIALOG-MATCH`: with the same balance typed, the dialog says "Saldo matches the bank" and "Mark as matching" records a check.
- `BC-DIFFERS`: deleting a counted entry after a check turns the line amber: "Verified <day> · now differs by X".
- `BC-LEDGER`: the "Unexplained Differences" type chip, the entry's badge, delete only (no pencil).
- `BC-HIDDEN`: no line and no buttons on buckets, assets, or any card in time-machine mode.
- `BC-CAUSE-NONE`: a gap no single row closes lists "No single row explains the difference".
- `BC-CAUSE-EQUALS`: a row equal to the gap: "This row is exactly the difference", "Delete this row".
- `BC-CAUSE-FLIP`: a cash flow whose flipped sign closes the gap: "This row may have the wrong sign", "Flip the sign".
- `BC-CAUSE-COPY`: a row with the gap's amount and a twin within 5 days: "Looks like a copy of the row from <day>", "Delete this copy".
- `BC-CAUSE-FIX`: a fix runs after a nested confirm, the dialog stays open with the typed balance, and the difference updates ("Saldo matches the bank", "Mark as matching").
- `BC-CAUSE-WINDOW`: only rows added or changed after the account's newest check are causes.
- `BC-CAUSE-STALE`: on an account imported from CSV whose newest row is more than 3 days old, "Newest row in Saldo is from <day>. Import the latest export." and "Import CSV", which opens CSV import with the account picked. Never on an account without an import profile.
- `BC-CAUSE-TYPING`: the list appears once typing pauses and never shows an earlier value's causes under a newer difference.
- `BC-CAUSE-BOOKED`: a booked unexplained difference that closes the gap is listed first, even when older than the newest check: "The unexplained difference dated <day> is explained now", "Delete the booked difference". `<day>` is the entry's date, the one the ledger shows (a later balance update's day when the booking was dated at it). The real row that arrived later shows as "This row is exactly the difference", never as a copy.

- `BC-GUARD-NOTE`: changing only the note of a row a ✓ Verified check counted saves without asking.
- `BC-GUARD-DELETE`: deleting a counted row shows "Change a verified account?" above the delete confirm: "<account> matched the bank app on <day>. After this change it differs by X." Cancel closes both, deletes nothing and shows no toast. "Change anyway" is disabled for its first 600 ms, so a double-click on Delete cannot answer it; after that it deletes, and the card turns amber.
- `BC-GUARD-EDIT`: changing a counted balance update's amount asks on Save; Cancel keeps Edit Balance Update open with the typed amount and Save enabled.
- `BC-GUARD-TWO`: deleting or editing a transfer between two ✓ Verified accounts lists both accounts.
- `BC-GUARD-NEW-BU`: a new balance update dated before the check's day asks; one dated on the check's day or later saves without asking.
- `BC-GUARD-AMBER`: on an account whose card already shows "now differs by", changes save without asking.
- `BC-GUARD-FIX`: a fix in Find the difference that breaks a ✓ Verified check asks first, for example "Delete the booked difference" on a booking the check counted.
- `BC-NOT-ARRIVED`: when the sending account's import added a transfer the receiving bank app does not show yet, Find the difference on the receiving account lists "On its way from <account>. The bank app may not show it yet." with "Not arrived yet" instead of a delete or a copy to delete. A transfer the newest check counted is never listed this way. The button records a check without that transfer, closes the dialog, toasts "Marked as matching the bank without the transfer on its way" with Undo, and the card shows "Verified <today>" above the bank balance. The receiving account's own import later moves the leg to its bank's day, and the card stays verified.
- `BC-HOLD`: a ✓ Verified check stays verified when a later CSV import posts a card hold at another amount or removes a cancelled hold, because the check counts each hold as it was at the check. Driven in [csv-import.md](./csv-import.md) "Card holds" step 2. Deleting or editing the posted row by hand still turns the card amber.

## How to get to it (user POV)

- Dashboard → Accounts → an own account's card → bottom row, right side: the double-check icon ("Matches bank") and the scale icon ("Find the difference").
- Ledger → type chip "Unexplained Differences" → the entry's delete button.

## Driving it with computer use

Preconditions: Saldo Dev in live mode on the dev DB with at least two own accounts. The migration count in the dev DB must include `041_balance_checks.sql` as this branch defines it; a dev DB that ran an older copy of 041 needs `reset-dev-db.sh`.

- **Match.** `app_ax_find` role `AXButton` lists `"Matches bank"` and `"Find the difference"` per card. Click the center of "Matches bank". Result: the card shows "Verified <day>" in green, and `evidence.sh db` on `balance_check` shows one row at the account's balance in minor units.
- **Undo.** Click "Matches bank", take a screenshot, then click the toast's `"Undo"` button in the same `app_batch`. Result: `sqlite_sequence` for `balance_check` moved on and that id is gone.
- **Find and book.** Click "Find the difference", click the `"Balance in the bank app"` field, type a balance 12.50 above Saldo's. Result: "Saldo is 12.50 EUR lower than the bank". Click "Book as unexplained difference". Result: the card shows the new balance and "Verified <day>"; an `unexplained_difference` event and a second check exist.
- **Differs.** Ledger → chip "Unexplained Differences" → delete the entry → "Confirm deletion" → "Delete". Dashboard: the card's line is amber, "Verified <day> · now differs by 12.50 EUR".
- **Later update.** "Update balance" with a lower amount (the dialog dates it today at 23:59), then "Find the difference" with the old balance and "Book as unexplained difference". Result: the card shows the bank balance, and `evidence.sh db` shows the entry dated at that balance update's 23:59:00. "Update balance" again with the bank balance. Result: the card shows exactly the typed amount, not the typed amount plus the booked gap.
- **Dialog match.** "Find the difference" with Saldo's own balance typed: "Saldo matches the bank", button "Mark as matching". Click it: the card is green again.
- **Hidden.** Click the "Date" label, pick a day: the cards show only "Update balance".
- **Slovak.** Settings → click blank space above Language → `tab`, `down`, `down`, `return`. The bottom row reads "ZMENIŤ ZOSTATOK" with both icons on one line. Switch back with `up` instead of the second `down`.
- **Causes.** Load `fixtures/check-causes.sql` while the app is down (`sqlite3 "$HOME/Library/Application Support/com.teamsumbal.saldo.dev/saldo.db" < .claude/skills/verify-saldo/fixtures/check-causes.sql`, sandbox off), then launch. Verify Causes shows 960.00 EUR and "Verified 10 Nov · now differs by 12.50 EUR"; Verify Stale shows 480.00 EUR. On Verify Causes, "Find the difference", then type, one at a time with a triple-click on the field before each, and take a second screenshot after each because the list waits for typing to pause:
  - `961.00`: "No single row explains the difference".
  - `1005.00`: "This row is exactly the difference", "13 Nov · verify-causes groceries · -45.00 EUR".
  - `900.00`: "This row may have the wrong sign", "12 Nov · verify-causes parking · 30.00 EUR". "Flip the sign" → "Confirm" with "from 30.00 EUR to -30.00 EUR" → "Confirm". Result: toast "Sign flipped", Balance in Saldo 900.00, "Saldo matches the bank". `evidence.sh db` shows a second `event_data` version at -3000.
  - `912.50`: "Looks like a copy of the row from 5 Nov", "7 Nov · verify-causes coffee again · -12.50 EUR". "Delete this copy" → "Confirm deletion" → "Delete". Result: toast "Row deleted", Saldo 912.50, "Mark as matching" → the card shows "Verified <today>".
  - After that check, `1005.00` again lists nothing: the groceries row is older than the newest check (`BC-CAUSE-WINDOW`).
  - On Verify Stale (scroll the cards right), any other balance lists "Newest row in Saldo is from 10 Nov. Import the latest export."; "Import CSV" opens Import CSV with "Verify Stale" and the profile "Verify CSV" picked. Cancel it.
- **Booked difference explained.** On a verified account, "Find the difference" with a balance 12.50 below Saldo's, then "Book as unexplained difference". Quit, insert the real row the import would bring (`INSERT INTO event (account_id, event_type) SELECT id, 'cashflow' FROM account WHERE name = '<account>'; INSERT INTO event_data (event_id, amount_minor, event_date, note) SELECT max(id), -1250, '<yesterday>T12:00:00', 'verify-causes card payment' FROM event;`, sandbox off), launch. The card says "now differs by 12.50 EUR". "Find the difference" with the bank balance typed again: first "The unexplained difference dated <today> is explained now" with "Delete the booked difference", then "This row is exactly the difference" for the card payment. "Delete the booked difference" → "Delete the unexplained difference of -12.50 EUR dated <today>?" → "Delete". Result: "Saldo matches the bank"; `evidence.sh db` shows the `unexplained_difference` deleted and the card payment live.

- **Verified rows.** Load `fixtures/verified-rows.sql` while the app is down (as for Causes), then launch. Its four accounts sort first: Verify Guard 987.50, Verify Guard Two 400.00 and Verify Guard Three 300.00 show "Verified 10 Nov", Verify Booked 675.00 shows "Verified 8 Nov". Drive the dialogs with full-screen control (see Gotchas).
  - Verify Guard → `⋮` → View in ledger → pencil on the 1 Nov balance update → change only the note → Save: saved, no warning (`BC-GUARD-NOTE`).
  - Trash on the 5 Nov coffee → Delete: "Change a verified account?" with "Verify Guard matched the bank app on 10 Nov. After this change it differs by 12.50 EUR." Cancel: the row stays, no toast (`BC-GUARD-DELETE`).
  - Pencil on the balance update → amount 990 → Save → the warning ("differs by 10.00 EUR") → Cancel: the dialog stays with 990. Save → Change anyway: the card shows "Verified 10 Nov · now differs by 10.00 EUR" (`BC-GUARD-EDIT`). Then delete the coffee: no warning (`BC-GUARD-AMBER`).
  - Ledger → Transfers → trash on the 6 Nov transfer → Delete: the warning lists Verify Guard Two and Verify Guard Three, each "differs by 100.00 EUR". Cancel (`BC-GUARD-TWO`).
  - Verify Guard Two → Update balance dated 9 Nov: the warning; dated today: saves without asking (`BC-GUARD-NEW-BU`).
  - Verify Booked → Find the difference → `687.50` → "Delete the booked difference" → Delete → the warning ("Verify Booked matched the bank app on 8 Nov … differs by 12.50 EUR") → Change anyway: "Saldo matches the bank", and the card behind turns amber until "Mark as matching" (`BC-GUARD-FIX`). Verify Booked sits past the window's right edge and the card row ignores scroll-wheel input, so click the card's title and press `tab` four times to focus its "Find the difference" (the row scrolls it into view), then `return`.
  - Slovak: Settings → Language → Slovenčina, then Kniha → Prevody → trash on the transfer → Vymazať: "Zmeniť overený účet?" with "Účet Verify Guard Two sa 10. 11. zhodoval s aplikáciou banky. Po tejto zmene sa bude líšiť o 100.00 EUR." and "Aj tak zmeniť". Zrušiť, then switch back to English.

- **Transfer on its way (`BC-NOT-ARRIVED`).** Load `fixtures/in-transit.sql` while the app is down (as for Causes), then launch. It adds Verify Sender (`SK2199990000001000000004`) and Verify Receiver (`SK9899990000001000000005`), 1000.00 each on 1 Nov, sorted first. Write two exports with today's dates outside the repo (the scratchpad): `in-transit-sender.csv` with `Date,Amount,IBAN,Note` and `<yesterday>,-150.00,SK9899990000001000000005,verify transfer on its way`, and `in-transit-receiver.csv` with `<today>,150.00,SK2199990000001000000004,verify transfer arrived`. Drive with full-screen control (see Gotchas).
  - Ledger section → `Import` → `CSV` → `Browse` → `cmd+shift+g` → the sender file → `Open`; account Verify Sender → `Next` → `Next` → `Next`: one new row, `Transfer` to Verify Receiver. `Import 1 row` → `Close`. The cards read 850.00 and 1150.00.
  - Verify Receiver → "Find the difference" → type `1000`: "Saldo is 150.00 EUR higher than the bank", then "On its way from Verify Sender. The bank app may not show it yet.", "<yesterday> · verify transfer on its way · 150.00 EUR" and "Not arrived yet". No "Delete this row" for it.
  - "Not arrived yet": the dialog closes, the toast reads "Marked as matching the bank without the transfer on its way" with Undo, and the card shows "Verified <today>" at 1150.00. `evidence.sh db` on `balance_check` joined to `balance_check_not_arrived` shows a check at 100000 that leaves out the receiving leg.
  - Import the receiver file into Verify Receiver: "New rows: 0. Already in Saldo: 1.", and the folded row says "Transfer with Verify Sender, moves from <yesterday> to <today>". `Finish` → `Close`. The leg is on today with a bank entry, and the card still shows "Verified <today>" at 1150.00.
  - Write `in-transit-sender-2.csv` with `<today>,-150.00,SK9899990000001000000005,verify second transfer` and import it into Verify Sender. Verify Receiver shows 1300.00. "Find the difference" → `1150`: only "On its way from Verify Sender" for the second transfer, and no "Looks like a copy" although the first 150.00 transfer is within 5 days. Cancel.
  - "Matches bank" on Verify Receiver, then "Find the difference" → `1150`: "No single row explains the difference". The newest check saw the second transfer, so it is no longer on its way.

## Gotchas

- `BC-NOT-ARRIVED` driven end to end on 2026-11-28 in run `20261128-100514-feat-money-in-transit`, with full-screen control and the window zoomed. Its two steps with the second transfer were driven in run `20261128-130212-feat-money-in-transit`. The card's icons move down when its verification line shows, so take a fresh screenshot before clicking "Find the difference". Its exports must be dated within 5 days of today, so they are written at run time, not kept as fixtures.
- On 2026-11-21 (macOS Darwin 27.2) background input dismissed Radix dialogs: a background click on a dialog's title, label or input, or a keystroke into its focused field, closed the dialog without saving, on master too (run `20261121-173353-HEAD`). Button clicks mostly worked, but a dialog that opens right after a click (the verified-row warning) was lost about half the time. Drive dialog steps with full-screen control. Run `20261121-172150-feat-verified-row-warning` drove `BC-GUARD-NOTE` and `BC-GUARD-DELETE` in the background. Run `20261122-081618-feat-verified-row-warning` drove `BC-GUARD-EDIT`, `-AMBER`, `-TWO`, `-NEW-BU`, `-FIX` and the Slovak warning with full-screen control: double-click the window's title bar to zoom it to the display, then every dialog, select and calendar opened and stayed open with plain clicks and typing.

- Last driven end to end on 2026-11-14 (run `20261114-161629-feat-check-against-bank-app`): all sub-features in English, on a fresh dev DB with two accounts made through Add Account. `BC-LATER-UPDATE` and the Slovak layout last driven in run `20261114-200106-feat-check-against-bank-app`.
- An entry booked before 2026-11-14 20:00 on a dev DB may be dated one second after a 23:59 balance update (the first rule) and stays on top of a later balance update. Run `reset-dev-db.sh` or use another account.
- The toast auto-closes after about 4 seconds. Click Undo in the same `app_batch` as "Matches bank", or the click lands on nothing. In a background window the toast may not paint at all: its "Undo" button then sits in the AX summary at the window's bottom edge (about 48×7), and a click at its center still works.
- "Matches bank" sends the balance the card showed; when an import or `saldo mcp` wrote since, the toast says the balance changed and nothing is stored.
- The amber line wraps to two lines on a 250 px card.
- Causes last driven on 2026-11-20 in runs `20261120-172204-feat-check-causes` (every `BC-CAUSE-*` path in English) and `20261120-172549-feat-check-causes` (the typing and amount-width fixes, `BC-CAUSE-WINDOW`, Slovak). The window found two defects that are now fixed: the list from the previous value showed under a new difference while typing paused, and a long note cut the amount off. The final code's fix loop and `BC-CAUSE-BOOKED` last driven on 2026-11-21 in runs `20261121-084157-feat-check-causes` and `20261121-085318`/`085352-feat-check-causes` (the booking, then the explained booked difference after a relaunch).
- A fixture loaded while the app runs stays invisible: the window never gains focus in the background, so the refresh on outside writes never fires. Load it with the app down, or quit and launch again.
- `check-causes.sql` adds nothing on a second run, so after its rows were fixed and a check marked, the causes need fresh rows (insert one dated within 5 days of 5 Nov with -1250) or `reset-dev-db.sh`.
