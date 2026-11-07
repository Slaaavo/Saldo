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

export const legReclaimsVat = (leg: SplitLegDraft, personVatPayer: boolean): boolean => leg.reclaimedVat ?? personVatPayer

export const legShowsPrivateUse = (leg: SplitLegDraft, personVatPayer: boolean): boolean => showsPrivateUse(leg.vatRate, leg.vatReclaimablePct, legReclaimsVat(leg, personVatPayer))

export const legPrivateUse = (leg: SplitLegDraft, personVatPayer: boolean): boolean => leg.privateUse && legShowsPrivateUse(leg, personVatPayer)
