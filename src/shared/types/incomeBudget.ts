import type { Valuation } from '.'

/** A zero-padded `YYYY-MM`. Rust's `YearMonth` sends only this shape; in TypeScript `isMonthKey` is the guard that makes one. */
export type MonthKey = string & { readonly __brand: 'MonthKey' }

/** One typed month. Both amounts are sent on input, null when empty, and at least one of them is set. */
export interface BudgetCell {
  month: MonthKey
  budgetMinor: number | null
  vatBps: number | null
}

export interface CountedAccount {
  id: number
  name: string
}

/** `day` and `momentMonth` come from the dashboard's moment, so the UI never decides which month has started. */
export interface IncomeBudget {
  day: string
  momentMonth: MonthKey
  months: BudgetCell[]
  countedAccounts: CountedAccount[]
}

/** A month that has not started has no Received. A started month without a budget has Received at its VAT (0 % when empty) and no Difference. */
export interface MonthLine {
  month: MonthKey
  started: boolean
  budgeted: boolean
  receivedMinor: number | null
  differenceMinor: number | null
}

export interface IncomeBudgetPreview {
  valueMinor: number
  valuation: Valuation
  lines: MonthLine[]
}
