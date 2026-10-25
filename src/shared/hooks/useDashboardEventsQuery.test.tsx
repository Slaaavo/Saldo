import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { EventWithData } from '../types'
import { useDashboardEventsQuery } from './useDashboardEventsQuery'
import { listEvents } from '../api'
import { LIVE } from '../utils/snapshot'

vi.mock('../api', () => ({
  listEvents: vi.fn(),
}))

let selectedPersonId: number | null = null
vi.mock('../../app/useSelectedPerson', () => ({
  useSelectedPerson: () => ({ selectedPersonId, setSelectedPersonId: vi.fn() }),
}))

type EventsResult = { events: EventWithData[]; totalCount: number }

const result = (id: number): EventsResult => ({ events: [{ id } as EventWithData], totalCount: 1 })

const makeWrapper = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const Wrapper = ({ children }: { children: React.ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  return Wrapper
}

describe('useDashboardEventsQuery', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    selectedPersonId = null
  })

  it('sends no upper date when live, so entries dated after today are listed', async () => {
    ;(listEvents as Mock).mockResolvedValue(result(1))

    const { result: hook } = renderHook(() => useDashboardEventsQuery(LIVE), { wrapper: makeWrapper() })
    await waitFor(() => expect(hook.current.data).toBeDefined())

    expect(listEvents).toHaveBeenCalledWith({ beforeDate: undefined, limit: 20, personId: undefined })
  })

  it('lists up to the end of the picked day in the time machine', async () => {
    ;(listEvents as Mock).mockResolvedValue(result(1))

    const { result: hook } = renderHook(() => useDashboardEventsQuery({ kind: 'at', until: '2026-10-20' }), { wrapper: makeWrapper() })
    await waitFor(() => expect(hook.current.data).toBeDefined())

    expect(listEvents).toHaveBeenCalledWith({ beforeDate: '2026-10-20T23:59:59', limit: 20, personId: undefined })
  })

  it('keeps the previous events while the next person or date loads', async () => {
    const first = result(1)
    let resolveNext: (value: EventsResult) => void = () => {}
    ;(listEvents as Mock).mockResolvedValueOnce(first).mockReturnValueOnce(
      new Promise<EventsResult>((resolve) => {
        resolveNext = resolve
      }),
    )

    const { result: hook, rerender } = renderHook(() => useDashboardEventsQuery(LIVE), { wrapper: makeWrapper() })
    await waitFor(() => expect(hook.current.data).toEqual(first))

    selectedPersonId = 2
    rerender()

    await waitFor(() => expect(listEvents).toHaveBeenCalledTimes(2))
    expect(hook.current.isPending).toBe(false)
    expect(hook.current.isPlaceholderData).toBe(true)
    expect(hook.current.data).toEqual(first)

    const next = result(2)
    resolveNext(next)
    await waitFor(() => expect(hook.current.data).toEqual(next))
    expect(hook.current.isPlaceholderData).toBe(false)
  })
})
