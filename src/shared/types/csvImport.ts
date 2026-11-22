/** One parsed CSV row as Rust reads it. `index` is the row's index in the parsed file. Amounts are signed, in the account's minor units. */
export interface IncomingRowInput {
  index: number
  eventDate: string
  amountMinor: number
  note: string | null
  /** An own account makes the row a transfer, a partner a cash flow with that partner. */
  counterpartAccountId: number | null
  /** The row names an IBAN that matches no account in Saldo, so `counterpartAccountId` is null. */
  namesUnknownAccount: boolean
  originalCurrencyId: number | null
  originalAmountMinor: number | null
  fxRateMantissa: number | null
  fxRateExponent: number | null
  /** The masked card number on the row (`433703******2103`), null when it names none. */
  card: string | null
  /** The profile's pending rule marks the row as a card hold the bank has not booked. */
  pending: boolean
}

/** What Saldo already holds for a row. `new`, `completesTransfer` and `holdPostedAtOtherAmount` are new rows; the others are already in Saldo. */
export type ImportVerdict =
  | { kind: 'new' }
  | { kind: 'completesTransfer'; eventId: number; accountId: number; accountName: string; eventDate: string; note: string | null }
  | { kind: 'inSaldo'; eventId: number; eventDate: string; note: string | null }
  /** `eventDate` is the leg's day now; leaving the row unticked moves the leg to the row's day. */
  | { kind: 'transferLeg'; eventId: number; otherAccountName: string; eventDate: string }
  /** The row is the booked form of the pending card hold `eventId`, at the same amount. Leaving it unticked posts the hold in place. */
  | { kind: 'holdPosted'; eventId: number; eventDate: string; note: string | null }
  /** The booked form of the card hold `eventId` at another amount; `holdAmountMinor` is the hold's ledger amount. Ticked posts the hold at the row's amount. */
  | { kind: 'holdPostedAtOtherAmount'; eventId: number; eventDate: string; note: string | null; holdAmountMinor: number }

export type VerdictKind = ImportVerdict['kind']

export interface RowPreview {
  index: number
  verdict: ImportVerdict
}

export interface PreviewCsvImportInput {
  accountId: number
  rows: IncomingRowInput[]
}

/** A pending card hold the bank no longer lists. For a split group the amount is the group total and the note the group note. */
export interface CancelledHold {
  eventId: number
  eventDate: string
  amountMinor: number
  note: string | null
}

/** `rows` come in the order they were sent. `balanceUpdateDays` are the rows' days on which the account has a balance update. */
export interface CsvImportPreview {
  rows: RowPreview[]
  balanceUpdateDays: string[]
  cancelledHolds: CancelledHold[]
  reviewToken: string
}

export interface SplitLegChoice {
  amountMinor: number
  note: string | null
  counterpartAccountId: number | null
  bucketId: number | null
}

/** The review's choice for one row. `bucketId` is ignored when `split` is set or the verdict is `holdPostedAtOtherAmount`, which takes no `split`. */
export interface RowChoice {
  index: number
  selected: boolean
  bucketId: number | null
  split: SplitLegChoice[] | null
}

/** Whether the import removes one previewed cancelled hold. */
export interface CancelledHoldChoice {
  eventId: number
  remove: boolean
}

/** `rows` are exactly the previewed rows, with one choice per row and one `cancelledHolds` choice per previewed cancelled hold; a changed Saldo fails with `IMPORT_CHANGED`. */
export interface ApplyCsvImportInput {
  accountId: number
  rows: IncomingRowInput[]
  choices: RowChoice[]
  cancelledHolds: CancelledHoldChoice[]
  reviewToken: string
}

/** `imported` counts the entries created: one per plain row, one per split leg. `removedHolds` counts the cancelled holds removed. */
export interface CsvImportResult {
  imported: number
  removedHolds: number
}
