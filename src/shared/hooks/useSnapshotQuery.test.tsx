import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Snapshot } from '../types'
import { useSnapshotQuery } from './useSnapshotQuery'
import { getAccountsSnapshot } from '../api'
import { LIVE } from '../utils/snapshot'
import { makeRow, makeSnapshot } from '../testing/snapshotFixtures'

vi.mock('../api', () => ({
  getAccountsSnapshot: vi.fn(),
}))

let selectedPersonId: number | null = null
vi.mock('../../app/useSelectedPerson', () => ({
  useSelectedPerson: () => ({ selectedPersonId, setSelectedPersonId: vi.fn() }),
}))

const snapshotOf = (accountId: number) => makeSnapshot({ rows: [makeRow({ accountId, accountName: `Account ${accountId}` })] })

const makeWrapper = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const Wrapper = ({ children }: { children: React.ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  return Wrapper
}

describe('useSnapshotQuery', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    selectedPersonId = null
  })

  it('asks for the live snapshot without a date', async () => {
    ;(getAccountsSnapshot as Mock).mockResolvedValue(snapshotOf(1))

    const { result } = renderHook(() => useSnapshotQuery(LIVE), { wrapper: makeWrapper() })
    await waitFor(() => expect(result.current.data).toEqual(snapshotOf(1)))

    expect(getAccountsSnapshot).toHaveBeenCalledWith({ kind: 'live' }, undefined)
  })

  it('asks for the snapshot at a moment, for the selected person', async () => {
    selectedPersonId = 3
    ;(getAccountsSnapshot as Mock).mockResolvedValue(snapshotOf(1))

    const { result } = renderHook(() => useSnapshotQuery({ kind: 'at', until: '2026-10-20' }), { wrapper: makeWrapper() })
    await waitFor(() => expect(result.current.data).toBeDefined())

    expect(getAccountsSnapshot).toHaveBeenCalledWith({ kind: 'at', until: '2026-10-20' }, 3)
  })

  it('keeps the previous snapshot while the next person or moment loads', async () => {
    const first = snapshotOf(1)
    let resolveNext: (snapshot: Snapshot) => void = () => {}
    ;(getAccountsSnapshot as Mock).mockResolvedValueOnce(first).mockReturnValueOnce(
      new Promise<Snapshot>((resolve) => {
        resolveNext = resolve
      }),
    )

    const { result, rerender } = renderHook(() => useSnapshotQuery(LIVE), { wrapper: makeWrapper() })
    await waitFor(() => expect(result.current.data).toEqual(first))

    selectedPersonId = 2
    rerender()

    await waitFor(() => expect(getAccountsSnapshot).toHaveBeenCalledTimes(2))
    expect(result.current.isPending).toBe(false)
    expect(result.current.isPlaceholderData).toBe(true)
    expect(result.current.data).toEqual(first)

    const next = snapshotOf(2)
    resolveNext(next)
    await waitFor(() => expect(result.current.data).toEqual(next))
    expect(result.current.isPlaceholderData).toBe(false)
  })
})
