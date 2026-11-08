import { describe, it, expect } from 'vitest'
import { monthKey, monthsOfYears } from './month'
import { cellText, setCell, type BudgetDraft, type DraftRow } from './budgetDraft'
import { columnAtX, edgeScrollDelta, fillRange, scrollLeftForMoment, type TableGeometry } from './fillRange'

const MONTHS_2026 = monthsOfYears(2026, 2026)

const withText = (draft: BudgetDraft, monthIndex: number, row: DraftRow, text: string) => setCell(draft, MONTHS_2026[monthIndex], row, text)

const rowTexts = (draft: BudgetDraft, row: DraftRow) => MONTHS_2026.map((month) => cellText(draft, month, row))

describe('fillRange', () => {
  it('copies the origin into the months to its right', () => {
    const draft = withText(new Map(), 2, 'budget', '5640')

    const filled = fillRange(draft, MONTHS_2026, { row: 'budget', originIdx: 2, targetIdx: 5 })

    expect(rowTexts(filled, 'budget')).toEqual(['', '', '5640', '5640', '5640', '5640', '', '', '', '', '', ''])
  })

  it('copies the origin into the months to its left', () => {
    const draft = withText(withText(new Map(), 5, 'budget', '6120'), 3, 'budget', '1')

    const filled = fillRange(draft, MONTHS_2026, { row: 'budget', originIdx: 5, targetIdx: 2 })

    expect(rowTexts(filled, 'budget')).toEqual(['', '', '6120', '6120', '6120', '6120', '', '', '', '', '', ''])
  })

  it('clears the months when the origin is empty, and drops a month left with nothing', () => {
    let draft = withText(new Map(), 3, 'budget', '5640')
    draft = withText(withText(draft, 4, 'budget', '5640'), 4, 'vat', '23')

    const filled = fillRange(draft, MONTHS_2026, { row: 'budget', originIdx: 2, targetIdx: 4 })

    expect(rowTexts(filled, 'budget').every((text) => text === '')).toBe(true)
    expect([...filled.keys()]).toEqual([monthKey(2026, 5)])
    expect(cellText(filled, monthKey(2026, 5), 'vat')).toBe('23')
  })

  it('leaves the other row as it was', () => {
    const draft = withText(withText(withText(new Map(), 0, 'vat', '23'), 1, 'vat', '20'), 0, 'budget', '5640')

    const filled = fillRange(draft, MONTHS_2026, { row: 'budget', originIdx: 0, targetIdx: 1 })

    expect(rowTexts(filled, 'vat').slice(0, 3)).toEqual(['23', '20', ''])
    expect(rowTexts(filled, 'budget').slice(0, 3)).toEqual(['5640', '5640', ''])
  })
})

const STICKY = 148
const geometryOf = (columns: number, width = 100): TableGeometry => ({ edges: Array.from({ length: columns + 1 }, (_, i) => STICKY + i * width), stickyWidth: STICKY })

describe('columnAtX', () => {
  const geometry = geometryOf(3)
  const at = (viewportX: number, scrollLeft = 0) => columnAtX(geometry, { viewportX, scrollLeft, viewportWidth: 400 })

  it('finds the month column under the pointer', () => {
    expect(at(200)).toBe(0)
    expect(at(248)).toBe(1)
    expect(at(447)).toBe(2)
  })

  it('clamps a pointer past the last column to the last month', () => {
    expect(at(1000)).toBe(2)
  })

  it('reads a pointer over the sticky labels as the first month they cover', () => {
    expect(at(10)).toBe(0)
    expect(at(10, 150)).toBe(1)
  })
})

describe('edgeScrollDelta', () => {
  const delta = (viewportX: number) => edgeScrollDelta(geometryOf(24), { viewportX, scrollLeft: 0, viewportWidth: 600 })

  it('does not scroll away from the edges', () => {
    expect(delta(300)).toBe(0)
    expect(delta(STICKY + 48)).toBe(0)
  })

  it('scrolls right near the right edge, faster the deeper the pointer goes', () => {
    expect(delta(576)).toBe(12)
    expect(delta(600)).toBe(24)
    expect(delta(900)).toBe(24)
  })

  it('scrolls left near the sticky labels and over them', () => {
    expect(delta(STICKY + 24)).toBe(-12)
    expect(delta(10)).toBe(-24)
  })
})

describe('scrollLeftForMoment', () => {
  const months = monthsOfYears(2025, 2026)
  const geometry = geometryOf(24)

  it('puts four months before the moment next to the labels', () => {
    expect(scrollLeftForMoment(geometry, months, monthKey(2025, 11))).toBe(600)
  })

  it('stays at the start when the moment is in the first months', () => {
    expect(scrollLeftForMoment(geometry, months, monthKey(2025, 3))).toBe(0)
  })

  it('goes to the end for a moment after the shown years and to the start for one before them', () => {
    expect(scrollLeftForMoment(geometry, months, monthKey(2027, 2))).toBe(1900)
    expect(scrollLeftForMoment(geometry, months, monthKey(2024, 12))).toBe(0)
  })
})
