import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook } from '@testing-library/react'
import type { EventWithData } from '../types'
import { useEditEventHandler } from './useEditEventHandler'
import { getEventById } from '../api'

vi.mock('../api', () => ({
  getEventById: vi.fn(),
}))

const mockSetModalState = vi.fn()
vi.mock('../../app/useModal', () => ({
  useModal: () => ({ setModalState: mockSetModalState, closeModal: vi.fn(), modalState: { type: 'none' } }),
}))

const mockToastError = vi.fn()
vi.mock('sonner', () => ({
  toast: { error: (...args: unknown[]) => mockToastError(...args) },
}))

vi.mock('react-i18next', () => {
  const t = (key: string) => key
  return { useTranslation: () => ({ t }) }
})

const makeEvent = (overrides?: Partial<EventWithData>): EventWithData =>
  ({
    id: 1,
    accountId: 1,
    accountName: 'Checking',
    accountType: 'account',
    eventType: 'balance_update',
    eventDate: '2026-01-15T10:00:00',
    amountMinor: 100000,
    note: null,
    linkedEventId: null,
    splitGroupId: null,
    ...overrides,
  }) as EventWithData

const editEvent = async (event: EventWithData) => {
  const { result } = renderHook(() => useEditEventHandler())
  await result.current(event)
}

describe('useEditEventHandler', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('opens the balance update editor for a balance update', async () => {
    const event = makeEvent({ eventType: 'balance_update' })
    await editEvent(event)
    expect(mockSetModalState).toHaveBeenCalledWith({ type: 'editBalanceUpdate', event })
  })

  it.each(['revenue', 'expense', 'prepaid_expense'])('opens the taxable event editor for a %s event', async (eventType) => {
    const event = makeEvent({ eventType })
    await editEvent(event)
    expect(mockSetModalState).toHaveBeenCalledWith({ type: 'editTaxableEvent', event })
  })

  it('opens the transfer editor with the outgoing leg as the from event', async () => {
    const incoming = makeEvent({ id: 2, eventType: 'transfer', amountMinor: 5000, linkedEventId: 3 })
    const outgoing = makeEvent({ id: 3, eventType: 'transfer', amountMinor: -5000, linkedEventId: 2 })
    ;(getEventById as Mock).mockResolvedValue(outgoing)

    await editEvent(incoming)

    expect(getEventById).toHaveBeenCalledWith(3)
    expect(mockSetModalState).toHaveBeenCalledWith({ type: 'editTransfer', fromEvent: outgoing, toEvent: incoming })
  })

  it('toasts a dedicated message when a transfer has no linked event id', async () => {
    await editEvent(makeEvent({ eventType: 'transfer', linkedEventId: null }))

    expect(mockToastError).toHaveBeenCalledWith('errors.linkedEventMissing')
    expect(getEventById).not.toHaveBeenCalled()
    expect(mockSetModalState).not.toHaveBeenCalled()
  })

  it('toasts a dedicated message when the linked transfer event is gone', async () => {
    ;(getEventById as Mock).mockResolvedValue(null)

    await editEvent(makeEvent({ eventType: 'transfer', linkedEventId: 9 }))

    expect(mockToastError).toHaveBeenCalledWith('errors.linkedEventMissing')
    expect(mockSetModalState).not.toHaveBeenCalled()
  })

  it('toasts the backend error when loading the linked event fails', async () => {
    ;(getEventById as Mock).mockRejectedValue(new Error('db locked'))

    await editEvent(makeEvent({ eventType: 'transfer', linkedEventId: 9 }))

    expect(mockToastError).toHaveBeenCalledWith('db locked')
    expect(mockSetModalState).not.toHaveBeenCalled()
  })

  it('does nothing for event types without an editor', async () => {
    await editEvent(makeEvent({ eventType: 'cashflow' }))

    expect(mockSetModalState).not.toHaveBeenCalled()
    expect(mockToastError).not.toHaveBeenCalled()
  })
})
