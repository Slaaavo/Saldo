import type { IncomeBudgetPreview, MonthKey } from '@/shared/types'

export type Effect =
  | { kind: 'off' }
  | { kind: 'noRate' }
  | { kind: 'raises'; amountMinor: number; currentMonthPending: boolean }
  | { kind: 'lowers'; amountMinor: number }
  | { kind: 'unchanged' }

export const effectOf = (preview: IncomeBudgetPreview, { enabled, moment }: { enabled: boolean; moment: MonthKey }): Effect => {
  if (!enabled) return { kind: 'off' }
  if (preview.valuation.kind === 'noRate') return { kind: 'noRate' }
  const value = preview.valuation.valueMinor
  if (value < 0) {
    const current = preview.lines.find((line) => line.month === moment)
    return { kind: 'raises', amountMinor: -value, currentMonthPending: current?.budgeted === true && current.receivedMinor === 0 }
  }
  if (value > 0) return { kind: 'lowers', amountMinor: value }
  return { kind: 'unchanged' }
}
