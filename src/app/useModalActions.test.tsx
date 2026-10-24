import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Currency, SnapshotRow } from '../shared/types'
import { useModalActions } from './useModalActions'
import { createBalanceUpdate, updateEvent, createBucketBalanceUpdate, listFxRates, updateAccount, setAccountAssetLinks } from '../shared/api'

vi.mock('../shared/api', () => ({
  createBalanceUpdate: vi.fn(),
  createAccount: vi.fn(),
  updateAccount: vi.fn(),
  deleteAccount: vi.fn(),
  updateEvent: vi.fn(),
  deleteEvent: vi.fn(),
  deleteSplitGroup: vi.fn(),
  bulkCreateBalanceUpdates: vi.fn(),
  listFxRates: vi.fn(),
  updateSortOrder: vi.fn(),
  updateAssetValue: vi.fn(),
  setAccountAssetLinks: vi.fn(),
  createBucketBalanceUpdate: vi.fn(),
  updateBucketBalanceUpdate: vi.fn(),
  updateTransfer: vi.fn(),
}))

const mockToastError = vi.fn()
vi.mock('sonner', () => ({
  toast: { error: (...args: unknown[]) => mockToastError(...args) },
}))

vi.mock('react-i18next', () => {
  const t = (key: string, opts?: Record<string, unknown>) => (opts ? `${key}:${JSON.stringify(opts)}` : key)
  return { useTranslation: () => ({ t }) }
})

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }

const makeRow = (overrides: Partial<SnapshotRow>): SnapshotRow =>
  ({ accountId: 1, accountName: 'Checking', accountType: 'account', currencyCode: 'EUR', ...overrides }) as SnapshotRow

let snapshotRows: SnapshotRow[] = []
vi.mock('../shared/hooks/useSnapshotQuery', () => ({
  useSnapshotQuery: () => ({ data: snapshotRows }),
}))
vi.mock('../shared/hooks/useConsolidationCurrencyQuery', () => ({
  useConsolidationCurrencyQuery: () => ({ data: EUR }),
}))

const closeModal = vi.fn()
const onFxRatePrompt = vi.fn()

const renderActions = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const wrapper = ({ children }: { children: React.ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  return renderHook(() => useModalActions({ closeModal, onFxRatePrompt }), { wrapper })
}

describe('useModalActions error messages', () => {
  it('says the account failed to update, not to rename, when editing an account fails', async () => {
    ;(updateAccount as Mock).mockRejectedValue(new Error('name taken'))
    const { result } = renderActions()

    await result.current.handleEditAccount(1, 'Checking')

    expect(mockToastError).toHaveBeenCalledWith('errors.updateAccount:{"error":"name taken"}')
  })

  it('uses the asset links message when saving account asset links fails', async () => {
    ;(setAccountAssetLinks as Mock).mockRejectedValue(new Error('conflict'))
    const { result } = renderActions()

    await result.current.handleSetAccountAssetLinks(1, [4])

    expect(mockToastError).toHaveBeenCalledWith('errors.setAccountAssetLinks:{"error":"conflict"}')
  })
})

describe('useModalActions balance updates', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    snapshotRows = [
      makeRow({ accountId: 1, currencyCode: 'EUR' }),
      makeRow({ accountId: 2, accountName: 'Dollars', currencyCode: 'USD' }),
      makeRow({ accountId: 3, accountType: 'bucket', currencyCode: 'USD' }),
    ]
    ;(createBalanceUpdate as Mock).mockResolvedValue(undefined)
    ;(updateEvent as Mock).mockResolvedValue(undefined)
    ;(createBucketBalanceUpdate as Mock).mockResolvedValue(undefined)
    ;(listFxRates as Mock).mockResolvedValue([])
  })

  describe('handleCreateBalanceUpdate', () => {
    it('rethrows a failed save without closing the modal or toasting itself', async () => {
      ;(createBalanceUpdate as Mock).mockRejectedValue(new Error('db locked'))
      const { result } = renderActions()

      await expect(result.current.handleCreateBalanceUpdate(1, 1000, '2026-01-15T23:59:00', '')).rejects.toThrow('db locked')

      expect(closeModal).not.toHaveBeenCalled()
      expect(mockToastError).not.toHaveBeenCalled()
    })

    it('closes the modal and skips the FX lookup for the consolidation currency', async () => {
      const { result } = renderActions()

      await result.current.handleCreateBalanceUpdate(1, 1000, '2026-01-15T23:59:00', '')

      expect(closeModal).toHaveBeenCalledOnce()
      expect(listFxRates).not.toHaveBeenCalled()
      expect(onFxRatePrompt).not.toHaveBeenCalled()
    })

    it('prompts for FX rates on the calendar day of a timed foreign-currency balance update', async () => {
      const { result } = renderActions()

      await result.current.handleCreateBalanceUpdate(2, 1000, '2026-01-15T23:59:00', '')

      expect(listFxRates).toHaveBeenCalledWith('2026-01-15')
      expect(onFxRatePrompt).toHaveBeenCalledWith('2026-01-15')
    })

    it('does not report a saved balance update as failed when the FX lookup fails', async () => {
      ;(listFxRates as Mock).mockRejectedValue(new Error('rates unavailable'))
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
      const { result } = renderActions()

      await expect(result.current.handleCreateBalanceUpdate(2, 1000, '2026-01-15T23:59:00', '')).resolves.toBeUndefined()

      expect(closeModal).toHaveBeenCalledOnce()
      expect(onFxRatePrompt).not.toHaveBeenCalled()
      expect(mockToastError).not.toHaveBeenCalled()
      consoleError.mockRestore()
    })
  })

  describe('handleEditBalanceUpdate', () => {
    it('rethrows a failed save without closing the modal', async () => {
      ;(updateEvent as Mock).mockRejectedValue(new Error('db locked'))
      const { result } = renderActions()

      await expect(result.current.handleEditBalanceUpdate(5, 1000, '2026-01-15T23:59:00', '')).rejects.toThrow('db locked')

      expect(closeModal).not.toHaveBeenCalled()
    })

    it('closes the modal after a successful save', async () => {
      const { result } = renderActions()

      await result.current.handleEditBalanceUpdate(5, 1000, '2026-01-15T23:59:00', '')

      expect(closeModal).toHaveBeenCalledOnce()
    })
  })

  describe('handleCreateBucketBalanceUpdate', () => {
    it('closes the modal before opening the FX rate prompt', async () => {
      const { result } = renderActions()

      await result.current.handleCreateBucketBalanceUpdate(3, 1000, '2026-01-15T23:59:00', null, [])

      expect(onFxRatePrompt).toHaveBeenCalledWith('2026-01-15')
      expect(closeModal.mock.invocationCallOrder[0]).toBeLessThan(onFxRatePrompt.mock.invocationCallOrder[0])
    })

    it('does not close the modal when the save fails', async () => {
      ;(createBucketBalanceUpdate as Mock).mockRejectedValue(new Error('link conflict'))
      const { result } = renderActions()

      await expect(result.current.handleCreateBucketBalanceUpdate(3, 1000, '2026-01-15T23:59:00', null, [])).rejects.toThrow('link conflict')

      expect(closeModal).not.toHaveBeenCalled()
    })

    it('does not report a saved bucket balance update as failed when the FX lookup fails', async () => {
      ;(listFxRates as Mock).mockRejectedValue(new Error('rates unavailable'))
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
      const { result } = renderActions()

      await expect(result.current.handleCreateBucketBalanceUpdate(3, 1000, '2026-01-15T23:59:00', null, [])).resolves.toBeUndefined()

      expect(closeModal).toHaveBeenCalledOnce()
      consoleError.mockRestore()
    })
  })
})
