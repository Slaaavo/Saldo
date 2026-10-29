import { describe, it, expect } from 'vitest'
import { coversNoMonths, draftFromFigures, figuresFromDraft as parseDraft, placeholderFor, placeholdersCurrent, withCell, type FiguresDraft } from './figuresDraft'
import { makeCell, makeFigures, makeLineResult } from '../taxForecastFixtures'
import { MAX_AMOUNT_MINOR } from '@/shared/types'

const CALENDAR_YEAR = 2026

const figuresFromDraft = (draft: FiguresDraft, minorUnits: number, calendarYear = CALENDAR_YEAR) => parseDraft(draft, minorUnits, calendarYear)

const emptyDraft = (overrides?: Partial<FiguresDraft>): FiguresDraft => ({ actualsThroughMonth: 9, payrollMonthly: '', typed: {}, earlierProfits: [], ...overrides })

describe('draftFromFigures', () => {
  it('starts empty when nothing is typed', () => {
    expect(draftFromFigures(makeFigures(), 2)).toEqual({ actualsThroughMonth: 12, payrollMonthly: '', typed: {}, earlierProfits: [] })
  })

  it('shows typed cells as decimal text and leaves untyped cells out', () => {
    const figures = makeFigures({
      actualsThroughMonth: 9,
      payrollMonthlyMinor: 400000,
      typedValues: { revenue: { soFarMinor: 9000000, restMinor: null }, payroll: { soFarMinor: null, restMinor: 0 } },
    })

    expect(draftFromFigures(figures, 2)).toEqual({
      actualsThroughMonth: 9,
      payrollMonthly: '4000.00',
      typed: { revenue: { soFar: '90000.00' }, payroll: { rest: '0.00' } },
      earlierProfits: [],
    })
  })

  it('reads amounts in whole units for a currency without decimals', () => {
    const figures = makeFigures({ payrollMonthlyMinor: 4000, typedValues: { revenue: { soFarMinor: 90000, restMinor: null } } })

    const draft = draftFromFigures(figures, 0)

    expect(draft.payrollMonthly).toBe('4000')
    expect(draft.typed.revenue?.soFar).toBe('90000')
  })

  it('shows earlier profit with its dividend tax as a percentage', () => {
    const figures = makeFigures({ earlierProfits: [{ profitYear: 2025, amountMinor: 1500000, dividendTaxRateBps: 700 }] })

    expect(draftFromFigures(figures, 2).earlierProfits).toEqual([{ profitYear: '2025', amount: '15000.00', dividendTaxPct: '7' }])
  })
})

describe('figuresFromDraft', () => {
  it('sends no typed value when every field is empty', () => {
    const result = figuresFromDraft(emptyDraft(), 2)

    expect(result).toEqual({ ok: true, figures: { actualsThroughMonth: 9, payrollMonthlyMinor: 0, typedValues: {}, earlierProfits: [] } })
  })

  it('treats a field with only spaces as empty', () => {
    const result = figuresFromDraft(emptyDraft({ typed: { revenue: { soFar: '  ', rest: '' } } }), 2)

    expect(result).toMatchObject({ ok: true, figures: { typedValues: {} } })
  })

  it('converts typed amounts to minor units and leaves the other cell of a line untyped', () => {
    const draft = emptyDraft({ payrollMonthly: '4000', typed: { revenue: { soFar: '90000.50' }, deductible_costs: { rest: '5600' } } })

    const result = figuresFromDraft(draft, 2)

    expect(result).toEqual({
      ok: true,
      figures: {
        actualsThroughMonth: 9,
        payrollMonthlyMinor: 400000,
        typedValues: { revenue: { soFarMinor: 9000050, restMinor: null }, deductible_costs: { soFarMinor: null, restMinor: 560000 } },
        earlierProfits: [],
      },
    })
  })

  it('converts amounts in whole units for a currency without decimals', () => {
    const result = figuresFromDraft(emptyDraft({ typed: { revenue: { soFar: '1234' } } }), 0)

    expect(result).toMatchObject({ ok: true, figures: { typedValues: { revenue: { soFarMinor: 1234 } } } })
  })

  it('accepts a decimal comma', () => {
    const result = figuresFromDraft(emptyDraft({ typed: { revenue: { soFar: '12,50' } } }), 2)

    expect(result).toMatchObject({ ok: true, figures: { typedValues: { revenue: { soFarMinor: 1250 } } } })
  })

  it('keeps a typed zero, which differs from nothing typed', () => {
    const result = figuresFromDraft(emptyDraft({ typed: { payroll: { soFar: '0' } } }), 2)

    expect(result).toMatchObject({ ok: true, figures: { typedValues: { payroll: { soFarMinor: 0, restMinor: null } } } })
  })

  it('rejects a negative amount and names the cell', () => {
    const result = figuresFromDraft(emptyDraft({ typed: { depreciation: { rest: '-1' } } }), 2)

    expect(result).toEqual({ ok: false, error: { kind: 'amountNegative', field: { target: 'cell', line: 'depreciation', period: 'rest' } } })
  })

  it('rejects text that is not a number and names the cell', () => {
    const result = figuresFromDraft(emptyDraft({ typed: { revenue: { soFar: 'abc' } } }), 2)

    expect(result).toEqual({ ok: false, error: { kind: 'amountInvalid', field: { target: 'cell', line: 'revenue', period: 'soFar' } } })
  })

  it.each([
    ['a cell', { typed: { revenue: { soFar: '100000000000.01' } } }, { target: 'cell', line: 'revenue', period: 'soFar' }],
    ['the monthly payroll', { payrollMonthly: '100000000000.01' }, { target: 'payrollMonthly' }],
  ] as const)('rejects %s above the amount limit', (_name, overrides, field) => {
    const result = figuresFromDraft(emptyDraft(overrides), 2)

    expect(result).toEqual({ ok: false, error: { kind: 'amountInvalid', field } })
  })

  it('accepts an amount at the limit', () => {
    const result = figuresFromDraft(emptyDraft({ payrollMonthly: '100000000000.00' }), 2)

    expect(result).toMatchObject({ ok: true, figures: { payrollMonthlyMinor: MAX_AMOUNT_MINOR } })
  })

  it('rejects an earlier profit amount above the limit', () => {
    const draft = emptyDraft({ earlierProfits: [{ profitYear: '2025', amount: '100000000000.01', dividendTaxPct: '7' }] })

    expect(figuresFromDraft(draft, 2)).toEqual({ ok: false, error: { kind: 'amountInvalid', field: { target: 'earlierProfit', row: 1 } } })
  })

  it('rejects a negative or invalid monthly payroll', () => {
    expect(figuresFromDraft(emptyDraft({ payrollMonthly: '-5' }), 2)).toEqual({ ok: false, error: { kind: 'amountNegative', field: { target: 'payrollMonthly' } } })
    expect(figuresFromDraft(emptyDraft({ payrollMonthly: 'x' }), 2)).toEqual({ ok: false, error: { kind: 'amountInvalid', field: { target: 'payrollMonthly' } } })
  })

  it('ignores a so-far cell when no month is actual, even if it holds text', () => {
    const draft = emptyDraft({ actualsThroughMonth: 0, typed: { revenue: { soFar: '100', rest: '50' }, payroll: { soFar: 'abc' } } })

    const result = figuresFromDraft(draft, 2)

    expect(result).toEqual({
      ok: true,
      figures: { actualsThroughMonth: 0, payrollMonthlyMinor: 0, typedValues: { revenue: { soFarMinor: null, restMinor: 5000 } }, earlierProfits: [] },
    })
  })

  it('ignores a rest cell when every month is actual', () => {
    const draft = emptyDraft({ actualsThroughMonth: 12, typed: { revenue: { soFar: '100', rest: '50' } } })

    const result = figuresFromDraft(draft, 2)

    expect(result).toMatchObject({ ok: true, figures: { typedValues: { revenue: { soFarMinor: 10000, restMinor: null } } } })
  })

  describe('earlier profit', () => {
    it('converts the amount and the dividend tax rate', () => {
      const draft = emptyDraft({ earlierProfits: [{ profitYear: '2025', amount: '15000', dividendTaxPct: '7' }] })

      expect(figuresFromDraft(draft, 2)).toMatchObject({ ok: true, figures: { earlierProfits: [{ profitYear: 2025, amountMinor: 1500000, dividendTaxRateBps: 700 }] } })
    })

    it('drops a row nobody filled in', () => {
      const draft = emptyDraft({ earlierProfits: [{ profitYear: '', amount: '', dividendTaxPct: '' }] })

      expect(figuresFromDraft(draft, 2)).toMatchObject({ ok: true, figures: { earlierProfits: [] } })
    })

    it('numbers the row from 1 when a half-filled row is rejected', () => {
      const draft = emptyDraft({
        earlierProfits: [
          { profitYear: '2024', amount: '100', dividendTaxPct: '10' },
          { profitYear: '2025', amount: '', dividendTaxPct: '7' },
        ],
      })

      expect(figuresFromDraft(draft, 2)).toEqual({ ok: false, error: { kind: 'amountInvalid', field: { target: 'earlierProfit', row: 2 } } })
    })

    it.each(['', '25', 'abcd', '2025.5'])('rejects the year "%s"', (profitYear) => {
      const draft = emptyDraft({ earlierProfits: [{ profitYear, amount: '100', dividendTaxPct: '7' }] })

      expect(figuresFromDraft(draft, 2)).toEqual({ ok: false, error: { kind: 'yearInvalid', row: 1 } })
    })

    it.each(['2026', '2027', '9999'])('rejects the year %s, which is not before the calendar year of the model', (profitYear) => {
      const draft = emptyDraft({ earlierProfits: [{ profitYear, amount: '100', dividendTaxPct: '7' }] })

      expect(figuresFromDraft(draft, 2)).toEqual({ ok: false, error: { kind: 'yearNotBefore', row: 1, calendarYear: 2026 } })
    })

    it('accepts the year right before the calendar year', () => {
      const draft = emptyDraft({ earlierProfits: [{ profitYear: '2025', amount: '100', dividendTaxPct: '7' }] })

      expect(figuresFromDraft(draft, 2)).toMatchObject({ ok: true })
    })

    it('measures the year against the calendar year it is given', () => {
      const draft = emptyDraft({ earlierProfits: [{ profitYear: '2025', amount: '100', dividendTaxPct: '7' }] })

      expect(figuresFromDraft(draft, 2, 2025)).toEqual({ ok: false, error: { kind: 'yearNotBefore', row: 1, calendarYear: 2025 } })
    })

    it('rejects a year that appears twice and names the second row', () => {
      const draft = emptyDraft({
        earlierProfits: [
          { profitYear: '2024', amount: '100', dividendTaxPct: '7' },
          { profitYear: '', amount: '', dividendTaxPct: '' },
          { profitYear: ' 2024 ', amount: '200', dividendTaxPct: '7' },
        ],
      })

      expect(figuresFromDraft(draft, 2)).toEqual({ ok: false, error: { kind: 'yearDuplicate', row: 3 } })
    })

    it('rejects a negative amount', () => {
      const draft = emptyDraft({ earlierProfits: [{ profitYear: '2025', amount: '-1', dividendTaxPct: '7' }] })

      expect(figuresFromDraft(draft, 2)).toEqual({ ok: false, error: { kind: 'amountNegative', field: { target: 'earlierProfit', row: 1 } } })
    })

    it.each(['', '-1', '100.01', 'x'])('rejects the dividend tax "%s"', (dividendTaxPct) => {
      const draft = emptyDraft({ earlierProfits: [{ profitYear: '2025', amount: '100', dividendTaxPct }] })

      expect(figuresFromDraft(draft, 2)).toEqual({ ok: false, error: { kind: 'rateInvalid', row: 1 } })
    })

    it('accepts a dividend tax of 0 and of 100 percent', () => {
      const draft = emptyDraft({
        earlierProfits: [
          { profitYear: '2024', amount: '100', dividendTaxPct: '0' },
          { profitYear: '2025', amount: '100', dividendTaxPct: '100' },
        ],
      })

      expect(figuresFromDraft(draft, 2)).toMatchObject({ ok: true, figures: { earlierProfits: [{ dividendTaxRateBps: 0 }, { dividendTaxRateBps: 10000 }] } })
    })
  })

  it('gives back the figures it was seeded from', () => {
    const figures = makeFigures({
      actualsThroughMonth: 9,
      payrollMonthlyMinor: 400000,
      typedValues: {
        revenue: { soFarMinor: 9000000, restMinor: null },
        deductible_costs: { soFarMinor: 1800000, restMinor: 560000 },
        non_deductible_costs: { soFarMinor: 150000, restMinor: null },
      },
      earlierProfits: [{ profitYear: 2025, amountMinor: 1500000, dividendTaxRateBps: 700 }],
    })

    expect(figuresFromDraft(draftFromFigures(figures, 2), 2)).toEqual({ ok: true, figures })
  })
})

describe('withCell', () => {
  it('sets one cell without touching the others or the original draft', () => {
    const draft = emptyDraft({ typed: { revenue: { soFar: '1' } } })

    const next = withCell(draft, 'revenue', 'rest', '2')

    expect(next.typed).toEqual({ revenue: { soFar: '1', rest: '2' } })
    expect(draft.typed).toEqual({ revenue: { soFar: '1' } })
  })

  it('adds a line that had no cells yet', () => {
    expect(withCell(emptyDraft(), 'payroll', 'soFar', '5').typed).toEqual({ payroll: { soFar: '5' } })
  })
})

describe('coversNoMonths', () => {
  it('leaves so far without months only when no month is actual', () => {
    expect(coversNoMonths(0, 'soFar')).toBe(true)
    expect(coversNoMonths(1, 'soFar')).toBe(false)
    expect(coversNoMonths(12, 'soFar')).toBe(false)
  })

  it('leaves the rest of the year without months only when every month is actual', () => {
    expect(coversNoMonths(12, 'rest')).toBe(true)
    expect(coversNoMonths(11, 'rest')).toBe(false)
    expect(coversNoMonths(0, 'rest')).toBe(false)
  })
})

describe('placeholdersCurrent', () => {
  const saved = emptyDraft({ actualsThroughMonth: 9, payrollMonthly: '4000.00', typed: { revenue: { soFar: '100.00' } } })

  it('keeps both placeholders for a draft that equals the saved one', () => {
    expect(placeholdersCurrent({ ...saved }, saved, 'soFar')).toBe(true)
    expect(placeholdersCurrent({ ...saved }, saved, 'rest')).toBe(true)
  })

  it('hides both placeholders once the month differs', () => {
    const draft = { ...saved, actualsThroughMonth: 8 }

    expect(placeholdersCurrent(draft, saved, 'soFar')).toBe(false)
    expect(placeholdersCurrent(draft, saved, 'rest')).toBe(false)
  })

  it('hides the rest placeholders but keeps the so-far ones when a typed cell differs', () => {
    const draft = withCell(saved, 'payroll', 'soFar', '5')

    expect(placeholdersCurrent(draft, saved, 'soFar')).toBe(true)
    expect(placeholdersCurrent(draft, saved, 'rest')).toBe(false)
  })

  it('hides the rest placeholders when the payroll text differs', () => {
    expect(placeholdersCurrent({ ...saved, payrollMonthly: '4000' }, saved, 'rest')).toBe(false)
  })

  it('treats an empty cell and an absent one alike', () => {
    const draft = withCell(saved, 'revenue', 'rest', '')

    expect(placeholdersCurrent(draft, saved, 'rest')).toBe(true)
    expect(placeholdersCurrent(saved, draft, 'rest')).toBe(true)
  })
})

describe('placeholderFor', () => {
  const lines = [makeLineResult('revenue', { soFar: makeCell(100, { source: 'typed', untypedMinor: 4200 }), rest: makeCell(50, { untypedMinor: 1050 }) })]

  it('shows the value Saldo would use without a typed value', () => {
    expect(placeholderFor(lines, 'revenue', 'soFar', 2)).toBe('42.00')
    expect(placeholderFor(lines, 'revenue', 'rest', 2)).toBe('10.50')
  })

  it('is missing until the results arrive or for a line they do not list', () => {
    expect(placeholderFor(undefined, 'revenue', 'soFar', 2)).toBeUndefined()
    expect(placeholderFor(lines, 'payroll', 'soFar', 2)).toBeUndefined()
  })
})
