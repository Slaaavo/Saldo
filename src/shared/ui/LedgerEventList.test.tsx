import { describe, it, expect } from 'vitest'
import { groupSplitEvents } from './splitGroupUtils'
import type { SplitGroupRow } from './splitGroupUtils'
import type { EventWithData } from '../types'

const makeEvent = (overrides?: Partial<EventWithData>): EventWithData => {
  return {
    id: 1,
    accountId: 1,
    accountName: 'Checking',
    accountType: 'account',
    eventType: 'cashflow',
    eventDate: '2025-03-15T00:00:00',
    amountMinor: 10000,
    note: null,
    createdAt: '2025-03-15T00:00:00',
    currencyCode: 'EUR',
    currencyMinorUnits: 2,
    counterpartAccountId: null,
    counterpartAccountName: null,
    bucketId: null,
    bucketName: null,
    originalCurrencyId: null,
    originalCurrencyCode: null,
    originalAmountMinor: null,
    originalCurrencyMinorUnits: null,
    fxRateMantissa: null,
    fxRateExponent: null,
    linkedEventId: null,
    splitGroupId: null,
    splitGroupNote: null,
    vatRateBps: null,
    vatDeductiblePctBps: null,
    expenseDeductiblePctBps: null,
    prepaidPeriodMonths: null,
    isLinkedToTaxable: false,
    linkedTaxableEventId: null,
    hasLinkedCashflows: false,
    linkedCashflowCount: 0,
    linkedAssetId: null,
    isSystemGenerated: false,
    ...overrides,
  }
}

describe('groupSplitEvents', () => {
  it('passes standalone events through unchanged', () => {
    const ev1 = makeEvent({ id: 1, splitGroupId: null })
    const ev2 = makeEvent({ id: 2, splitGroupId: null, amountMinor: 5000 })
    const result = groupSplitEvents([ev1, ev2])
    expect(result).toHaveLength(2)
    expect(result).toContain(ev1)
    expect(result).toContain(ev2)
  })

  it('collapses two legs with the same splitGroupId into one SplitGroupRow', () => {
    const leg1 = makeEvent({ id: 10, splitGroupId: 42, amountMinor: 3000 })
    const leg2 = makeEvent({ id: 11, splitGroupId: 42, amountMinor: 7000 })
    const result = groupSplitEvents([leg1, leg2])
    expect(result).toHaveLength(1)
    const group = result[0] as { type: string }
    expect(group.type).toBe('splitGroup')
  })

  it('computes groupTotal as integer sum of amountMinor', () => {
    const leg1 = makeEvent({ id: 10, splitGroupId: 42, amountMinor: 3000 })
    const leg2 = makeEvent({ id: 11, splitGroupId: 42, amountMinor: 7000 })
    const [group] = groupSplitEvents([leg1, leg2]) as SplitGroupRow[]
    expect(group.groupTotal).toBe(10000)
    expect(group.legCount).toBe(2)
    expect(group.legs).toHaveLength(2)
  })

  it('handles three legs correctly', () => {
    const leg1 = makeEvent({ id: 1, splitGroupId: 5, amountMinor: 1000 })
    const leg2 = makeEvent({ id: 2, splitGroupId: 5, amountMinor: 2000 })
    const leg3 = makeEvent({ id: 3, splitGroupId: 5, amountMinor: -500 })
    const [group] = groupSplitEvents([leg1, leg2, leg3]) as SplitGroupRow[]
    expect(group.groupTotal).toBe(2500)
    expect(group.legCount).toBe(3)
  })

  it('keeps standalone events separate from split groups', () => {
    const standalone = makeEvent({ id: 1, splitGroupId: null, amountMinor: 9999 })
    const leg1 = makeEvent({ id: 2, splitGroupId: 10, amountMinor: 4000 })
    const leg2 = makeEvent({ id: 3, splitGroupId: 10, amountMinor: 6000 })
    const result = groupSplitEvents([standalone, leg1, leg2])
    expect(result).toHaveLength(2)
    const types = result.map((item) => ('type' in item ? 'splitGroup' : 'standalone'))
    expect(types).toContain('standalone')
    expect(types).toContain('splitGroup')
  })

  it('keeps distinct split groups separate', () => {
    const legA1 = makeEvent({ id: 1, splitGroupId: 1, amountMinor: 100 })
    const legA2 = makeEvent({ id: 2, splitGroupId: 1, amountMinor: 200 })
    const legB1 = makeEvent({ id: 3, splitGroupId: 2, amountMinor: 500 })
    const legB2 = makeEvent({ id: 4, splitGroupId: 2, amountMinor: 300 })
    const result = groupSplitEvents([legA1, legA2, legB1, legB2])
    expect(result).toHaveLength(2)
    const groups = result as SplitGroupRow[]
    const totals = groups.map((g) => g.groupTotal).sort()
    expect(totals).toEqual([300, 800])
  })

  it('uses the groupDate from the first leg', () => {
    const leg1 = makeEvent({ id: 1, splitGroupId: 7, eventDate: '2025-03-15T00:00:00' })
    const leg2 = makeEvent({ id: 2, splitGroupId: 7, eventDate: '2025-03-15T00:00:00' })
    const [group] = groupSplitEvents([leg1, leg2]) as SplitGroupRow[]
    expect(group.groupDate).toBe('2025-03-15T00:00:00')
  })

  it('uses splitGroupNote from the first leg', () => {
    const leg1 = makeEvent({ id: 1, splitGroupId: 3, splitGroupNote: 'Groceries split' })
    const leg2 = makeEvent({ id: 2, splitGroupId: 3, splitGroupNote: 'Groceries split' })
    const [group] = groupSplitEvents([leg1, leg2]) as SplitGroupRow[]
    expect(group.groupNote).toBe('Groceries split')
  })

  it('returns empty array for empty input', () => {
    expect(groupSplitEvents([])).toHaveLength(0)
  })
})
