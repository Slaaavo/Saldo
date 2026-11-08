import { describe, it, expect } from 'vitest'
import type { BudgetCell } from '@/shared/types'
import { monthKey } from './month'
import { draftFromCells, parseAmount, parseDraft, parseVatBps, setCell, spanOf, widen, type BudgetDraft } from './budgetDraft'

const NOV = monthKey(2026, 11)
const OCT = monthKey(2026, 10)

const draftOf = (entries: [string, { budget?: string; vat?: string }][]): BudgetDraft =>
  entries.reduce<BudgetDraft>((draft, [key, cell]) => {
    const month = monthKey(Number(key.slice(0, 4)), Number(key.slice(5, 7)))
    return setCell(setCell(draft, month, 'budget', cell.budget ?? ''), month, 'vat', cell.vat ?? '')
  }, new Map())

describe('parseAmount', () => {
  it.each([
    ['5640', 564000],
    ['5640,50', 564050],
    ['5640.5', 564050],
    ['5 640,50', 564050],
    ['0', 0],
    ['100000000000', 10_000_000_000_000],
  ])('reads %j as %i minor units', (text, minor) => {
    expect(parseAmount(text, 2)).toEqual({ kind: 'value', value: minor })
  })

  it.each(['1,234.50', '1.234,50', '1,234', '12abc', '-5', '+5', '5640.505', '5640.', ',50', '100000000000.01', '99999999999999999999'])('refuses %j', (text) => {
    expect(parseAmount(text, 2)).toEqual({ kind: 'invalid' })
  })

  it('refuses any decimals in a currency without minor units', () => {
    expect(parseAmount('5640', 0)).toEqual({ kind: 'value', value: 5640 })
    expect(parseAmount('5640.5', 0)).toEqual({ kind: 'invalid' })
  })

  it('reads blank text as empty, not as zero', () => {
    expect(parseAmount('  ', 2)).toEqual({ kind: 'empty' })
  })
})

describe('parseVatBps', () => {
  it.each([
    ['23', 2300],
    ['23%', 2300],
    ['23 %', 2300],
    ['21,5', 2150],
    ['0', 0],
    ['100', 10000],
  ])('reads %j as %i basis points', (text, bps) => {
    expect(parseVatBps(text)).toEqual({ kind: 'value', value: bps })
  })

  it.each(['101', '100.01', '23.555', '-1', '23%%', 'abc'])('refuses %j', (text) => {
    expect(parseVatBps(text)).toEqual({ kind: 'invalid' })
  })
})

describe('parseDraft', () => {
  it('sends an empty VAT in a budgeted month as null', () => {
    expect(parseDraft(draftOf([['2026-11', { budget: '6120' }]]), 2).cells).toEqual([{ month: NOV, budgetMinor: 612000, vatBps: null }])
  })

  it('keeps a month that has only its VAT typed', () => {
    expect(parseDraft(draftOf([['2026-11', { vat: '23' }]]), 2).cells).toEqual([{ month: NOV, budgetMinor: null, vatBps: 2300 }])
  })

  it('counts an invalid cell as empty and lists it', () => {
    const parsed = parseDraft(
      draftOf([
        ['2026-11', { budget: '6 120,00', vat: 'abc' }],
        ['2026-10', { budget: '-5' }],
      ]),
      2,
    )

    expect(parsed.cells).toEqual([{ month: NOV, budgetMinor: 612000, vatBps: null }])
    expect(parsed.invalid).toEqual([
      { month: OCT, row: 'budget' },
      { month: NOV, row: 'vat' },
    ])
  })

  it('leaves out a month whose cells were both cleared', () => {
    const draft = setCell(draftOf([['2026-11', { budget: '6120' }]]), NOV, 'budget', ' ')

    expect(draft.size).toBe(0)
    expect(parseDraft(draft, 2).cells).toEqual([])
  })

  it('reads back the cells it was opened with', () => {
    const cells: BudgetCell[] = [
      { month: monthKey(2026, 7), budgetMinor: 285000, vatBps: 2300 },
      { month: monthKey(2026, 8), budgetMinor: 564050, vatBps: 2150 },
      { month: monthKey(2026, 9), budgetMinor: 1, vatBps: null },
      { month: OCT, budgetMinor: 0, vatBps: 0 },
      { month: NOV, budgetMinor: null, vatBps: 1 },
    ]

    const draft = draftFromCells(cells, 2)

    expect([...draft.values()]).toEqual([
      { budget: '2850', vat: '23' },
      { budget: '5640.50', vat: '21.5' },
      { budget: '0.01', vat: '' },
      { budget: '0', vat: '0' },
      { budget: '', vat: '0.01' },
    ])
    expect(parseDraft(draft, 2)).toEqual({ cells, invalid: [] })
  })
})

describe('spanOf', () => {
  it('shows only the moment year when nothing is typed', () => {
    expect(spanOf(new Map(), 2026)).toEqual({ fromYear: 2026, toYear: 2026 })
  })

  it('shows the one year that holds values, even when it is not the moment year', () => {
    expect(spanOf(draftOf([['2025-07', { budget: '2850' }]]), 2026)).toEqual({ fromYear: 2025, toYear: 2025 })
  })

  it('shows every year from the first to the last value, an empty year between them too', () => {
    expect(
      spanOf(
        draftOf([
          ['2026-11', { budget: '6120' }],
          ['2024-03', { vat: '23' }],
        ]),
        2026,
      ),
    ).toEqual({ fromYear: 2024, toYear: 2026 })
  })
})

describe('widen', () => {
  it('adds a year on either side', () => {
    expect(widen({ fromYear: 2026, toYear: 2026 }, 'before')).toEqual({ fromYear: 2025, toYear: 2026 })
    expect(widen({ fromYear: 2026, toYear: 2026 }, 'after')).toEqual({ fromYear: 2026, toYear: 2027 })
  })

  it('stops at the years and the span the preview accepts', () => {
    expect(widen({ fromYear: 1990, toYear: 2026 }, 'before')).toBeNull()
    expect(widen({ fromYear: 2026, toYear: 2200 }, 'after')).toBeNull()
    expect(widen({ fromYear: 2000, toYear: 2039 }, 'after')).toBeNull()
    expect(widen({ fromYear: 2000, toYear: 2038 }, 'after')).toEqual({ fromYear: 2000, toYear: 2039 })
  })
})
