import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import EditTaxableSplitGroupModal from './EditTaxableSplitGroupModal'
import type { EventWithData } from '../../shared/types'

vi.mock('../../shared/api', () => ({
  listPersons: vi.fn(),
  updateTaxableSplitGroup: vi.fn(),
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'en' } }),
}))

vi.mock('sonner', () => ({
  toast: { error: vi.fn(), success: vi.fn() },
}))

import { listPersons, updateTaxableSplitGroup } from '../../shared/api'

const leg = (id: number, privateUse: boolean) =>
  ({
    id,
    eventType: 'expense',
    eventDate: '2026-07-15T00:00:00',
    amountMinor: 5830,
    note: `Leg ${id}`,
    currencyCode: 'EUR',
    currencyMinorUnits: 2,
    vatRateBps: 2300,
    vatReclaimablePctBps: 5000,
    expenseDeductiblePctBps: 8000,
    reclaimedVat: true,
    privateUse,
  }) as EventWithData

const renderModal = (legs: EventWithData[]) => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  render(
    <QueryClientProvider client={queryClient}>
      <EditTaxableSplitGroupModal splitGroupId={3} eventType="expense" legs={legs} groupNote={null} accountId={1} onClose={vi.fn()} />
    </QueryClientProvider>,
  )
}

const save = async () => {
  fireEvent.click(screen.getByText('modals.editTaxableSplitGroup.submit'))
  await waitFor(() => expect(updateTaxableSplitGroup).toHaveBeenCalledOnce())
  return (updateTaxableSplitGroup as Mock).mock.calls[0][0]
}

describe('EditTaxableSplitGroupModal private-use switch', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(updateTaxableSplitGroup as Mock).mockResolvedValue(undefined)
    ;(listPersons as Mock).mockResolvedValue([])
  })

  it('sends a leg whose only change is the switch', async () => {
    renderModal([leg(1, false), leg(2, false)])

    fireEvent.click(screen.getAllByLabelText('modals.createExpense.privateUse')[0])

    const { updatedLegs } = await save()
    expect(updatedLegs).toHaveLength(1)
    expect(updatedLegs[0]).toMatchObject({ eventId: 1, privateUse: true })
  })

  it('keeps the switch on a leg edited for another reason', async () => {
    renderModal([leg(1, true), leg(2, false)])

    fireEvent.change(screen.getAllByDisplayValue('Leg 1')[0], { target: { value: 'Diesel' } })

    const { updatedLegs } = await save()
    expect(updatedLegs[0]).toMatchObject({ eventId: 1, note: 'Diesel', privateUse: true })
  })

  it('leaves unchanged legs out', async () => {
    renderModal([leg(1, true), leg(2, false)])

    expect((await save()).updatedLegs).toEqual([])
  })
})
