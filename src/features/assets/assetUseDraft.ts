import type { AssetUseTerms, BusinessUseChange, PurchaseVat } from '../../shared/types'
import { bpsToPct, fromMinorUnits, pctToBps, toMinorUnits } from '../../shared/utils/format'

export const FULL_BUSINESS_USE_BPS = 10000

export const DEPRECIATION_PRESETS = [
  ['presetInstant', '0'],
  ['preset2y', '24'],
  ['preset4y', '48'],
  ['preset5y', '60'],
  ['preset6y', '72'],
  ['preset20y', '240'],
] as const

export interface AssetUseDraft {
  privateUse: boolean
  grossPrice: string
  vatRate: string
  vatReclaimablePct: string
  businessUsePct: string
  changes: BusinessUseChange[]
}

export const EMPTY_ASSET_USE_DRAFT: AssetUseDraft = {
  privateUse: false,
  grossPrice: '',
  vatRate: '',
  vatReclaimablePct: '',
  businessUsePct: '',
  changes: [],
}

export const draftFromTerms = (terms: AssetUseTerms, minorUnits: number): AssetUseDraft => {
  const purchaseVat = terms.purchaseVat
  const businessUsePctBps = terms.businessUsePctBps ?? FULL_BUSINESS_USE_BPS
  return {
    privateUse: purchaseVat !== null || businessUsePctBps < FULL_BUSINESS_USE_BPS,
    grossPrice: purchaseVat ? fromMinorUnits(purchaseVat.grossMinor, minorUnits) : '',
    vatRate: purchaseVat ? bpsToPct(purchaseVat.vatRateBps) : '',
    vatReclaimablePct: purchaseVat ? bpsToPct(purchaseVat.vatReclaimablePctBps) : '',
    businessUsePct: bpsToPct(businessUsePctBps),
    changes: terms.businessUseChanges,
  }
}

export const purchaseVatFromDraft = (draft: AssetUseDraft, minorUnits: number, showVatFields: boolean): PurchaseVat | null => {
  if (!draft.privateUse || !showVatFields || !draft.grossPrice.trim()) return null
  const grossMinor = toMinorUnits(draft.grossPrice, minorUnits)
  const vatRateBps = pctToBps(draft.vatRate)
  const vatReclaimablePctBps = pctToBps(draft.vatReclaimablePct)
  if (!(grossMinor > 0) || vatRateBps === null || vatReclaimablePctBps === null) return null
  return { grossMinor, vatRateBps, vatReclaimablePctBps }
}

export const assetUseProblem = (draft: AssetUseDraft, minorUnits: number, showVatFields: boolean): 'businessUseMissing' | 'vatIncomplete' | null => {
  if (!draft.privateUse) return null
  if (pctToBps(draft.businessUsePct) === null) return 'businessUseMissing'
  const vatStarted = draft.grossPrice.trim() !== '' || draft.vatRate.trim() !== ''
  if (showVatFields && vatStarted && purchaseVatFromDraft(draft, minorUnits, showVatFields) === null) return 'vatIncomplete'
  return null
}

export const termsFromDraft = (draft: AssetUseDraft, minorUnits: number, showVatFields: boolean): AssetUseTerms => ({
  purchaseVat: purchaseVatFromDraft(draft, minorUnits, showVatFields),
  businessUsePctBps: draft.privateUse ? pctToBps(draft.businessUsePct) : null,
  businessUseChanges: draft.changes,
})
