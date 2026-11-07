import { describe, expect, it } from 'vitest'
import { legPrivateUse, legReclaimsVat, legShowsPrivateUse, makeEmptyLeg } from './splitLegDraft'

const partlyReclaimed = { ...makeEmptyLeg('2026-07-15'), vatRate: '23', vatReclaimablePct: '50' }

describe('split leg private use', () => {
  it('takes the person flag on a leg with none of its own', () => {
    expect(legReclaimsVat(partlyReclaimed, true)).toBe(true)
    expect(legShowsPrivateUse(partlyReclaimed, true)).toBe(true)
    expect(legShowsPrivateUse(partlyReclaimed, false)).toBe(false)
  })

  it('prefers the leg flag over the person flag', () => {
    expect(legShowsPrivateUse({ ...partlyReclaimed, reclaimedVat: false }, true)).toBe(false)
    expect(legShowsPrivateUse({ ...partlyReclaimed, reclaimedVat: true }, false)).toBe(true)
  })

  it('drops a ticked switch the leg no longer shows', () => {
    const ticked = { ...partlyReclaimed, privateUse: true }

    expect(legPrivateUse(ticked, true)).toBe(true)
    expect(legPrivateUse(ticked, false)).toBe(false)
    expect(legPrivateUse({ ...ticked, vatReclaimablePct: '100' }, true)).toBe(false)
  })
})
