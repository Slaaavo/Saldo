import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import type { Currency, EventWithData, ImportProfileRow, SnapshotRow } from '../../../shared/types'
import { toast } from 'sonner'
import { useImportWizard } from './useImportWizard'
import { makeRow } from '../../../shared/testing/snapshotFixtures'

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
  createPartnerAccount,
  updateAccount,
  bulkCreateCashflows,
  createSplitGroup,
  listImportProfiles,
  getPreferredProfile,
  setPreferredProfile,
} from '../../../shared/api'

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }
const JPY: Currency = { id: 3, code: 'JPY', name: 'Japanese Yen', minorUnits: 0, isCustom: false }

const makeAccount = (overrides: Partial<SnapshotRow>): SnapshotRow => makeRow({ balanceMinor: 0, ...overrides })

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
  ;(listCurrencies as Mock).mockResolvedValue([EUR, JPY])
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

  describe('review', () => {
    it('counts rows dropped for an unparseable date or amount', async () => {
      const { result } = await renderAtReview('Date,Amount\n2026-03-01,-10.00\n31.02.2026,-5.00\n2026-03-02,n/a')

      expect(result.current.wizardState.importRows).toHaveLength(1)
      expect(result.current.wizardState.unparseableRowCount).toBe(2)
    })
  })

  describe('foreign currency', () => {
    it('applies a sign rule to the original amount as well as the amount', async () => {
      const wizard = renderWizard()
      await act(() => wizard.result.current.handleFileSelect(csvFile('Date,Amount,Currency,Typ\n2026-03-01,10.00,JPY,D')))
      await act(() => wizard.result.current.handleAccountSelect(1))
      act(() => wizard.result.current.goToMapping())
      act(() => wizard.result.current.setRules([{ type: 'sign_from_column', sortOrder: 0, typeColumn: 'Typ', negativeType: 'D' }]))
      act(() => wizard.result.current.goToReview())

      expect(wizard.result.current.wizardState.importRows[0]).toMatchObject({ amountMinor: -1000, originalAmountMinor: -10, originalCurrencyCode: 'JPY' })
    })

    it('pro-rates the original amount across split legs with the remainder on the last leg', async () => {
      const { result } = await renderAtReview('Date,Amount,Currency\n2026-03-01,-10.00,JPY')
      act(() => result.current.handleSplitOpen(0))
      act(() => result.current.handleLegAmountChange(0, 0, -333))
      act(() => result.current.handleAddLeg(0))
      act(() => result.current.handleLegAmountChange(0, 1, -333))
      act(() => result.current.handleAddLeg(0))

      await act(() => result.current.handleImport())

      const legs = (createSplitGroup as Mock).mock.calls[0][0].legs
      expect(legs.map((l: { amountMinor: number; originalAmountMinor: number }) => [l.amountMinor, l.originalAmountMinor])).toEqual([
        [-333, -3],
        [-333, -3],
        [-334, -4],
      ])
    })
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

  describe('profiles', () => {
    const PROFILE: ImportProfileRow = {
      id: 7,
      name: 'Bank',
      columnMappingJson: JSON.stringify({ date: 'Datum', amount: 'Suma', partner: null, note: 'Popis', currency: null, fxRate: null }),
      rules: [{ id: 1, profileId: 7, ruleType: 'sign_from_column', sortOrder: 0, paramsJson: JSON.stringify({ typeColumn: 'Typ', negativeType: 'D' }) }],
      createdAt: '2026-01-01T00:00:00',
      updatedAt: '2026-01-01T00:00:00',
    }
    const CSV = 'Datum,Suma,Popis,Typ\n01.03.2026,10.00,Coffee,D'

    it('resolves a preferred profile picked before the file against the file headers', async () => {
      ;(getPreferredProfile as Mock).mockResolvedValue(PROFILE)
      const { result } = renderWizard()

      await act(() => result.current.handleAccountSelect(1))
      await act(() => result.current.handleFileSelect(csvFile(CSV)))
      act(() => result.current.goToMapping())

      expect(result.current.wizardState.columnMapping).toMatchObject({ date: 'Datum', amount: 'Suma', note: 'Popis' })
      expect(result.current.wizardState.rules).toEqual([{ type: 'sign_from_column', sortOrder: 0, typeColumn: 'Typ', negativeType: 'D' }])
      expect(result.current.wizardState.originalProfileColumnMapping).toEqual(result.current.wizardState.columnMapping)
    })

    it('resolves a profile chosen from the list before the file against the file headers', async () => {
      ;(listImportProfiles as Mock).mockResolvedValue([PROFILE])
      const { result } = renderWizard()
      await waitFor(() => expect(result.current.availableProfiles).toHaveLength(1))

      act(() => result.current.handleProfileSelect(PROFILE.id))
      await act(() => result.current.handleFileSelect(csvFile(CSV)))

      expect(result.current.wizardState.columnMapping).toMatchObject({ date: 'Datum', amount: 'Suma', note: 'Popis' })
    })
  })

  describe('account switch', () => {
    const SAVINGS = makeAccount({ accountId: 2, accountName: 'Savings' })

    it("drops the previous account's profile when the new account has none", async () => {
      ;(getPreferredProfile as Mock).mockImplementation(async (accountId: number) =>
        accountId === 1
          ? {
              id: 7,
              name: 'Bank',
              columnMappingJson: JSON.stringify({ date: 'Date', amount: 'Amount', partner: null, note: null, currency: null, fxRate: null }),
              rules: [],
              createdAt: '',
              updatedAt: '',
            }
          : null,
      )
      const { result } = renderWizard([...SNAPSHOT, SAVINGS])
      await act(() => result.current.handleFileSelect(csvFile('Date,Amount\n2026-03-01,-10.00')))

      await act(() => result.current.handleAccountSelect(1))
      expect(result.current.wizardState.loadedProfile?.id).toBe(7)
      await act(() => result.current.handleAccountSelect(2))

      expect(result.current.wizardState.loadedProfile).toBeNull()
      expect(result.current.wizardState.originalProfileColumnMapping).toBeNull()
    })

    it('ignores events that arrive late for an account no longer selected', async () => {
      let resolveFirst: (value: { events: EventWithData[] }) => void = () => {}
      ;(listEvents as Mock).mockImplementation(({ accountId }: { accountId: number }) =>
        accountId === 1 ? new Promise((resolve) => (resolveFirst = resolve)) : Promise.resolve({ events: [] }),
      )
      const { result } = renderWizard([...SNAPSHOT, SAVINGS])
      await act(() => result.current.handleFileSelect(csvFile('Date,Amount\n2026-03-01,-10.00')))

      let firstSelect: Promise<void> = Promise.resolve()
      act(() => {
        firstSelect = result.current.handleAccountSelect(1)
      })
      await act(() => result.current.handleAccountSelect(2))
      await act(async () => {
        resolveFirst({ events: [makeEvent({ accountId: 1, eventDate: '2026-03-01T12:00:00', amountMinor: -1000 })] })
        await firstSelect
      })
      act(() => result.current.goToMapping())
      act(() => result.current.goToReview())

      expect(result.current.wizardState.importRows[0].isDuplicate).toBe(false)
    })
  })

  describe('create partner', () => {
    it('reports failure and shows an error when the account currency is unknown', async () => {
      ;(listCurrencies as Mock).mockResolvedValue([])
      const { result } = await renderAtReview('Date,Amount\n2026-03-01,-10.00')

      let created = true
      await act(async () => {
        created = await result.current.handleCreatePartner('SK3112000000198742637541', 'Shop')
      })

      expect(created).toBe(false)
      expect(toast.error).toHaveBeenCalledWith('import.reviewStep.partnerCurrencyMissing')
      expect(createPartnerAccount).not.toHaveBeenCalled()
    })
  })

  describe('assign IBAN', () => {
    it('keeps the target account person and depreciation fields', async () => {
      const car = makeAccount({ accountId: 2, accountName: 'Car', personId: 5, purchasePriceMinor: 2000000, purchaseDate: '2025-01-01', depreciationPeriodMonths: 60 })
      ;(updateAccount as Mock).mockResolvedValue(undefined)
      const { result } = renderWizard([...SNAPSHOT, car])

      await act(() => result.current.handleAssignIban('sk31 1200 0000 1987 4263 7541', 2))

      expect(updateAccount).toHaveBeenCalledWith(2, 'Car', 'SK3112000000198742637541', 5, 2000000, '2025-01-01', 60)
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
