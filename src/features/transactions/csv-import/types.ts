import type { ImportProfileRow, ImportRule, ImportVerdict } from '../../../shared/types'

export type CsvRow = Record<string, string>

export type CashflowFieldKey = 'date' | 'amount' | 'partner' | 'currency' | 'fxRate' | 'note'

export type ColumnMapping = Record<CashflowFieldKey, string | null>

export type IbanMatchResult =
  | { type: 'ownAccount'; accountId: number; accountName: string }
  | { type: 'partner'; accountId: number; accountName: string }
  | { type: 'unmatched'; rawIban: string }
  | { type: 'none' }

export interface SplitLeg {
  legId: number
  amountMinor: number
  note: string | null
  rawIban: string | null
  ibanMatch: IbanMatchResult
  bucketId: number | null
  counterpartAccountId: number | null
}

/** A row as the file and the rules give it. `ibanMatch` holds its counterpart: an own account or a partner, else none. */
export interface ParsedRow {
  index: number
  date: string
  amountMinor: number
  note: string | null
  originalCurrencyId: number | null
  originalAmountMinor: number | null
  fxRateMantissa: number | null
  fxRateExponent: number | null
  rawIban: string | null
  ibanMatch: IbanMatchResult
}

/** A parsed row with what Saldo already holds for it and the user's choice. */
export interface ImportRow extends ParsedRow {
  verdict: ImportVerdict
  isSelected: boolean
  bucketId: number | null
  splitLegs: SplitLeg[] | null
}

export type WizardStep = 'upload' | 'mapping' | 'review' | 'rules' | 'save-profile'

export interface WizardState {
  step: WizardStep
  file: File | null
  csvHeaders: string[]
  csvRows: CsvRow[]
  selectedAccountId: number | null
  columnMapping: ColumnMapping
  importRows: ImportRow[]
  unparseableRowCount: number
  /** From the preview the rows' verdicts came from; apply sends it back. */
  reviewToken: string | null
  balanceUpdateDays: string[]
  previewing: boolean
  importing: boolean
  rules: ImportRule[]
  loadedProfile: ImportProfileRow | null
  originalProfileColumnMapping: ColumnMapping | null
  originalProfileRules: ImportRule[] | null
  importedCount: number | null
}

export type TransformedCsvRow = CsvRow & {
  __negateAmount: boolean
  __overrideDateString: string | null
}
