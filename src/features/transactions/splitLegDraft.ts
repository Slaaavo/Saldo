export interface SplitLegDraft {
  eventId?: number
  eventDate: string
  amount: string
  vatRate: string
  vatDeductiblePct: string
  expenseDeductiblePct: string
  note: string
}

export const makeEmptyLeg = (date: string): SplitLegDraft => ({
  eventDate: date,
  amount: '',
  vatRate: '',
  vatDeductiblePct: '',
  expenseDeductiblePct: '',
  note: '',
})
