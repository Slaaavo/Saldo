import { describe, expect, it } from 'vitest'
import { EMPTY_ASSET_USE_DRAFT, draftFromTerms, purchaseVatFromDraft, termsFromDraft } from './assetUseDraft'

const kona = {
  purchaseVat: { grossMinor: 1560000, vatRateBps: 2300, vatReclaimablePctBps: 5000 },
  businessUsePctBps: 8000,
  businessUseChanges: [],
}

describe('assetUseDraft', () => {
  it('reads a mixed-use asset with the switch on', () => {
    const draft = draftFromTerms(kona, 2)

    expect(draft.privateUse).toBe(true)
    expect(draft.grossPrice).toBe('15600.00')
    expect(draft.vatRate).toBe('23')
    expect(draft.vatReclaimablePct).toBe('50')
    expect(draft.businessUsePct).toBe('80')
  })

  it('reads a plain asset with the switch off and keeps its dated changes', () => {
    const changes = [{ fromMonth: '2026-07', businessUsePctBps: 8000 }]

    const draft = draftFromTerms({ purchaseVat: null, businessUsePctBps: null, businessUseChanges: changes }, 2)

    expect(draft.privateUse).toBe(false)
    expect(draft.changes).toEqual(changes)
  })

  it('gives back the same terms it read', () => {
    expect(termsFromDraft(draftFromTerms(kona, 2), 2, true)).toEqual(kona)
  })

  it('sends only the dated changes with the switch off', () => {
    const changes = [{ fromMonth: '2026-07', businessUsePctBps: 8000 }]

    expect(termsFromDraft({ ...EMPTY_ASSET_USE_DRAFT, changes }, 2, true)).toEqual({
      purchaseVat: null,
      businessUsePctBps: null,
      businessUseChanges: changes,
    })
  })

  it('leaves the VAT facts out for a person who sees no VAT fields', () => {
    const terms = termsFromDraft(draftFromTerms(kona, 2), 2, false)

    expect(terms.purchaseVat).toBeNull()
    expect(terms.businessUsePctBps).toBe(8000)
  })

  it('has no VAT facts until the price, rate and reclaimed share are all there', () => {
    const draft = { ...draftFromTerms(kona, 2) }

    expect(purchaseVatFromDraft({ ...draft, grossPrice: '' }, 2, true)).toBeNull()
    expect(purchaseVatFromDraft({ ...draft, grossPrice: '0' }, 2, true)).toBeNull()
    expect(purchaseVatFromDraft({ ...draft, vatRate: '' }, 2, true)).toBeNull()
    expect(purchaseVatFromDraft({ ...draft, vatReclaimablePct: '' }, 2, true)).toBeNull()
  })
})
