import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import EditTaxableEventModal from './EditTaxableEventModal'
import type { EventWithData } from '../../shared/types'

vi.mock('../../shared/api', () => ({
  updateTaxableEvent: vi.fn(),
  listLinkedCashflows: vi.fn(),
  linkCashflowsToTaxable: vi.fn(),
  unlinkCashflowFromTaxable: vi.fn(),
  listPersons: vi.fn(),
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'en' } }),
}))

vi.mock('../../shared/ui/date-picker', () => ({
  DatePicker: () => <input />,
}))

vi.mock('./CashflowPicker', () => ({
  default: () => null,
}))

vi.mock('sonner', () => ({
  toast: { error: vi.fn(), success: vi.fn() },
}))

import { listLinkedCashflows, listPersons, updateTaxableEvent } from '../../shared/api'

const diesel = {
  id: 7,
  eventType: 'expense',
  eventDate: '2026-07-15T00:00:00',
  amountMinor: 5830,
  note: 'Diesel',
  currencyCode: 'EUR',
  currencyMinorUnits: 2,
  vatRateBps: 2300,
  vatReclaimablePctBps: 5000,
  expenseDeductiblePctBps: 8000,
  prepaidUntil: null,
  reclaimedVat: true,
  privateUse: true,
} as EventWithData

const renderModal = (event: EventWithData) => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  render(
    <QueryClientProvider client={queryClient}>
      <EditTaxableEventModal event={event} onClose={vi.fn()} />
    </QueryClientProvider>,
  )
}

const save = async () => {
  fireEvent.click(screen.getByText('modals.editTaxableEvent.submit'))
  await waitFor(() => expect(updateTaxableEvent).toHaveBeenCalledOnce())
  return (updateTaxableEvent as Mock).mock.calls[0][0]
}

describe('EditTaxableEventModal private-use switch', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(listLinkedCashflows as Mock).mockResolvedValue([])
    ;(listPersons as Mock).mockResolvedValue([])
    ;(updateTaxableEvent as Mock).mockResolvedValue(undefined)
  })

  it('keeps the switch when the entry is saved', async () => {
    renderModal(diesel)

    expect((await save()).privateUse).toBe(true)
  })

  it('drops the switch once the entry reclaims all of its VAT, so it cannot come back unseen', async () => {
    renderModal(diesel)

    fireEvent.change(screen.getByLabelText('modals.createExpense.vatReclaimablePct'), { target: { value: '100' } })

    expect((await save()).privateUse).toBe(false)
  })

  it('keeps the switch on a prepaid expense', async () => {
    renderModal({ ...diesel, eventType: 'prepaid_expense', prepaidUntil: '2027-07-14' })

    expect((await save()).privateUse).toBe(true)
  })

  it('sends false for revenue', async () => {
    renderModal({ ...diesel, eventType: 'revenue', privateUse: false })

    expect((await save()).privateUse).toBe(false)
  })
})
