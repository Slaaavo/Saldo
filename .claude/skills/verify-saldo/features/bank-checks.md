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
- `BC-CAUSE-BOOKED`: a booked unexplained difference that closes the gap is listed first, even when older than the newest check: "The difference booked on <day> is explained now", "Delete the booked difference". The real row that arrived later shows as "This row is exactly the difference", never as a copy.

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
- **Booked difference explained.** On a verified account, "Find the difference" with a balance 12.50 below Saldo's, then "Book as unexplained difference". Quit, insert the real row the import would bring (`INSERT INTO event (account_id, event_type) SELECT id, 'cashflow' FROM account WHERE name = '<account>'; INSERT INTO event_data (event_id, amount_minor, event_date, note) SELECT max(id), -1250, '<yesterday>T12:00:00', 'verify-causes card payment' FROM event;`, sandbox off), launch. The card says "now differs by 12.50 EUR". "Find the difference" with the bank balance typed again: first "The difference booked on <today> is explained now" with "Delete the booked difference", then "This row is exactly the difference" for the card payment. "Delete the booked difference" → "Delete the unexplained difference of -12.50 EUR booked on <today>?" → "Delete". Result: "Saldo matches the bank"; `evidence.sh db` shows the `unexplained_difference` deleted and the card payment live.

## Gotchas

- Last driven end to end on 2026-11-14 (run `20261114-161629-feat-check-against-bank-app`): all sub-features in English, on a fresh dev DB with two accounts made through Add Account. `BC-LATER-UPDATE` and the Slovak layout last driven in run `20261114-200106-feat-check-against-bank-app`.
- An entry booked before 2026-11-14 20:00 on a dev DB may be dated one second after a 23:59 balance update (the first rule) and stays on top of a later balance update. Run `reset-dev-db.sh` or use another account.
- The toast auto-closes after about 4 seconds. Click Undo in the same `app_batch` as "Matches bank", or the click lands on nothing. In a background window the toast may not paint at all: its "Undo" button then sits in the AX summary at the window's bottom edge (about 48×7), and a click at its center still works.
- "Matches bank" sends the balance the card showed; when an import or `saldo mcp` wrote since, the toast says the balance changed and nothing is stored.
- The amber line wraps to two lines on a 250 px card.
- Causes last driven on 2026-11-20 in runs `20261120-172204-feat-check-causes` (every `BC-CAUSE-*` path in English) and `20261120-172549-feat-check-causes` (the typing and amount-width fixes, `BC-CAUSE-WINDOW`, Slovak). The window found two defects that are now fixed: the list from the previous value showed under a new difference while typing paused, and a long note cut the amount off. The final code's fix loop and `BC-CAUSE-BOOKED` last driven on 2026-11-21 in runs `20261121-084157-feat-check-causes` and `20261121-085318`/`085352-feat-check-causes` (the booking, then the explained booked difference after a relaunch).
- A fixture loaded while the app runs stays invisible: the window never gains focus in the background, so the refresh on outside writes never fires. Load it with the app down, or quit and launch again.
- `check-causes.sql` adds nothing on a second run, so after its rows were fixed and a check marked, the causes need fresh rows (insert one dated within 5 days of 5 Nov with -1250) or `reset-dev-db.sh`.
