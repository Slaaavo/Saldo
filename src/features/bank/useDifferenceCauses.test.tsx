import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import type { ReactNode } from 'react'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useDifferenceCauses } from './useDifferenceCauses'
import { findDifferenceCauses } from '@/shared/api'

vi.mock('@/shared/api', () => ({ findDifferenceCauses: vi.fn() }))

const SEEN = 100000

let queryClient: QueryClient

const Providers = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>

interface Typed {
  typed: number | null
}

const nothingTyped: Typed = { typed: null }

const renderCauses = () => renderHook(({ typed }: Typed) => useDifferenceCauses(7, typed, SEEN), { wrapper: Providers, initialProps: nothingTyped })

beforeEach(() => {
  vi.clearAllMocks()
  queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0, staleTime: Infinity } } })
  ;(findDifferenceCauses as Mock).mockResolvedValue([])
})

describe('useDifferenceCauses', () => {
  it('asks once, for the balance typed last, when typing pauses', async () => {
    const { result, rerender } = renderCauses()

    rerender({ typed: 100 })
    rerender({ typed: 10100 })
    rerender({ typed: 101250 })

    await waitFor(() => expect(result.current).toEqual([]))
    expect(findDifferenceCauses).toHaveBeenCalledTimes(1)
    expect(findDifferenceCauses).toHaveBeenCalledWith(7, 101250, SEEN)
  })

  it('withholds the answer for an older balance while a newer one is typed', async () => {
    ;(findDifferenceCauses as Mock).mockImplementation(async (_accountId: number, typed: number) => (typed === 101250 ? [{ kind: 'staleImport', newestDay: null }] : []))
    const { result, rerender } = renderCauses()
    rerender({ typed: 101250 })
    await waitFor(() => expect(result.current).toEqual([{ kind: 'staleImport', newestDay: null }]))

    rerender({ typed: 104500 })

    expect(result.current).toBeUndefined()
    await waitFor(() => expect(result.current).toEqual([]))
  })

  it('asks again when the dialog opens again with the same balances', async () => {
    const first = renderCauses()
    first.rerender({ typed: 101250 })
    await waitFor(() => expect(first.result.current).toBeDefined())
    first.unmount()

    const second = renderCauses()
    second.rerender({ typed: 101250 })
    await waitFor(() => expect(second.result.current).toBeDefined())

    expect(findDifferenceCauses).toHaveBeenCalledTimes(2)
  })

  it('does not ask while nothing is typed or the balances match', async () => {
    const { rerender } = renderCauses()

    rerender({ typed: SEEN })
    await new Promise((resolve) => setTimeout(resolve, 400))

    expect(findDifferenceCauses).not.toHaveBeenCalled()
  })
})
