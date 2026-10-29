import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen, fireEvent, within } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import TaxModelResultsPage from './TaxModelResultsPage'
import { FORECAST_LINES, type TaxEventBreakdown, type TaxCalculationResult } from '@/shared/types'
import { makeCell, makeLineResult, makeResult, makeTaxModel } from '../taxForecastFixtures'

vi.mock('@/shared/api', () => ({
  getTaxModel: vi.fn(),
  calculateTaxModel: vi.fn(),
  getConsolidationCurrency: vi.fn(),
}))

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

vi.mock('@/shared/ui/NumberValue', () => ({
  default: ({ value, minorUnits, currencyCode }: { value: number; minorUnits?: number; currencyCode?: string }) => (
    <span data-testid="number-value" data-minor-units={minorUnits} data-currency-code={currencyCode}>
      {value}
    </span>
  ),
}))

import { getTaxModel, calculateTaxModel, getConsolidationCurrency } from '@/shared/api'

const EUR = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }
const JPY = { id: 2, code: 'JPY', name: 'Japanese Yen', minorUnits: 0, isCustom: false }

const makeBreakdown = (overrides?: Partial<TaxEventBreakdown>): TaxEventBreakdown => ({
  eventId: 1,
  kind: 'revenue',
  eventDate: '2026-03-01T00:00:00',
  note: null,
  amountMinor: 100000,
  netAmountMinor: 100000,
  vatAmountMinor: 0,
  reclaimableVatMinor: 0,
  nonReclaimableVatMinor: 0,
  taxDeductibleCostMinor: 0,
  nonTaxDeductibleCostMinor: 0,
  notCounted: null,
  ...overrides,
})

const renderPage = (queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0, staleTime: Infinity } } })) =>
  render(
    <QueryClientProvider client={queryClient}>
      <TaxModelResultsPage />
    </QueryClientProvider>,
  )

describe('TaxModelResultsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(getTaxModel as Mock).mockResolvedValue(makeTaxModel())
    ;(getConsolidationCurrency as Mock).mockResolvedValue(EUR)
    ;(calculateTaxModel as Mock).mockResolvedValue(makeResult())
  })

  it('shows amounts in the minor units and code of the consolidation currency', async () => {
    ;(getConsolidationCurrency as Mock).mockResolvedValue(JPY)
    ;(calculateTaxModel as Mock).mockResolvedValue(makeResult({ taxAmountMinor: 11111 }))
    renderPage()

    const taxAmount = await screen.findByText('11111')
    expect(taxAmount).toHaveAttribute('data-minor-units', '0')
    expect(taxAmount).toHaveAttribute('data-currency-code', 'JPY')
    const values = screen.getAllByTestId('number-value')
    expect(values.length).toBeGreaterThan(20)
    for (const value of values) {
      expect(value).toHaveAttribute('data-minor-units', '0')
    }
  })

  it('recalculates the results every time the page is mounted', async () => {
    ;(calculateTaxModel as Mock).mockResolvedValueOnce(makeResult({ taxAmountMinor: 11111 })).mockResolvedValueOnce(makeResult({ taxAmountMinor: 22222 }))
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0, staleTime: Infinity } } })

    const first = renderPage(queryClient)
    expect(await screen.findByText('11111')).toBeInTheDocument()
    first.unmount()

    renderPage(queryClient)

    expect(await screen.findByText('22222')).toBeInTheDocument()
    expect(screen.queryByText('11111')).not.toBeInTheDocument()
    expect(calculateTaxModel).toHaveBeenCalledTimes(2)
  })

  describe('forecast sheet', () => {
    it('shows the five lines in the order the backend sends them', async () => {
      renderPage()

      await screen.findByText('taxModels.forecast.line.revenue')

      const rows = screen.getAllByRole('row').slice(1)
      expect(rows.map((row) => row.querySelector('th')?.textContent)).toEqual(FORECAST_LINES.map((line) => `taxModels.forecast.line.${line}`))
    })

    it('shows each cell with its amount and where the amount came from', async () => {
      ;(calculateTaxModel as Mock).mockResolvedValue(
        makeResult({
          lines: [
            makeLineResult('revenue', { soFar: makeCell(9000000, { source: 'typed' }), rest: makeCell(3000000, { source: 'average' }), yearMinor: 12000000 }),
            makeLineResult('payroll', { soFar: makeCell(0, { source: 'nothing' }), rest: makeCell(1200000, { source: 'monthly_amount' }), yearMinor: 1200000 }),
            makeLineResult('depreciation', { soFar: makeCell(315000, { source: 'schedule' }), rest: makeCell(105000, { source: 'schedule' }), yearMinor: 420000 }),
            makeLineResult('deductible_costs', { soFar: makeCell(1800000, { source: 'entries' }), rest: makeCell(600000, { source: 'average' }), yearMinor: 2400000 }),
            makeLineResult('non_deductible_costs', { soFar: makeCell(150000, { source: 'typed' }), rest: makeCell(50000, { source: 'typed' }), yearMinor: 200000 }),
          ],
        }),
      )
      renderPage()

      const revenueRow = (await screen.findByText('taxModels.forecast.line.revenue')).closest('tr')!
      expect(
        within(revenueRow)
          .getAllByTestId('number-value')
          .map((cell) => cell.textContent),
      ).toEqual(['9000000', '3000000', '12000000'])
      expect(within(revenueRow).getByText('taxModels.forecast.source.typed')).toBeInTheDocument()
      expect(within(revenueRow).getByText('taxModels.forecast.source.average')).toBeInTheDocument()

      const payrollRow = screen.getByText('taxModels.forecast.line.payroll').closest('tr')!
      expect(within(payrollRow).getByText('taxModels.forecast.source.nothing')).toBeInTheDocument()
      expect(within(payrollRow).getByText('taxModels.forecast.source.monthly_amount')).toBeInTheDocument()

      const depreciationRow = screen.getByText('taxModels.forecast.line.depreciation').closest('tr')!
      expect(within(depreciationRow).getAllByText('taxModels.forecast.source.schedule')).toHaveLength(2)

      const costsRow = screen.getByText('taxModels.forecast.line.deductible_costs').closest('tr')!
      expect(within(costsRow).getByText('taxModels.forecast.source.entries')).toBeInTheDocument()
    })

    it.each([
      [9, 'taxModels.forecast.actualAndProjected January September October December'],
      [12, 'taxModels.forecast.wholeYearActual'],
      [0, 'taxModels.forecast.wholeYearProjected'],
    ])('describes the actual and projected months when the actuals run through month %i', async (month, heading) => {
      ;(calculateTaxModel as Mock).mockResolvedValue(makeResult({ actualsThroughMonth: month }))
      renderPage()

      expect(await screen.findByText(heading)).toBeInTheDocument()
    })

    it('shows a sheet for a year without any entries instead of an empty state', async () => {
      ;(calculateTaxModel as Mock).mockResolvedValue(makeResult({ eventBreakdowns: [] }))
      renderPage()

      expect(await screen.findByText('taxModels.forecast.line.revenue')).toBeInTheDocument()
      expect(screen.queryByText('taxModels.noEventsForYear')).not.toBeInTheDocument()
      expect(screen.queryByText(/taxModels\.breakdownSection/)).not.toBeInTheDocument()
    })
  })

  describe('result summary', () => {
    it('leaves out the company rows for a physical person', async () => {
      ;(calculateTaxModel as Mock).mockResolvedValue(makeResult({ appliedFlatRateBps: 1500 }))
      renderPage()

      await screen.findByText('taxModels.totalTax')

      expect(screen.queryByText('taxModels.forecast.maximumDividend')).not.toBeInTheDocument()
      expect(screen.queryByText('taxModels.reserveFundAllocation')).not.toBeInTheDocument()
      expect(screen.queryByText('taxModels.forecast.corporateTaxAt 15%')).not.toBeInTheDocument()
    })

    it('shows the company rows with the rate the revenue picked', async () => {
      ;(calculateTaxModel as Mock).mockResolvedValue(
        makeResult({ personType: 'legal', appliedFlatRateBps: 2100, dividendFromYearMinor: 3260200, dividendFromYearTaxMinor: 228214, dividendMinor: 4760200 }),
      )
      renderPage()

      expect(await screen.findByText('taxModels.forecast.corporateTaxAt 21%')).toBeInTheDocument()
      expect(screen.getByText('taxModels.forecast.dividendFromYear 2026')).toBeInTheDocument()
      expect(screen.getByText('taxModels.forecast.maximumDividend')).toBeInTheDocument()
      expect(screen.getByText('taxModels.reserveFundAllocation')).toBeInTheDocument()
    })

    it('shows one row per earlier profit year with what is payable and its dividend tax', async () => {
      ;(calculateTaxModel as Mock).mockResolvedValue(
        makeResult({
          personType: 'legal',
          earlierProfits: [
            { profitYear: 2024, amountMinor: 400000, payableMinor: 0, dividendTaxRateBps: 1000, dividendTaxMinor: 0 },
            { profitYear: 2025, amountMinor: 300000, payableMinor: 166000, dividendTaxRateBps: 700, dividendTaxMinor: 11620 },
          ],
        }),
      )
      renderPage()

      const row2025 = (await screen.findByText('taxModels.forecast.profitLeftFrom 2025')).parentElement!.parentElement!
      expect(within(row2025).getByText('taxModels.forecast.dividendTaxAt 7%')).toBeInTheDocument()
      expect(
        within(row2025)
          .getAllByTestId('number-value')
          .map((value) => value.textContent),
      ).toEqual(['166000', '11620'])
      expect(screen.getByText('taxModels.forecast.profitLeftFrom 2024')).toBeInTheDocument()
    })

    it('says the minimum tax applies and shows the tax it replaced', async () => {
      ;(calculateTaxModel as Mock).mockResolvedValue(makeResult({ personType: 'legal', taxAmountMinor: 34000, computedTaxMinor: 30000, minimumTaxMinor: 34000 }))
      renderPage()

      const computedRow = (await screen.findByText('taxModels.forecast.minimumTaxApplies')).parentElement!
      expect(within(computedRow).getByTestId('number-value')).toHaveTextContent('30000')
      expect(screen.getByText('taxModels.totalTax')).toBeInTheDocument()
      expect(screen.queryByText(/taxModels\.forecast\.corporateTaxAt/)).not.toBeInTheDocument()
    })

    it('does not mention the minimum tax when it did not apply', async () => {
      renderPage()

      await screen.findByText('taxModels.totalTax')

      expect(screen.queryByText('taxModels.forecast.minimumTaxApplies')).not.toBeInTheDocument()
    })
  })

  describe('entry breakdown', () => {
    it('groups the entries by kind and shows why an uncounted entry is dimmed', async () => {
      const result: TaxCalculationResult = makeResult({
        eventBreakdowns: [
          makeBreakdown({ eventId: 1, kind: 'revenue', netAmountMinor: 500000 }),
          makeBreakdown({ eventId: 2, kind: 'revenue', netAmountMinor: 700000, notCounted: 'typed_value' }),
          makeBreakdown({ eventId: 3, kind: 'depreciation', taxDeductibleCostMinor: 35000 }),
          makeBreakdown({ eventId: 5, kind: 'prepaid', taxDeductibleCostMinor: 12000 }),
          makeBreakdown({ eventId: 4, kind: 'expense', notCounted: 'after_actual_months' }),
        ],
      })
      ;(calculateTaxModel as Mock).mockResolvedValue(result)
      renderPage()

      await screen.findByText('taxModels.breakdownSection (EUR)')

      const groupHeadings = document.querySelectorAll('td[colspan="9"]')
      expect(Array.from(groupHeadings, (cell) => cell.textContent)).toEqual([
        'taxModels.forecast.kind.revenue',
        'taxModels.forecast.kind.depreciation',
        'taxModels.forecast.kind.prepaid',
        'taxModels.forecast.kind.expense',
      ])
      const typedRow = screen.getByText('taxModels.forecast.reason.typed_value').closest('tr')!
      expect(typedRow).toHaveClass('opacity-50')
      expect(screen.getByText('taxModels.forecast.reason.after_actual_months').closest('tr')).toHaveClass('opacity-50')
    })

    it('leaves the entries that are not counted out of the group total', async () => {
      ;(calculateTaxModel as Mock).mockResolvedValue(
        makeResult({
          eventBreakdowns: [
            makeBreakdown({ eventId: 1, kind: 'revenue', netAmountMinor: 500001 }),
            makeBreakdown({ eventId: 2, kind: 'revenue', netAmountMinor: 700002, notCounted: 'typed_value' }),
          ],
        }),
      )
      renderPage()

      const totalRow = (await screen.findByText('taxModels.breakdownTotal')).closest('tr')!

      expect(within(totalRow).getAllByTestId('number-value')[1]).toHaveTextContent('500001')
    })
  })

  describe('navigation', () => {
    it('opens the figures page from Update figures', async () => {
      renderPage()

      fireEvent.click(await screen.findByText('taxModels.updateFigures'))

      expect(navigateMock).toHaveBeenCalledWith({ to: '/tax-models/$modelId/figures', params: { modelId: '5' } })
    })

    it('opens the settings from Adjust settings', async () => {
      renderPage()

      fireEvent.click(await screen.findByText('taxModels.adjustSettings'))

      expect(navigateMock).toHaveBeenCalledWith({ to: '/tax-models/$modelId/edit', params: { modelId: '5' } })
    })
  })
})
