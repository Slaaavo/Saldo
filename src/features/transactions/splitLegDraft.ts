import { showsPrivateUse } from './taxableEventForm'

export interface SplitLegDraft {
  eventId?: number
  eventDate: string
  amount: string
  vatRate: string
  vatReclaimablePct: string
  expenseDeductiblePct: string
  note: string
  reclaimedVat: boolean | null
  privateUse: boolean
}

export const makeEmptyLeg = (date: string): SplitLegDraft => ({
  eventDate: date,
  amount: '',
  vatRate: '',
  vatReclaimablePct: '',
  expenseDeductiblePct: '',
  note: '',
  reclaimedVat: null,
  privateUse: false,
})

/** A leg with no flag of its own takes the person's, so it may reclaim VAT. */
export const legShowsPrivateUse = (leg: SplitLegDraft): boolean => showsPrivateUse(leg.vatRate, leg.vatReclaimablePct, leg.reclaimedVat !== false)

export const legPrivateUse = (leg: SplitLegDraft): boolean => leg.privateUse && legShowsPrivateUse(leg)
