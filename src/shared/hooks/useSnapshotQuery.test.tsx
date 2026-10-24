import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { SnapshotRow } from '../types'
import { useSnapshotQuery } from './useSnapshotQuery'
import { getAccountsSnapshot } from '../api'

vi.mock('../api', () => ({
  getAccountsSnapshot: vi.fn(),
}))

let selectedPersonId: number | null = null
vi.mock('../../app/useSelectedPerson', () => ({
  useSelectedPerson: () => ({ selectedPersonId, setSelectedPersonId: vi.fn() }),
}))

const row = (accountId: number) => ({ accountId, accountName: `Account ${accountId}` }) as SnapshotRow

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

  it('keeps the previous snapshot while the next person or date loads', async () => {
    const first = [row(1)]
    let resolveNext: (rows: SnapshotRow[]) => void = () => {}
    ;(getAccountsSnapshot as Mock).mockResolvedValueOnce(first).mockReturnValueOnce(
      new Promise<SnapshotRow[]>((resolve) => {
        resolveNext = resolve
      }),
    )

    const { result, rerender } = renderHook(() => useSnapshotQuery('2026-01-15'), { wrapper: makeWrapper() })
    await waitFor(() => expect(result.current.data).toEqual(first))

    selectedPersonId = 2
    rerender()

    await waitFor(() => expect(getAccountsSnapshot).toHaveBeenCalledTimes(2))
    expect(result.current.isPending).toBe(false)
    expect(result.current.isPlaceholderData).toBe(true)
    expect(result.current.data).toEqual(first)

    const next = [row(2)]
    resolveNext(next)
    await waitFor(() => expect(result.current.data).toEqual(next))
    expect(result.current.isPlaceholderData).toBe(false)
  })
})
