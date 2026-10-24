import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, waitFor, act } from '@testing-library/react'
import { createElement, type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { usePartners } from './usePartners'

vi.mock('../../shared/api', () => ({
  listPartnerAccounts: vi.fn(),
  createPartnerAccount: vi.fn(),
  updatePartnerAccount: vi.fn(),
  deletePartnerAccount: vi.fn(),
  getConsolidationCurrency: vi.fn(),
}))

import { listPartnerAccounts, updatePartnerAccount, deletePartnerAccount, getConsolidationCurrency } from '../../shared/api'

const setup = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
  const wrapper = ({ children }: { children: ReactNode }) => createElement(QueryClientProvider, { client: queryClient }, children)
  const hook = renderHook(() => usePartners(), { wrapper })
  const invalidatedKeys = () => invalidate.mock.calls.map(([filters]) => (filters?.queryKey as string[])[0])
  return { hook, invalidatedKeys }
}

describe('usePartners', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(listPartnerAccounts as Mock).mockResolvedValue([])
    ;(getConsolidationCurrency as Mock).mockResolvedValue({ id: 1 })
    ;(updatePartnerAccount as Mock).mockResolvedValue(undefined)
    ;(deletePartnerAccount as Mock).mockResolvedValue(undefined)
  })

  it('invalidates events after a partner is renamed', async () => {
    const { hook, invalidatedKeys } = setup()
    await waitFor(() => expect(hook.result.current.loading).toBe(false))

    await act(() => hook.result.current.handleUpdate(5, 'Renamed', 'SK3112000000198742637541'))

    expect(invalidatedKeys()).toContain('events')
  })

  it('invalidates events after a partner is deleted', async () => {
    const { hook, invalidatedKeys } = setup()
    await waitFor(() => expect(hook.result.current.loading).toBe(false))

    await act(() => hook.result.current.handleDelete(5))

    expect(invalidatedKeys()).toContain('events')
  })
})
