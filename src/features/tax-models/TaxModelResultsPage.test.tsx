import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import TaxModelResultsPage from './TaxModelResultsPage'
import type { TaxCalculationResult, TaxModelDetail } from '../../shared/types'

vi.mock('../../shared/api', () => ({
  getTaxModel: vi.fn(),
  calculateTaxModel: vi.fn(),
  getConsolidationCurrency: vi.fn(),
}))

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
  useParams: () => ({ modelId: '5' }),
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock('../../app/usePageHeader', () => ({ usePageTitle: vi.fn() }))

vi.mock('../../shared/ui/NumberValue', () => ({
  default: ({ value, minorUnits, currencyCode }: { value: number; minorUnits?: number; currencyCode?: string }) => (
    <span data-testid="number-value" data-minor-units={minorUnits} data-currency-code={currencyCode}>
      {value}
    </span>
  ),
}))

import { getTaxModel, calculateTaxModel, getConsolidationCurrency } from '../../shared/api'

const EUR = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }
const JPY = { id: 2, code: 'JPY', name: 'Japanese Yen', minorUnits: 0, isCustom: false }

const MODEL = { id: 5, name: 'Model', calendarYear: 2025 } as TaxModelDetail

const makeResult = (taxAmountMinor: number): TaxCalculationResult => ({
  totalIncomeMinor: 100000,
  totalTaxDeductibleExpensesMinor: 0,
  totalNonTaxDeductibleExpensesMinor: 0,
  taxBasisMinor: 100000,
  taxAmountMinor,
  totalProfitMinor: 100000 - taxAmountMinor,
  reserveFundGenerationMinor: 0,
  dividendMinor: 0,
  withholdingTaxMinor: 0,
  netDividendMinor: 0,
  monthlyTaxBurdenMinor: 0,
  personType: 'physical',
  eventBreakdowns: [
    {
      eventId: 1,
      eventType: 'revenue',
      eventDate: '2025-03-01T00:00:00',
      note: null,
      amountMinor: 100000,
      netAmountMinor: 100000,
      vatAmountMinor: 0,
      reclaimableVatMinor: 0,
      nonReclaimableVatMinor: 0,
      taxDeductibleCostMinor: 0,
      nonTaxDeductibleCostMinor: 0,
    },
  ],
})

describe('TaxModelResultsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(getTaxModel as Mock).mockResolvedValue(MODEL)
    ;(getConsolidationCurrency as Mock).mockResolvedValue(EUR)
  })

  it('shows amounts in the minor units and code of the consolidation currency', async () => {
    ;(getConsolidationCurrency as Mock).mockResolvedValue(JPY)
    ;(calculateTaxModel as Mock).mockResolvedValue(makeResult(11111))
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0, staleTime: Infinity } } })
    render(
      <QueryClientProvider client={queryClient}>
        <TaxModelResultsPage />
      </QueryClientProvider>,
    )

    const taxAmount = await screen.findByText('11111')
    expect(taxAmount).toHaveAttribute('data-minor-units', '0')
    expect(taxAmount).toHaveAttribute('data-currency-code', 'JPY')
    const values = screen.getAllByTestId('number-value')
    expect(values.length).toBeGreaterThan(10)
    for (const value of values) {
      expect(value).toHaveAttribute('data-minor-units', '0')
    }
  })

  it('recalculates the results every time the page is mounted', async () => {
    ;(calculateTaxModel as Mock).mockResolvedValueOnce(makeResult(11111)).mockResolvedValueOnce(makeResult(22222))
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0, staleTime: Infinity } } })
    const renderPage = () =>
      render(
        <QueryClientProvider client={queryClient}>
          <TaxModelResultsPage />
        </QueryClientProvider>,
      )

    const first = renderPage()
    expect(await screen.findByText('11111')).toBeInTheDocument()
    first.unmount()

    renderPage()

    expect(await screen.findByText('22222')).toBeInTheDocument()
    expect(screen.queryByText('11111')).not.toBeInTheDocument()
    expect(calculateTaxModel).toHaveBeenCalledTimes(2)
  })
})
