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
}

/** What Saldo already holds for a row. `new` and `completesTransfer` are new rows; `inSaldo` and `transferLeg` are already in Saldo. */
export type ImportVerdict =
  | { kind: 'new' }
  | { kind: 'completesTransfer'; eventId: number; accountId: number; accountName: string; eventDate: string; note: string | null }
  | { kind: 'inSaldo'; eventId: number; eventDate: string; note: string | null }
  /** `eventDate` is the leg's day now; leaving the row unticked moves the leg to the row's day. */
  | { kind: 'transferLeg'; eventId: number; otherAccountName: string; eventDate: string }

export type VerdictKind = ImportVerdict['kind']

export interface RowPreview {
  index: number
  verdict: ImportVerdict
}

export interface PreviewCsvImportInput {
  accountId: number
  rows: IncomingRowInput[]
}

/** `rows` come in the order they were sent. `balanceUpdateDays` are the rows' days on which the account has a balance update. */
export interface CsvImportPreview {
  rows: RowPreview[]
  balanceUpdateDays: string[]
  reviewToken: string
}

export interface SplitLegChoice {
  amountMinor: number
  note: string | null
  counterpartAccountId: number | null
  bucketId: number | null
}

/** The review's choice for one row. `bucketId` is ignored when `split` is set. */
export interface RowChoice {
  index: number
  selected: boolean
  bucketId: number | null
  split: SplitLegChoice[] | null
}

/** `rows` are exactly the previewed rows, with one choice per row; a changed Saldo fails with `IMPORT_CHANGED`. */
export interface ApplyCsvImportInput {
  accountId: number
  rows: IncomingRowInput[]
  choices: RowChoice[]
  reviewToken: string
}

/** `imported` counts the entries created: one per plain row, one per split leg. */
export interface CsvImportResult {
  imported: number
}
