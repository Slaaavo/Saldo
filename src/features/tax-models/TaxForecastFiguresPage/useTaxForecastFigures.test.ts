import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { createElement } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { toast } from 'sonner'
import { useTaxForecastFigures } from './useTaxForecastFigures'
import { makeCell, makeFigures, makeLineResult, makeResult, makeTaxModel } from '../taxForecastFixtures'

vi.mock('@/shared/api', () => ({
  calculateTaxModel: vi.fn(),
  saveTaxForecastFigures: vi.fn(),
}))

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))

const navigateMock = vi.fn()
vi.mock('@tanstack/react-router', () => ({ useNavigate: () => navigateMock }))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => (opts ? `${key} ${Object.values(opts).join(' ')}` : key),
  }),
}))

import { calculateTaxModel, saveTaxForecastFigures } from '@/shared/api'

const makeQueryClient = () => new QueryClient({ defaultOptions: { queries: { retry: 0 } } })

const makeWrapper = (queryClient: QueryClient) => {
  const Wrapper = ({ children }: { children: Parameters<typeof createElement>[2] }) => createElement(QueryClientProvider, { client: queryClient }, children)
  return Wrapper
}

const renderFigures = (model = makeTaxModel(), minorUnits = 2, queryClient = makeQueryClient()) =>
  renderHook(({ current }) => useTaxForecastFigures(current, minorUnits), { wrapper: makeWrapper(queryClient), initialProps: { current: model } })

describe('useTaxForecastFigures', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(calculateTaxModel as Mock).mockResolvedValue(makeResult())
    ;(saveTaxForecastFigures as Mock).mockResolvedValue(undefined)
  })

  describe('draft', () => {
    it('starts from the figures of the model', () => {
      const model = makeTaxModel({
        figures: makeFigures({ actualsThroughMonth: 9, payrollMonthlyMinor: 400000, typedValues: { revenue: { soFarMinor: 9000000, restMinor: null } } }),
      })

      const { result } = renderFigures(model)

      expect(result.current.draft).toEqual({ actualsThroughMonth: 9, payrollMonthly: '4000.00', typed: { revenue: { soFar: '90000.00' } }, earlierProfits: [] })
    })

    it('keeps what the user typed when the model is fetched again', () => {
      const { result, rerender } = renderFigures()
      act(() => result.current.setCell('revenue', 'soFar', '123'))

      rerender({ current: makeTaxModel({ figures: makeFigures({ typedValues: { revenue: { soFarMinor: 1, restMinor: null } } }) }) })

      expect(result.current.draft.typed).toEqual({ revenue: { soFar: '123' } })
    })

    it('adds, edits and removes earlier profit rows', () => {
      const { result } = renderFigures()

      act(() => result.current.addEarlierProfit())
      act(() => result.current.addEarlierProfit())
      act(() => result.current.changeEarlierProfit(1, { profitYear: '2025', amount: '100' }))
      act(() => result.current.removeEarlierProfit(0))

      expect(result.current.draft.earlierProfits).toEqual([{ profitYear: '2025', amount: '100', dividendTaxPct: '' }])
    })

    it('shows the value Saldo would use as the placeholder once the results arrive', async () => {
      ;(calculateTaxModel as Mock).mockResolvedValue(makeResult({ lines: [makeLineResult('revenue', { soFar: makeCell(100, { untypedMinor: 4200 }) })] }))
      const { result } = renderFigures()
      expect(result.current.placeholder('revenue', 'soFar')).toBeUndefined()

      await waitFor(() => expect(result.current.placeholder('revenue', 'soFar')).toBe('42.00'))

      expect(result.current.draft.typed).toEqual({})
    })

    describe('placeholders after the draft leaves the saved figures', () => {
      const lines = [
        makeLineResult('revenue', { soFar: makeCell(100, { untypedMinor: 4200 }), rest: makeCell(50, { untypedMinor: 1050 }) }),
        makeLineResult('payroll', { soFar: makeCell(100, { untypedMinor: 700 }), rest: makeCell(50, { untypedMinor: 300 }) }),
      ]

      const renderWithPlaceholders = async () => {
        ;(calculateTaxModel as Mock).mockResolvedValue(makeResult({ lines }))
        const rendered = renderFigures(makeTaxModel({ figures: makeFigures({ actualsThroughMonth: 9 }) }))
        await waitFor(() => expect(rendered.result.current.placeholder('revenue', 'soFar')).toBe('42.00'))
        return rendered
      }

      it('shows every placeholder while the draft equals the saved figures', async () => {
        const { result } = await renderWithPlaceholders()

        expect(result.current.placeholder('revenue', 'rest')).toBe('10.50')
        expect(result.current.placeholder('payroll', 'soFar')).toBe('7.00')
      })

      it('hides every placeholder when the month changes', async () => {
        const { result } = await renderWithPlaceholders()

        act(() => result.current.setMonth(8))

        expect(result.current.placeholder('revenue', 'soFar')).toBeUndefined()
        expect(result.current.placeholder('revenue', 'rest')).toBeUndefined()
      })

      it('shows the placeholders again when the month goes back to the saved one', async () => {
        const { result } = await renderWithPlaceholders()

        act(() => result.current.setMonth(8))
        act(() => result.current.setMonth(9))

        expect(result.current.placeholder('revenue', 'soFar')).toBe('42.00')
        expect(result.current.placeholder('revenue', 'rest')).toBe('10.50')
      })

      it('hides the rest placeholders but keeps the so-far ones when a so-far value is typed', async () => {
        const { result } = await renderWithPlaceholders()

        act(() => result.current.setCell('revenue', 'soFar', '90'))

        expect(result.current.placeholder('payroll', 'soFar')).toBe('7.00')
        expect(result.current.placeholder('payroll', 'rest')).toBeUndefined()
        expect(result.current.placeholder('revenue', 'rest')).toBeUndefined()
      })

      it('hides the rest placeholders when the monthly payroll is typed', async () => {
        const { result } = await renderWithPlaceholders()

        act(() => result.current.setPayrollMonthly('4000'))

        expect(result.current.placeholder('revenue', 'soFar')).toBe('42.00')
        expect(result.current.placeholder('revenue', 'rest')).toBeUndefined()
      })
    })
  })

  describe('saving', () => {
    it('sends no typed value when every field is empty', async () => {
      const { result } = renderFigures()

      await act(() => result.current.handleSave())

      expect(saveTaxForecastFigures).toHaveBeenCalledWith({
        modelId: 5,
        figures: { actualsThroughMonth: 12, payrollMonthlyMinor: 0, typedValues: {}, earlierProfits: [] },
      })
    })

    it('converts amounts to minor units', async () => {
      const { result } = renderFigures()
      act(() => {
        result.current.setMonth(9)
        result.current.setPayrollMonthly('4000')
        result.current.setCell('revenue', 'soFar', '90000.50')
        result.current.setCell('deductible_costs', 'rest', '5600')
        result.current.addEarlierProfit()
        result.current.changeEarlierProfit(0, { profitYear: '2025', amount: '15000', dividendTaxPct: '7' })
      })

      await act(() => result.current.handleSave())

      expect(saveTaxForecastFigures).toHaveBeenCalledWith({
        modelId: 5,
        figures: {
          actualsThroughMonth: 9,
          payrollMonthlyMinor: 400000,
          typedValues: { revenue: { soFarMinor: 9000050, restMinor: null }, deductible_costs: { soFarMinor: null, restMinor: 560000 } },
          earlierProfits: [{ profitYear: 2025, amountMinor: 1500000, dividendTaxRateBps: 700 }],
        },
      })
    })

    it('converts amounts in whole units for a currency without decimals', async () => {
      const { result } = renderFigures(makeTaxModel(), 0)
      act(() => result.current.setCell('revenue', 'soFar', '1234'))

      await act(() => result.current.handleSave())

      expect((saveTaxForecastFigures as Mock).mock.calls[0][0].figures.typedValues.revenue.soFarMinor).toBe(1234)
    })

    it('tells the user about a negative amount instead of saving it', async () => {
      const { result } = renderFigures()
      act(() => result.current.setCell('revenue', 'soFar', '-5'))

      await act(() => result.current.handleSave())

      expect(toast.error).toHaveBeenCalledWith('taxModels.figures.validation.amountNegative taxModels.forecast.line.revenue, taxModels.forecast.column.soFar')
      expect(saveTaxForecastFigures).not.toHaveBeenCalled()
      expect(navigateMock).not.toHaveBeenCalled()
    })

    it('tells the user about an amount that is not a number instead of saving it', async () => {
      const { result } = renderFigures()
      act(() => result.current.setPayrollMonthly('abc'))

      await act(() => result.current.handleSave())

      expect(toast.error).toHaveBeenCalledWith('taxModels.figures.validation.amountInvalid taxModels.figures.payrollMonthly')
      expect(saveTaxForecastFigures).not.toHaveBeenCalled()
    })

    it('tells the user which earlier profit row has a bad year', async () => {
      const { result } = renderFigures()
      act(() => {
        result.current.addEarlierProfit()
        result.current.changeEarlierProfit(0, { profitYear: '25', amount: '100', dividendTaxPct: '7' })
      })

      await act(() => result.current.handleSave())

      expect(toast.error).toHaveBeenCalledWith('taxModels.figures.validation.yearInvalid 1')
      expect(saveTaxForecastFigures).not.toHaveBeenCalled()
    })

    it('tells the user when an earlier profit year is not before the calendar year of the model', async () => {
      const { result } = renderFigures(makeTaxModel({ calendarYear: 2026 }))
      act(() => {
        result.current.addEarlierProfit()
        result.current.changeEarlierProfit(0, { profitYear: '2026', amount: '100', dividendTaxPct: '7' })
      })

      await act(() => result.current.handleSave())

      expect(toast.error).toHaveBeenCalledWith('taxModels.figures.validation.yearNotBefore 1 2026')
      expect(saveTaxForecastFigures).not.toHaveBeenCalled()
    })

    it('tells the user which earlier profit row repeats a year', async () => {
      const { result } = renderFigures()
      act(() => {
        result.current.addEarlierProfit()
        result.current.addEarlierProfit()
        result.current.changeEarlierProfit(0, { profitYear: '2024', amount: '100', dividendTaxPct: '7' })
        result.current.changeEarlierProfit(1, { profitYear: '2024', amount: '200', dividendTaxPct: '7' })
      })

      await act(() => result.current.handleSave())

      expect(toast.error).toHaveBeenCalledWith('taxModels.figures.validation.yearDuplicate 2')
      expect(saveTaxForecastFigures).not.toHaveBeenCalled()
    })

    it('refreshes the model and its results, then returns to the results page', async () => {
      const queryClient = makeQueryClient()
      const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
      const { result } = renderFigures(makeTaxModel(), 2, queryClient)

      await act(() => result.current.handleSave())

      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['taxModel', 5] })
      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['taxModelResults', 5] })
      expect(navigateMock).toHaveBeenCalledWith({ to: '/tax-models/$modelId/results', params: { modelId: '5' } })
    })

    it('shows the error and stays on the page when the backend rejects the figures', async () => {
      ;(saveTaxForecastFigures as Mock).mockRejectedValue({ code: 'VALIDATION', message: 'Profit year must be before 2026' })
      const queryClient = makeQueryClient()
      const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
      const { result } = renderFigures(makeTaxModel(), 2, queryClient)

      await act(() => result.current.handleSave())

      expect(toast.error).toHaveBeenCalledWith('Profit year must be before 2026')
      expect(invalidate).not.toHaveBeenCalled()
      expect(navigateMock).not.toHaveBeenCalled()
      expect(result.current.saving).toBe(false)
    })
  })

  it('returns to the results page on cancel without saving', () => {
    const { result } = renderFigures()

    result.current.handleCancel()

    expect(navigateMock).toHaveBeenCalledWith({ to: '/tax-models/$modelId/results', params: { modelId: '5' } })
    expect(saveTaxForecastFigures).not.toHaveBeenCalled()
  })
})
