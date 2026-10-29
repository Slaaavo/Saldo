import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import TaxForecastFiguresPage from './index'
import { makeCell, makeFigures, makeLineResult, makeResult, makeTaxModel } from '../taxForecastFixtures'

vi.mock('@/shared/api', () => ({
  getTaxModel: vi.fn(),
  calculateTaxModel: vi.fn(),
  saveTaxForecastFigures: vi.fn(),
  getConsolidationCurrency: vi.fn(),
}))

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))

const navigateMock = vi.fn()
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigateMock,
  useParams: () => ({ modelId: '5' }),
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => (opts ? `${key} ${Object.values(opts).join(' ')}` : key),
    i18n: { language: 'en' },
  }),
}))

vi.mock('@/app/usePageHeader', () => ({ usePageTitle: vi.fn() }))

import { getTaxModel, calculateTaxModel, saveTaxForecastFigures, getConsolidationCurrency } from '@/shared/api'

const EUR = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }

const makeQueryClient = () => new QueryClient({ defaultOptions: { queries: { retry: 0, staleTime: Infinity } } })

const renderPage = (queryClient = makeQueryClient()) =>
  render(
    <QueryClientProvider client={queryClient}>
      <TaxForecastFiguresPage />
    </QueryClientProvider>,
  )

const revenueSoFar = () => screen.findByLabelText('taxModels.forecast.line.revenue, taxModels.forecast.column.soFar')

describe('TaxForecastFiguresPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(getTaxModel as Mock).mockResolvedValue(makeTaxModel({ figures: makeFigures({ actualsThroughMonth: 9 }) }))
    ;(getConsolidationCurrency as Mock).mockResolvedValue(EUR)
    ;(calculateTaxModel as Mock).mockResolvedValue(makeResult())
    ;(saveTaxForecastFigures as Mock).mockResolvedValue(undefined)
  })

  describe('loading', () => {
    it('shows one input per line and period, in the sheet order', async () => {
      renderPage()

      await revenueSoFar()

      const labels = screen.getAllByRole('spinbutton').map((input) => input.getAttribute('aria-label'))
      expect(labels.slice(0, 4)).toEqual([
        'taxModels.forecast.line.revenue, taxModels.forecast.column.soFar',
        'taxModels.forecast.line.revenue, taxModels.forecast.column.rest',
        'taxModels.forecast.line.payroll, taxModels.forecast.column.soFar',
        'taxModels.forecast.line.payroll, taxModels.forecast.column.rest',
      ])
      expect(labels.filter((label) => label?.startsWith('taxModels.forecast.line.'))).toHaveLength(10)
    })

    it('renders nothing from the cached model until a fresh one has arrived', async () => {
      const queryClient = makeQueryClient()
      queryClient.setQueryData(['taxModel', 5], makeTaxModel({ figures: makeFigures({ typedValues: { revenue: { soFarMinor: 10000, restMinor: null } } }) }))
      queryClient.setQueryData(['consolidation-currency'], EUR)
      ;(getTaxModel as Mock).mockResolvedValue(makeTaxModel({ figures: makeFigures({ typedValues: { revenue: { soFarMinor: 20000, restMinor: null } } }) }))

      const { container } = renderPage(queryClient)

      expect(container).toBeEmptyDOMElement()
      expect(await revenueSoFar()).toHaveValue(200)
      expect(getTaxModel).toHaveBeenCalledTimes(1)
    })

    it('keeps the form and what was typed while the model is fetched again', async () => {
      const queryClient = makeQueryClient()
      renderPage(queryClient)
      fireEvent.change(await revenueSoFar(), { target: { value: '321' } })

      await queryClient.invalidateQueries({ queryKey: ['taxModel', 5] })

      expect(await revenueSoFar()).toHaveValue(321)
    })

    it('shows the typed figures of the model', async () => {
      ;(getTaxModel as Mock).mockResolvedValue(
        makeTaxModel({
          figures: makeFigures({ actualsThroughMonth: 9, payrollMonthlyMinor: 400000, typedValues: { revenue: { soFarMinor: 9000000, restMinor: null } } }),
        }),
      )
      renderPage()

      expect(await revenueSoFar()).toHaveValue(90000)
      expect(screen.getByLabelText('taxModels.figures.payrollMonthly')).toHaveValue(4000)
    })

    it('shows what Saldo would use as the placeholder of an empty field', async () => {
      ;(calculateTaxModel as Mock).mockResolvedValue(makeResult({ lines: [makeLineResult('revenue', { rest: makeCell(50, { untypedMinor: 1050 }) })] }))
      renderPage()
      const revenueRest = await screen.findByLabelText('taxModels.forecast.line.revenue, taxModels.forecast.column.rest')

      await waitFor(() => expect(revenueRest).toHaveAttribute('placeholder', '10.50'))
      expect(await revenueSoFar()).toHaveAttribute('placeholder', '0.00')
      expect(screen.getByLabelText('taxModels.forecast.line.payroll, taxModels.forecast.column.soFar')).not.toHaveAttribute('placeholder')
    })
  })

  describe('months', () => {
    it('does not take so-far figures when no month is actual', async () => {
      ;(getTaxModel as Mock).mockResolvedValue(makeTaxModel({ figures: makeFigures({ actualsThroughMonth: 0 }) }))
      renderPage()

      expect(await revenueSoFar()).toBeDisabled()
      expect(screen.getByLabelText('taxModels.forecast.line.revenue, taxModels.forecast.column.rest')).toBeEnabled()
    })

    it('does not take rest-of-year figures when every month is actual', async () => {
      ;(getTaxModel as Mock).mockResolvedValue(makeTaxModel({ figures: makeFigures({ actualsThroughMonth: 12 }) }))
      renderPage()

      expect(await revenueSoFar()).toBeEnabled()
      expect(screen.getByLabelText('taxModels.forecast.line.revenue, taxModels.forecast.column.rest')).toBeDisabled()
    })
  })

  describe('earlier profit', () => {
    it('is only offered for a company', async () => {
      renderPage()
      await revenueSoFar()

      expect(screen.queryByText('taxModels.figures.earlierProfitTitle')).not.toBeInTheDocument()
    })

    it('lists the rows of a company and adds a new one', async () => {
      ;(getTaxModel as Mock).mockResolvedValue(
        makeTaxModel({ personType: 'legal', figures: makeFigures({ earlierProfits: [{ profitYear: 2025, amountMinor: 1500000, dividendTaxRateBps: 700 }] }) }),
      )
      renderPage()

      expect(await screen.findByDisplayValue('2025')).toBeInTheDocument()
      expect(screen.getByDisplayValue('15000.00')).toBeInTheDocument()
      expect(screen.getByDisplayValue('7')).toBeInTheDocument()

      fireEvent.click(screen.getByText('taxModels.figures.addYear'))

      expect(screen.getAllByText('taxModels.figures.remove')).toHaveLength(2)
    })

    it('names every input of a row by its row number and column', async () => {
      ;(getTaxModel as Mock).mockResolvedValue(
        makeTaxModel({
          personType: 'legal',
          figures: makeFigures({
            earlierProfits: [
              { profitYear: 2024, amountMinor: 1000000, dividendTaxRateBps: 700 },
              { profitYear: 2025, amountMinor: 1500000, dividendTaxRateBps: 500 },
            ],
          }),
        }),
      )
      renderPage()

      expect(await screen.findByLabelText('taxModels.figures.earlierProfitRow 1, taxModels.figures.year')).toHaveValue(2024)
      expect(screen.getByLabelText('taxModels.figures.earlierProfitRow 1, taxModels.figures.amount')).toHaveValue(10000)
      expect(screen.getByLabelText('taxModels.figures.earlierProfitRow 2, taxModels.figures.year')).toHaveValue(2025)
      expect(screen.getByLabelText('taxModels.figures.earlierProfitRow 2, taxModels.figures.dividendTax')).toHaveValue(5)
    })

    it('removes a row', async () => {
      ;(getTaxModel as Mock).mockResolvedValue(
        makeTaxModel({ personType: 'legal', figures: makeFigures({ earlierProfits: [{ profitYear: 2025, amountMinor: 1500000, dividendTaxRateBps: 700 }] }) }),
      )
      renderPage()

      fireEvent.click(await screen.findByText('taxModels.figures.remove'))

      expect(screen.queryByDisplayValue('2025')).not.toBeInTheDocument()
    })
  })

  describe('saving', () => {
    it('saves what was typed and returns to the results', async () => {
      renderPage()
      fireEvent.change(await revenueSoFar(), { target: { value: '90000' } })

      fireEvent.click(screen.getByText('taxModels.figures.save'))

      await waitFor(() =>
        expect(saveTaxForecastFigures).toHaveBeenCalledWith({
          modelId: 5,
          figures: { actualsThroughMonth: 9, payrollMonthlyMinor: 0, typedValues: { revenue: { soFarMinor: 9000000, restMinor: null } }, earlierProfits: [] },
        }),
      )
      await waitFor(() => expect(navigateMock).toHaveBeenCalledWith({ to: '/tax-models/$modelId/results', params: { modelId: '5' } }))
    })

    it('stops placeholders from showing a stale value once a so-far figure is typed', async () => {
      ;(calculateTaxModel as Mock).mockResolvedValue(
        makeResult({ lines: [makeLineResult('revenue', { soFar: makeCell(100, { untypedMinor: 4200 }), rest: makeCell(50, { untypedMinor: 1050 }) })] }),
      )
      renderPage()
      const revenueRest = await screen.findByLabelText('taxModels.forecast.line.revenue, taxModels.forecast.column.rest')
      await waitFor(() => expect(revenueRest).toHaveAttribute('placeholder', '10.50'))

      fireEvent.change(await revenueSoFar(), { target: { value: '321' } })

      expect(revenueRest).not.toHaveAttribute('placeholder')
    })

    it('goes back to the results on cancel', async () => {
      renderPage()
      await revenueSoFar()

      fireEvent.click(screen.getByText('taxModels.figures.cancel'))

      expect(navigateMock).toHaveBeenCalledWith({ to: '/tax-models/$modelId/results', params: { modelId: '5' } })
      expect(saveTaxForecastFigures).not.toHaveBeenCalled()
    })
  })
})
