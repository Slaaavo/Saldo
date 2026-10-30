# eKasa receipt import

The user imports a Slovak eKasa receipt from a photo, a PDF or a screenshot of its QR code. An online receipt id is fetched from the government API; an offline code (`OKP:cash register:YYMMDDHHMISS:number:total`) is parsed locally. The review step shows the date, the total and the VAT fields, warns when the same receipt was already imported, and saves a taxable expense (or a split group for several items). Any QR text that is not an eKasa code stops at an error step and is never sent to the API.

## Sub-features

- `ekasa-offline` parses an offline QR code into the review step without network access.
- `ekasa-reject` stops on the error step for a malformed offline code (impossible date, total without two decimals) or a non-eKasa QR.
- `ekasa-duplicate` warns in the review step when a live entry already carries the receipt, and still allows the import.
- `ekasa-online` fetches an online receipt id from the government API.
- `ekasa-pdf` decodes the QR from a PDF through PDFium.
- `ekasa-rules` matches an online receipt's items to the rules step's patterns through Saldo's matcher (`match_ekasa_rules`): case and accents ignored, the first match wins, a matched item takes its row's percentages exactly as typed (a cleared one stays blank), an unmatched item takes the Default row, and a pattern that does not compile is skipped with a warning toast naming its row. A percentage never blocks the step. Saving a profile with an uncompilable pattern is refused with a message naming the pattern.

## How to get to it (user POV)

- Dashboard → Ledger section → `Import` menu → `eKasa Receipt`.
- Ledger page → `Import` menu → `eKasa Receipt`.

## Driving it with computer use

Preconditions:

- Baseline from the [README](./README.md), on the dev DB.
- Display-scope control approved by the user for the menu and the native file panel.
- QR images. No generator ships with macOS. With the user's approval, install `segno` into a throwaway venv in `$TMPDIR` (not the sandbox one) and run `segno.make(text, error="m").save(path, scale=10, border=4)`; a `.pdf` path gives a PDF. The spec example `C44B3977-0E415CC6-EE663AA1-776C973A-A143B660:99920045678900001:180213093414:23:237.23` is a valid offline code (13 Feb 2018, 237.23 EUR). Change the date to `180230093414` for an impossible date, or use any URL for a non-eKasa QR.

- **Open the wizard.** Click `Import`, then `eKasa Receipt`. The `eKasa Receipt Import — Upload` dialog shows a drop zone and `Browse`.
- **Pick the file.** `Browse`, then `cmd+shift+g`, the absolute path, `return`, `Open`. Click `Next`.
- **Offline code (`ekasa-offline`).** The rules step lists `Default` with deductible and VAT percentages. Click `Next`. The review shows the receipt's date and total. Click `Save`; a profile prompt may follow.
- **Duplicate (`ekasa-duplicate`).** Import the same file again. The review shows an amber box "This receipt was already imported. Saving it again will create a duplicate." above the form, and `Save` stays enabled. Query `select id, source_ref, deleted_at from event where source_ref like 'o%line:%'` as proof: both entries carry the same `offline:` reference.
- **Rejected codes (`ekasa-reject`).** A malformed offline code or a URL ends on `eKasa Receipt Import — Error` with "The QR code in this file is not an eKasa receipt code..." and `Try Again`.

## Gotchas

- Last driven on 2026-10-25, on the dev DB, in run `20261025-081534-worktree-open-review-fixes`: `ekasa-offline`, `ekasa-duplicate` and `ekasa-reject` (Feb 30 and a URL), with the UI in Slovak, and `ekasa-pdf` with PDFium staged only in the bundle's Resources folder. `ekasa-online` (needs a real receipt id and the network) was not driven.
- PDFium binds once per app process. When `src-tauri/libpdfium.dylib` exists, `runner.sh` puts it in `.verify-saldo/Saldo Dev.app/Contents/Resources/`, as the installed app has it, so the bundle lookup is the one exercised. Without the library the app still starts, and a PDF import's error names every directory it tried.
- The wizard and `saldo mcp` share one reader (`ekasa/reading.rs`) and one rule matcher (`ekasa/rules.rs`), and the wizard gets the reader's own reading. An online id Finančná správa does not know now ends on the error step with `RECEIPT_NOT_FOUND` instead of an empty review.
- `ekasa-rules` has not been driven in the window: it needs a real online receipt id, and the `Import` menu and the file panel need display-scope control. The matcher, the command and the wizard are covered by cargo and Vitest tests, and `saldo mcp` exercised the matcher on the dev DB (see [mcp-recording](./mcp-recording.md)). The wizard's switch to the reading's shape and to pattern-only matching (2026-10-30) has not been driven in the window either.
- Backend error messages are English even when the UI is Slovak; the wizard shows them as-is.
- A taxable expense from eKasa stores a date-only `event_date` (`2018-02-13`).
