import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { toast } from 'sonner'
import ReviewStep from './ReviewStep'
import type { SplitLegDraft } from '../transactions/splitLegDraft'

vi.mock('../../shared/api', () => ({
  createTaxableEvent: vi.fn(),
  createTaxableSplitGroup: vi.fn(),
  linkCashflowsToTaxable: vi.fn(),
  linkCashflowsToSplitGroup: vi.fn(),
  listEligibleCashflows: vi.fn(),
}))

vi.mock('sonner', () => ({ toast: { error: vi.fn(), warning: vi.fn() } }))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock('../transactions/useResolvedPersonId', () => ({
  useResolvedPersonId: () => ({ personId: 1, persons: [] }),
}))

vi.mock('../../shared/hooks/useConsolidationCurrencyQuery', () => ({
  useConsolidationCurrencyQuery: () => ({ data: { code: 'EUR', minorUnits: 2 } }),
}))

vi.mock('../../shared/ui/date-picker', () => ({
  DatePicker: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <input data-testid="date-picker" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}))

vi.mock('../transactions/TaxableEventSplitEditor', () => ({ default: () => null }))
vi.mock('../transactions/ExpenseFormFields', () => ({ default: () => null }))

vi.mock('../transactions/CashflowPicker', () => ({
  default: ({ onToggle }: { onToggle: (id: number) => void }) => (
    <button type="button" onClick={() => onToggle(99)}>
      pick-cashflow
    </button>
  ),
}))

import { createTaxableEvent, createTaxableSplitGroup, linkCashflowsToTaxable, linkCashflowsToSplitGroup, listEligibleCashflows } from '../../shared/api'

const makeLeg = (amount: string, eventDate: string): SplitLegDraft => ({
  eventDate,
  amount,
  vatRate: '20',
  vatReclaimablePct: '100',
  expenseDeductiblePct: '100',
  note: amount,
  reclaimedVat: false,
})

const deferred = () => {
  let resolve!: () => void
  const promise = new Promise<void>((res) => {
    resolve = res
  })
  return { promise, resolve }
}

const renderStep = (processedLegs: SplitLegDraft[], onImportSuccess = vi.fn()) => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
  render(
    <QueryClientProvider client={queryClient}>
      <ReviewStep processedLegs={processedLegs} vendorName="Shop" receiptDate="2025-05-03" onImportSuccess={onImportSuccess} onBack={vi.fn()} />
    </QueryClientProvider>,
  )
  return { invalidate, onImportSuccess }
}

describe('ReviewStep', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(listEligibleCashflows as Mock).mockResolvedValue([])
    ;(createTaxableEvent as Mock).mockResolvedValue(5)
    ;(createTaxableSplitGroup as Mock).mockResolvedValue(8)
    ;(linkCashflowsToTaxable as Mock).mockResolvedValue(undefined)
    ;(linkCashflowsToSplitGroup as Mock).mockResolvedValue(undefined)
  })

  it('saves every leg of a multi-item receipt on the date chosen in the picker', async () => {
    renderStep([makeLeg('1.00', '2025-05-03'), makeLeg('2.00', '2025-05-03')])

    fireEvent.change(screen.getByTestId('date-picker'), { target: { value: '2025-06-01' } })
    fireEvent.click(screen.getByText('ekasaImport.reviewStep.save'))

    await waitFor(() => expect(createTaxableSplitGroup).toHaveBeenCalledOnce())
    const input = (createTaxableSplitGroup as Mock).mock.calls[0][0]
    expect(input.legs.map((leg: { eventDate: string }) => leg.eventDate)).toEqual(['2025-06-01', '2025-06-01'])
  })

  it('waits for the cashflow link before refreshing events and snapshots', async () => {
    const link = deferred()
    ;(linkCashflowsToTaxable as Mock).mockReturnValue(link.promise)
    const { invalidate, onImportSuccess } = renderStep([makeLeg('5.00', '2025-05-03')])

    fireEvent.click(screen.getByText('pick-cashflow'))
    fireEvent.click(screen.getByText('ekasaImport.reviewStep.save'))

    await waitFor(() => expect(linkCashflowsToTaxable).toHaveBeenCalledWith(5, [99]))
    expect(invalidate).not.toHaveBeenCalledWith({ queryKey: ['events'] })

    link.resolve()

    await waitFor(() => expect(onImportSuccess).toHaveBeenCalledOnce())
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['events'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['snapshot'] })
  })

  it('warns but still finishes the import when linking the cashflow fails', async () => {
    ;(linkCashflowsToSplitGroup as Mock).mockRejectedValue({ code: 'LINK', message: 'Cashflow already linked' })
    const { onImportSuccess } = renderStep([makeLeg('1.00', '2025-05-03'), makeLeg('2.00', '2025-05-03')])

    fireEvent.click(screen.getByText('pick-cashflow'))
    fireEvent.click(screen.getByText('ekasaImport.reviewStep.save'))

    await waitFor(() => expect(onImportSuccess).toHaveBeenCalledOnce())
    expect(toast.warning).toHaveBeenCalledWith('Cashflow already linked')
  })
})
