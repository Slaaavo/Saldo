import { describe, it, expect } from 'vitest'
import type { IncomeBudgetPreview, MonthLine } from '@/shared/types'
import { NO_RATE, converted, sameCurrency } from '@/shared/testing/snapshotFixtures'
import { monthKey } from './month'
import { effectOf } from './effect'

const NOV = monthKey(2026, 11)

const line = (overrides?: Partial<MonthLine>): MonthLine => ({ month: NOV, started: true, budgeted: true, receivedMinor: 0, differenceMinor: -612000, ...overrides })

const preview = (overrides?: Partial<IncomeBudgetPreview>): IncomeBudgetPreview => ({ valueMinor: -631500, valuation: sameCurrency(-631500), lines: [line()], ...overrides })

const on = { enabled: true, moment: NOV }

describe('effectOf', () => {
  it('says the bucket is ignored while it is switched off, whatever its value', () => {
    expect(effectOf(preview(), { enabled: false, moment: NOV })).toEqual({ kind: 'off' })
  })

  it('says Left to Spend leaves it out when its currency has no rate', () => {
    expect(effectOf(preview({ valuation: NO_RATE }), on)).toEqual({ kind: 'noRate' })
  })

  it('says Left to Spend shows more than the cash, and that this month is still waiting for its payment', () => {
    expect(effectOf(preview(), on)).toEqual({ kind: 'aboveCash', amountMinor: 631500, currentMonthPending: true })
  })

  it('does not call the month pending once its payment arrived or when it has no budget', () => {
    expect(effectOf(preview({ lines: [line({ receivedMinor: 497561, differenceMinor: -114439 })] }), on)).toEqual({
      kind: 'aboveCash',
      amountMinor: 631500,
      currentMonthPending: false,
    })
    expect(effectOf(preview({ lines: [line({ budgeted: false, differenceMinor: null })] }), on)).toEqual({ kind: 'aboveCash', amountMinor: 631500, currentMonthPending: false })
  })

  it('says how much cash stays out of Left to Spend when more arrived than budgeted', () => {
    expect(effectOf(preview({ valueMinor: 25000, valuation: sameCurrency(25000) }), on)).toEqual({ kind: 'belowCash', amountMinor: 25000 })
  })

  it('says Left to Spend equals the cash at zero', () => {
    expect(effectOf(preview({ valueMinor: 0, valuation: sameCurrency(0) }), on)).toEqual({ kind: 'equalsCash' })
  })

  it('uses the value in the consolidation currency', () => {
    expect(effectOf(preview({ valueMinor: -1000, valuation: converted(-920) }), on)).toEqual({ kind: 'aboveCash', amountMinor: 920, currentMonthPending: true })
  })
})
