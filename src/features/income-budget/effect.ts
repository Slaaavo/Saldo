import type { IncomeBudgetPreview, MonthKey } from '@/shared/types'

/** What the previewed value does to Left to Spend. Amounts are in the consolidation currency and never negative. */
export type Effect =
  | { kind: 'off' }
  | { kind: 'noRate' }
  | { kind: 'aboveCash'; amountMinor: number; currentMonthPending: boolean }
  | { kind: 'belowCash'; amountMinor: number }
  | { kind: 'equalsCash' }

export const effectOf = (preview: IncomeBudgetPreview, { enabled, moment }: { enabled: boolean; moment: MonthKey }): Effect => {
  if (!enabled) return { kind: 'off' }
  if (preview.valuation.kind === 'noRate') return { kind: 'noRate' }
  const value = preview.valuation.valueMinor
  if (value < 0) {
    const current = preview.lines.find((line) => line.month === moment)
    return { kind: 'aboveCash', amountMinor: -value, currentMonthPending: current?.budgeted === true && current.receivedMinor === 0 }
  }
  if (value > 0) return { kind: 'belowCash', amountMinor: value }
  return { kind: 'equalsCash' }
}
