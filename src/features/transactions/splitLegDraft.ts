export interface SplitLegDraft {
  eventId?: number
  eventDate: string
  amount: string
  vatRate: string
  vatReclaimablePct: string
  expenseDeductiblePct: string
  note: string
  reclaimedVat: boolean | null
}

export const makeEmptyLeg = (date: string): SplitLegDraft => ({
  eventDate: date,
  amount: '',
  vatRate: '',
  vatReclaimablePct: '',
  expenseDeductiblePct: '',
  note: '',
  reclaimedVat: null,
})
