---
name: record-invoices
description: Record invoices, credit notes and equipment purchases in Saldo through the `saldo` MCP server, and undo a recorded batch. Use when the user hands over invoice PDFs, scans or photos to record, or asks to "record these invoices in Saldo", "add October's invoices", "book this credit note" or "undo that batch".
---

# Record invoices in Saldo

You read the documents. Saldo validates them, stores them as entries and assets, and feeds them to the tax forecast. The `saldo` MCP server is a mode of the installed Saldo app (`saldo mcp`), so it always matches the app's database. Never record documents by driving the Saldo window with computer use.

## Before you start

- Check that the `mcp__saldo__*` tools are loaded. If they are not, tell the user to register the server once and start a new Claude Code session:

	```bash
	claude mcp add --scope user saldo -- /Applications/saldo.app/Contents/MacOS/saldo mcp
	```

- Call `saldo_context`. Check that `databasePath` is the file the user expects. Pick the person the documents belong to: company documents go to the person with `personType` `legal`. Note its `vatPayer` flag and the `recentBatches`.
- A tool can refuse with a sentence for the user, for example `DATABASE_BEHIND` ("open Saldo once"), `DATABASE_FOLDER_MISSING`, or `DATABASE_CHANGED` when Saldo moved or switched its database during the call. Relay it and stop. Saldo never writes into a database it refused.

## Read each document

Take these from every document. Amounts are integer cents (minor units), VAT included.

- **Issuer and number.** The issuer is whoever issued the document: the supplier's IČO for a received invoice, the company's own IČO for an issued one. Use the IČO whenever it is printed, and the issuer's name only when no IČO exists, so the same supplier always gets the same key. The number is the invoice or credit note number.
- **Dates.** `issueDate` is the dátum vyhotovenia. `deliveryDate` is the dátum dodania (DUZP) when the document prints one. Saldo dates the entry by the delivery date, or by the issue date when there is none.
- **Lines.** One line per VAT rate, with the gross amount at that rate. Saldo makes one entry for one line and a split group for two or more.
- **The kind.**
  - A received invoice is `expense`. An issued invoice is `revenue`.
  - A credit note (dobropis) is the same kind with a negative `grossMinor`, and its own number and dates.
  - A service paid ahead for a period that ends in a later year, such as a yearly licence, is `prepaid_expense` with `prepaidUntil` set to the last day it covers. Saldo spreads it over the years. A period inside one year is a plain `expense`.
- **Equipment.** A purchase over 1,700 € with a useful life of more than one year is an asset, which goes in the `assets` list of an `expense` document. The 1,700 € is the price without VAT when the VAT is reclaimed, and the full price otherwise. Saldo computes that price and refuses an asset under the threshold.
  - Set `depreciationMonths` from the Slovak tax depreciation group, for example 48 months (group 1) for computers, phones, office machines and most cars. Show the months in the table, so the user can correct them.
  - Accessories under the threshold on the same invoice stay as expense lines of that document.
  - An asset takes no payment link. A document with only assets must have empty `paymentIds`.
- **Shares.** Every expense line needs `deductiblePctBps` and `vatReclaimablePctBps` (10000 is 100 %). Look up the supplier's last entry with `find_entries` and its `issuer`, and reuse its shares. For a new supplier, propose 10000 deductible, and a VAT reclaim of 10000 when the person is a VAT payer or 0 when it is not. Mark proposed shares in the table. Set `reclaimedVat` only when a document differs from the person's VAT-payer flag.
- **Receipts with an eKasa QR code** go through the eKasa import in the Saldo app, not through this skill. That import keeps the receipt's own reference.

## Find each payment

- Call `find_payments` with the person, a window from a few days before the issue date to a few weeks after the due date, `amountMinor` set to the document total (either sign matches), and `unlinkedOnly` true. `noteContains` ignores case and accents, so `cez` finds `ČEZ`.
- Put the matching payment ids in the document's `paymentIds`. Saldo links them to the entry.
- A foreign-currency document takes the euro amount of its payment, so its `grossMinor` values come from the payment, split by the document's VAT rates. Search with `originalCurrency` and the original amount. If the payment is not in Saldo yet, leave the document out of the batch and list it as waiting for its payment.

## Record the batch

1. Call `get_forecast` for the person's tax model of that year, and keep the result.
2. Call `record_batch` with all documents and `dryRun` true. Saldo runs the real write and rolls it back. The answer carries a `reviewToken`.
3. If it refuses (`BATCH_REFUSED`), fix every listed item and run the dry run again. `ALREADY_RECORDED` means the document is in Saldo already. Drop it and tell the user.
4. Show the user one table, built from the dry run's answer. Give one row per document: the file, issuer, number, the date Saldo used and its source, the kind and shape, the gross total, the VAT rates, the shares, the asset price and months, the linked payments and every warning. List the documents waiting for a payment below it.
5. Wait for the user's yes.
6. Call `record_batch` with the identical label and documents, `dryRun` false and the dry run's `reviewToken`. Keep the `batchId`. Saldo refuses with `REVIEW_TOKEN_MISMATCH` when anything differs from the reviewed dry run, `file` and `note` included. After a correction, run the dry run again and show the user the new table before you write.
7. Call `get_forecast` for each id in `affectedTaxModelIds`, and tell the user which forecast lines moved and by how much. The ids include later years that a prepaid expense or an asset's depreciation reaches.

A forecast line counts entries only for the months the model marks as actual, and only while the user has not typed that line's year-so-far value. If a line did not move, check those two causes first and name the one that applies.

## Undo a batch

- Call `undo_batch` with the `batchId`. It soft-deletes the batch's entries and deletes its assets with their depreciation. A second call changes nothing.
- `BATCH_CHANGED` lists what the user changed in Saldo after the batch: an edited entry, a leg added to a split, an event added to an asset, an edited asset (name, price, purchase date or months), and a payment link added or removed by hand. Show the list, and send `includeChanged` true only after the user agrees.
- A lost batch id is in `saldo_context`'s `recentBatches`, or on any entry `find_entries` returns.

## Warnings in a result

- `payment_amount_differs`: the linked payments do not add up to the document total. For a foreign-currency document this usually means the amounts were not taken from the payment.
- `payment_direction_differs`: a linked payment goes the wrong way, for example money coming in for an expense. Check that it is the right payment.
- `payment_already_linked`: the payment is linked to another entry too. Check that this is intended.
- `similar_document`: another entry of the same person and kind has the same total within two weeks, from any issuer or typed in by hand (`sourceRef` null). Check that it is not the same document under another key or number.
