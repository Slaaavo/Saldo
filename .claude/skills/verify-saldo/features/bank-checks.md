# Checks against the bank app

An own account's dashboard card can be checked against the user's bank app. "Matches bank" records in one click that Saldo's balance matched the bank app at that moment. "Find the difference" asks for the bank app's balance, shows how far Saldo is off, and books the gap as one "Unexplained difference" entry. The card then shows "Verified <day>", or "Verified <day> · now differs by X" when a later change broke the check. Buckets and assets have no check, and time-machine mode hides it.

## Sub-features

- `BC-MATCH`: "Matches bank" records a check at Saldo's live balance; the card shows "Verified <day>".
- `BC-UNDO`: the toast's Undo deletes that check.
- `BC-FIND`: "Find the difference" shows Saldo's balance, the typed bank balance and "Saldo is X lower/higher than the bank".
- `BC-BOOK`: "Book as unexplained difference" adds one entry and a matching check; Net worth and Liquid move by the difference.
- `BC-DIALOG-MATCH`: with the same balance typed, the dialog says "Saldo matches the bank" and "Mark as matching" records a check.
- `BC-DIFFERS`: deleting a counted entry after a check turns the line amber: "Verified <day> · now differs by X".
- `BC-LEDGER`: the "Unexplained Differences" type chip, the entry's badge, delete only (no pencil).
- `BC-HIDDEN`: no line and no buttons on buckets, assets, or any card in time-machine mode.

## How to get to it (user POV)

- Dashboard → Accounts → an own account's card → bottom row, right side: the double-check icon ("Matches bank") and the scale icon ("Find the difference").
- Ledger → type chip "Unexplained Differences" → the entry's delete button.

## Driving it with computer use

Preconditions: Saldo Dev in live mode on the dev DB with at least two own accounts. The migration count in the dev DB must include `041_balance_checks.sql` as this branch defines it; a dev DB that ran an older copy of 041 needs `reset-dev-db.sh`.

- **Match.** `app_ax_find` role `AXButton` lists `"Matches bank"` and `"Find the difference"` per card. Click the center of "Matches bank". Result: the card shows "Verified <day>" in green, and `evidence.sh db` on `balance_check` shows one row at the account's balance in minor units.
- **Undo.** Click "Matches bank", take a screenshot, then click the toast's `"Undo"` button in the same `app_batch`. Result: `sqlite_sequence` for `balance_check` moved on and that id is gone.
- **Find and book.** Click "Find the difference", click the `"Balance in the bank app"` field, type a balance 12.50 above Saldo's. Result: "Saldo is 12.50 EUR lower than the bank". Click "Book as unexplained difference". Result: the card shows the new balance and "Verified <day>"; an `unexplained_difference` event and a second check exist.
- **Differs.** Ledger → chip "Unexplained Differences" → delete the entry → "Confirm deletion" → "Delete". Dashboard: the card's line is amber, "Verified <day> · now differs by 12.50 EUR".
- **Dialog match.** "Find the difference" with Saldo's own balance typed: "Saldo matches the bank", button "Mark as matching". Click it: the card is green again.
- **Hidden.** Click the "Date" label, pick a day: the cards show only "Update balance".
- **Slovak.** Settings → click blank space above Language → `tab`, `down`, `down`, `return`. The bottom row reads "ZMENIŤ ZOSTATOK" with both icons on one line. Switch back with `up` instead of the second `down`.

## Gotchas

- Last driven end to end on 2026-11-14 (run `20261114-161629-feat-check-against-bank-app`): all sub-features in English, on a fresh dev DB with two accounts made through Add Account. Slovak last driven in run `20261114-154628-feat-check-against-bank-app`.
- The toast auto-closes after about 4 seconds. Click Undo in the same `app_batch` as "Matches bank", or the click lands on nothing. In a background window the toast may not paint at all: its "Undo" button then sits in the AX summary at the window's bottom edge (about 48×7), and a click at its center still works.
- "Matches bank" sends the balance the card showed; when an import or `saldo mcp` wrote since, the toast says the balance changed and nothing is stored.
- The amber line wraps to two lines on a 250 px card.
