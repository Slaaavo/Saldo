import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import type { Currency, EventWithData, SnapshotRow } from '../../../shared/types'
import { useImportWizard } from './useImportWizard'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (!opts) return key
      let result = key
      for (const [k, v] of Object.entries(opts)) result = result.replace(`{{${k}}}`, String(v))
      return result
    },
    i18n: { language: 'en' },
  }),
}))

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn(), warning: vi.fn() },
}))

vi.mock('../../../shared/api', () => ({
  listEvents: vi.fn(),
  listPartnerAccounts: vi.fn(),
  listCurrencies: vi.fn(),
  createPartnerAccount: vi.fn(),
  updateAccount: vi.fn(),
  updateEvent: vi.fn(),
  bulkCreateCashflows: vi.fn(),
  createSplitGroup: vi.fn(),
  listImportProfiles: vi.fn(),
  getPreferredProfile: vi.fn(),
  setPreferredProfile: vi.fn(),
  createImportProfile: vi.fn(),
  updateImportProfile: vi.fn(),
}))

import {
  listEvents,
  listPartnerAccounts,
  listCurrencies,
  bulkCreateCashflows,
  createSplitGroup,
  listImportProfiles,
  getPreferredProfile,
  setPreferredProfile,
} from '../../../shared/api'

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }

const makeAccount = (overrides: Partial<SnapshotRow>): SnapshotRow => ({
  accountId: 1,
  accountName: 'Checking',
  accountType: 'account',
  iban: null,
  balanceMinor: 0,
  currencyCode: 'EUR',
  currencyMinorUnits: 2,
  isCustom: false,
  convertedBalanceMinor: 0,
  fxRateMissing: false,
  isLinkedToAsset: false,
  linkedAssetIds: [],
  isBucketLinked: false,
  bucketLinks: [],
  linkedBalanceMinor: 0,
  cashflowTaggedMinor: 0,
  personId: null,
  purchasePriceMinor: null,
  purchaseDate: null,
  depreciationPeriodMonths: null,
  ...overrides,
})

const makeEvent = (overrides: Partial<EventWithData>): EventWithData => ({
  id: 100,
  accountId: 1,
  accountName: 'Checking',
  accountType: 'account',
  eventType: 'cashflow',
  eventDate: '2026-03-01T12:00:00',
  amountMinor: 0,
  note: null,
  createdAt: '2026-03-01T12:00:00',
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
  vatRateBps: null,
  vatReclaimablePctBps: null,
  expenseDeductiblePctBps: null,
  prepaidUntil: null,
  isLinkedToTaxable: false,
  linkedTaxableEventId: null,
  hasLinkedCashflows: false,
  linkedCashflowCount: 0,
  linkedAssetId: null,
  linkedPrepaidEventId: null,
  isSystemGenerated: false,
  reclaimedVat: null,
  ...overrides,
})

const SNAPSHOT: SnapshotRow[] = [makeAccount({ accountId: 1, accountName: 'Checking' })]

const setupMocks = (overrides?: { events?: EventWithData[] }) => {
  ;(listEvents as Mock).mockResolvedValue({ events: overrides?.events ?? [] })
  ;(listPartnerAccounts as Mock).mockResolvedValue([])
  ;(listCurrencies as Mock).mockResolvedValue([EUR])
  ;(listImportProfiles as Mock).mockResolvedValue([])
  ;(getPreferredProfile as Mock).mockResolvedValue(null)
  ;(setPreferredProfile as Mock).mockResolvedValue(undefined)
  ;(bulkCreateCashflows as Mock).mockResolvedValue(undefined)
  ;(createSplitGroup as Mock).mockResolvedValue(1)
}

const renderWizard = (snapshot: SnapshotRow[] = SNAPSHOT) => {
  const onClose = vi.fn()
  const onSuccess = vi.fn().mockResolvedValue(undefined)
  const hook = renderHook(() => useImportWizard({ snapshot, onClose, onSuccess }))
  return { ...hook, onClose, onSuccess }
}

const csvFile = (content: string) => new File([content], 'bank.csv', { type: 'text/csv' })

const renderAtReview = async (csv: string, snapshot: SnapshotRow[] = SNAPSHOT) => {
  const wizard = renderWizard(snapshot)
  await act(() => wizard.result.current.handleFileSelect(csvFile(csv)))
  await act(() => wizard.result.current.handleAccountSelect(1))
  act(() => wizard.result.current.goToMapping())
  act(() => wizard.result.current.goToReview())
  return wizard
}

describe('useImportWizard', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    setupMocks()
  })

  describe('import', () => {
    it('reports a split that has only one leg', async () => {
      const { result } = await renderAtReview('Date,Amount\n2026-03-01,-50.00')

      act(() => result.current.handleSplitOpen(0))

      expect(result.current.splitValidationErrors).toEqual(['import.reviewStep.split.minLegsError'])
    })

    it('drops committed rows and refreshes data when a later split fails, so a retry does not insert them again', async () => {
      const { result, onSuccess } = await renderAtReview('Date,Amount\n2026-03-01,-10.00\n2026-03-02,-50.00')
      act(() => result.current.handleSplitOpen(1))
      act(() => result.current.handleLegAmountChange(1, 0, -2000))
      act(() => result.current.handleAddLeg(1))
      ;(createSplitGroup as Mock).mockRejectedValueOnce(new Error('boom'))

      await act(() => result.current.handleImport())

      expect(bulkCreateCashflows).toHaveBeenCalledTimes(1)
      expect(onSuccess).toHaveBeenCalledTimes(1)
      expect(result.current.wizardState.importRows.map((r) => r.index)).toEqual([1])

      await act(() => result.current.handleImport())

      expect(bulkCreateCashflows).toHaveBeenCalledTimes(1)
      expect(createSplitGroup).toHaveBeenCalledTimes(2)
    })

    it('refreshes data before switching to the save-profile step', async () => {
      const { result, onSuccess, onClose } = await renderAtReview('Date,Amount\n2026-03-01,-10.00')

      await act(() => result.current.handleImport())

      expect(onSuccess).toHaveBeenCalledTimes(1)
      expect(result.current.wizardState.step).toBe('save-profile')
      expect(onClose).not.toHaveBeenCalled()
    })

    it('imports a duplicate row once the user ticks it', async () => {
      setupMocks({ events: [makeEvent({ eventDate: '2026-03-01T12:00:00', amountMinor: -1000 })] })
      const { result } = await renderAtReview('Date,Amount\n2026-03-01,-10.00')
      expect(result.current.wizardState.importRows[0]).toMatchObject({ isDuplicate: true, isSelected: false })

      act(() => result.current.handleToggleRow(0))
      await act(() => result.current.handleImport())

      expect(bulkCreateCashflows).toHaveBeenCalledWith({ entries: [expect.objectContaining({ amountMinor: -1000 })] })
    })
  })

  describe('split legs', () => {
    it('keeps leg ids stable when a leg is removed', async () => {
      const { result } = await renderAtReview('Date,Amount\n2026-03-01,-60.00')
      act(() => result.current.handleSplitOpen(0))
      act(() => result.current.handleLegAmountChange(0, 0, -1000))
      act(() => result.current.handleAddLeg(0))
      act(() => result.current.handleLegAmountChange(0, 1, -2000))
      act(() => result.current.handleAddLeg(0))

      act(() => result.current.handleRemoveLeg(0, 1))
      act(() => result.current.handleLegAmountChange(0, 2, -5000))

      expect(result.current.wizardState.importRows[0].splitLegs!.map((l) => [l.legId, l.amountMinor])).toEqual([
        [0, -1000],
        [2, -5000],
      ])

      act(() => result.current.handleAddLeg(0))
      expect(result.current.wizardState.importRows[0].splitLegs!.map((l) => l.legId)).toEqual([0, 2, 3])
    })
  })
})
