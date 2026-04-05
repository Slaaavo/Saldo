import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import React from 'react'
import { renderHook, waitFor, act } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useLedgerData } from './useLedgerData'
import type { EventWithData, SnapshotRow } from '../../shared/types'

// ── Mock the API layer ──────────────────────────────────────────────────────
vi.mock('../../shared/api', () => ({
  listEvents: vi.fn(),
}))

vi.mock('react-i18next', () => {
  // Stable `t` reference prevents useCallback deps from changing on every render
  const t = (key: string, opts?: Record<string, unknown>) => {
    if (opts) {
      let result = key
      for (const [k, v] of Object.entries(opts)) {
        result = result.replace(`{{${k}}}`, String(v))
      }
      return result
    }
    return key
  }
  return {
    useTranslation: () => ({ t }),
  }
})

import { listEvents } from '../../shared/api'

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: 0, refetchOnWindowFocus: false } },
  })
  return ({ children }: { children: React.ReactNode }) => React.createElement(QueryClientProvider, { client: queryClient }, children)
}

const makeSnapshot = (overrides?: Partial<SnapshotRow>): SnapshotRow => {
  return {
    accountId: 1,
    accountName: 'Checking',
    accountType: 'account',
    iban: null,
    balanceMinor: 100000,
    currencyCode: 'EUR',
    currencyMinorUnits: 2,
    isCustom: false,
    convertedBalanceMinor: 100000,
    fxRateMissing: false,
    isBucketLinked: false,
    bucketLinks: [],
    linkedBalanceMinor: 0,
    cashflowTaggedMinor: 0,
    isLinkedToAsset: false,
    linkedAssetIds: [],
    ...overrides,
  }
}

const makeEvent = (overrides?: Partial<EventWithData>): EventWithData => {
  return {
    id: 1,
    accountId: 1,
    accountName: 'Checking',
    accountType: 'account',
    eventType: 'balance_update',
    eventDate: '2026-01-15T10:00:00',
    amountMinor: 100000,
    note: null,
    createdAt: '2026-01-15T10:00:00',
    currencyCode: 'EUR',
    currencyMinorUnits: 2,
    counterpartAccountId: null,
    counterpartAccountName: null,
    bucketId: null,
    bucketName: null,
    originalCurrencyId: null,
    originalCurrencyCode: null,
    originalAmountMinor: null,
    originalCurrencyMinorUnits: null,
    fxRateMantissa: null,
    fxRateExponent: null,
    linkedEventId: null,
    splitGroupId: null,
    splitGroupNote: null,
    ...overrides,
  }
}

describe('useLedgerData', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(listEvents as Mock).mockResolvedValue({ events: [makeEvent()], totalCount: 1 })
  })

  // ── Initial load ──────────────────────────────────────────────────────────

  it('loads events on mount with no filters', async () => {
    const { result } = renderHook(() => useLedgerData(), { wrapper: createWrapper() })

    await waitFor(() => {
      expect(result.current.events).toHaveLength(1)
    })

    expect(listEvents).toHaveBeenCalledWith({})
  })

  it('starts with empty filter state', () => {
    const { result } = renderHook(() => useLedgerData(), { wrapper: createWrapper() })

    expect(result.current.fromDate).toBe('')
    expect(result.current.toDate).toBe('')
    expect(result.current.selectedAccountIds).toEqual([])
  })

  // ── Filter state management ───────────────────────────────────────────────

  it('updates fromDate and re-fetches with fromDate filter', async () => {
    const { result } = renderHook(() => useLedgerData(), { wrapper: createWrapper() })

    await waitFor(() => expect(result.current.events).toHaveLength(1))
    vi.clearAllMocks()
    ;(listEvents as Mock).mockResolvedValue({ events: [], totalCount: 0 })

    act(() => {
      result.current.setFromDate('2026-01-01')
    })

    await waitFor(() => {
      expect(listEvents).toHaveBeenCalledWith(expect.objectContaining({ fromDate: '2026-01-01T00:00:00' }))
    })
  })

  it('updates toDate and re-fetches with beforeDate filter', async () => {
    const { result } = renderHook(() => useLedgerData(), { wrapper: createWrapper() })

    await waitFor(() => expect(result.current.events).toHaveLength(1))
    vi.clearAllMocks()
    ;(listEvents as Mock).mockResolvedValue({ events: [], totalCount: 0 })

    act(() => {
      result.current.setToDate('2026-01-31')
    })

    await waitFor(() => {
      expect(listEvents).toHaveBeenCalledWith(expect.objectContaining({ beforeDate: '2026-01-31T23:59:59' }))
    })
  })

  it('passes accountIds when selectedAccountIds is non-empty', async () => {
    const { result } = renderHook(() => useLedgerData(), { wrapper: createWrapper() })

    await waitFor(() => expect(result.current.events).toHaveLength(1))
    vi.clearAllMocks()
    ;(listEvents as Mock).mockResolvedValue({ events: [], totalCount: 0 })

    act(() => {
      result.current.setSelectedAccountIds([1, 2, 3])
    })

    await waitFor(() => {
      expect(listEvents).toHaveBeenCalledWith(expect.objectContaining({ accountIds: [1, 2, 3] }))
    })
  })

  it('does not pass accountIds when selectedAccountIds is empty', async () => {
    const { result } = renderHook(() => useLedgerData(), { wrapper: createWrapper() })

    await waitFor(() => expect(result.current.events).toHaveLength(1))

    expect(listEvents).toHaveBeenCalledWith({})
  })

  // ── Bucket ID splitting ───────────────────────────────────────────────────

  it('routes bucket IDs to both accountIds and bucketIds', async () => {
    const bucketId = 42
    const snapshot = [makeSnapshot({ accountId: bucketId, accountType: 'bucket' })]
    const { result } = renderHook(() => useLedgerData({ snapshot }), { wrapper: createWrapper() })

    await waitFor(() => expect(result.current.events).toHaveLength(1))
    vi.clearAllMocks()
    ;(listEvents as Mock).mockResolvedValue({ events: [], totalCount: 0 })

    act(() => {
      result.current.setSelectedAccountIds([bucketId])
    })

    await waitFor(() => {
      expect(listEvents).toHaveBeenCalledWith(expect.objectContaining({ accountIds: [bucketId], bucketIds: [bucketId] }))
    })
  })
})
