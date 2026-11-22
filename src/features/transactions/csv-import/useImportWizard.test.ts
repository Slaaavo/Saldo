import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import type {
  CancelledHold,
  CsvImportPreview,
  Currency,
  ImportProfileRow,
  ImportRule,
  ImportVerdict,
  IncomingRowInput,
  PreviewCsvImportInput,
  SnapshotRow,
} from '../../../shared/types'
import { toast } from 'sonner'
import { useImportWizard } from './useImportWizard'
import { makeIncomeBudgetRow, makeRow } from '../../../shared/testing/snapshotFixtures'

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
  toast: { success: vi.fn(), error: vi.fn(), warning: vi.fn(), info: vi.fn() },
}))

vi.mock('../../../shared/api', () => ({
  previewCsvImport: vi.fn(),
  applyCsvImport: vi.fn(),
  listPartnerAccounts: vi.fn(),
  listCurrencies: vi.fn(),
  createPartnerAccount: vi.fn(),
  updateAccount: vi.fn(),
  listImportProfiles: vi.fn(),
  getPreferredProfile: vi.fn(),
  setPreferredProfile: vi.fn(),
  createImportProfile: vi.fn(),
  updateImportProfile: vi.fn(),
}))

import {
  previewCsvImport,
  applyCsvImport,
  listPartnerAccounts,
  listCurrencies,
  createPartnerAccount,
  updateAccount,
  listImportProfiles,
  getPreferredProfile,
  setPreferredProfile,
} from '../../../shared/api'

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }
const JPY: Currency = { id: 3, code: 'JPY', name: 'Japanese Yen', minorUnits: 0, isCustom: false }

const makeAccount = (overrides: Partial<SnapshotRow>): SnapshotRow => makeRow({ balanceMinor: 0, ...overrides })

const SNAPSHOT: SnapshotRow[] = [makeAccount({ accountId: 1, accountName: 'Checking' })]
const SAVINGS = makeAccount({ accountId: 2, accountName: 'Savings' })

const IN_SALDO: ImportVerdict = { kind: 'inSaldo', eventId: 40, eventDate: '2026-03-02T12:00:00', note: null }
const TRANSFER_LEG: ImportVerdict = { kind: 'transferLeg', eventId: 41, otherAccountName: 'Savings', eventDate: '2026-02-27T12:00:00' }
const COMPLETES_TRANSFER: ImportVerdict = { kind: 'completesTransfer', eventId: 42, accountId: 2, accountName: 'Savings', eventDate: '2026-03-01T12:00:00', note: null }

const answerPreviews = (...answers: Array<{ verdicts: ImportVerdict[]; token?: string; balanceUpdateDays?: string[]; cancelledHolds?: CancelledHold[] }>) => {
  let call = 0
  ;(previewCsvImport as Mock).mockImplementation(async ({ rows }: PreviewCsvImportInput): Promise<CsvImportPreview> => {
    const answer = answers[call++]
    return {
      rows: rows.map((row, i) => ({ index: row.index, verdict: answer?.verdicts[i] ?? { kind: 'new' } })),
      balanceUpdateDays: answer?.balanceUpdateDays ?? [],
      cancelledHolds: answer?.cancelledHolds ?? [],
      reviewToken: answer?.token ?? `token-${call}`,
    }
  })
}

const previewedRows = (call: number) => (previewCsvImport as Mock).mock.calls[call][0].rows

const setupMocks = () => {
  answerPreviews()
  ;(applyCsvImport as Mock).mockResolvedValue({ imported: 1, removedHolds: 0 })
  ;(listPartnerAccounts as Mock).mockResolvedValue([])
  ;(listCurrencies as Mock).mockResolvedValue([EUR, JPY])
  ;(listImportProfiles as Mock).mockResolvedValue([])
  ;(getPreferredProfile as Mock).mockResolvedValue(null)
  ;(setPreferredProfile as Mock).mockResolvedValue(undefined)
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
  await act(() => wizard.result.current.goToReview())
  return wizard
}

const THREE_ROWS = 'Date,Amount\n2026-03-01,-50.00\n2026-03-02,-10.00\n2026-03-03,20.00'

describe('useImportWizard', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    setupMocks()
  })

  describe('buckets', () => {
    it('offers every bucket for tagging a row or a split leg except an income budget', async () => {
      const { result } = renderWizard([...SNAPSHOT, makeAccount({ accountId: 10, accountName: 'Vacation', accountType: 'bucket' }), makeIncomeBudgetRow()])

      await waitFor(() => expect(listImportProfiles).toHaveBeenCalled())
      expect(result.current.availableBuckets.map((b) => b.accountName)).toEqual(['Vacation'])
    })
  })

  describe('review', () => {
    it('counts rows dropped for an unparseable date or amount', async () => {
      const { result } = await renderAtReview('Date,Amount\n2026-03-01,-10.00\n31.02.2026,-5.00\n2026-03-02,n/a')

      expect(result.current.wizardState.importRows).toHaveLength(1)
      expect(result.current.wizardState.unparseableRowCount).toBe(2)
    })

    it('sends the rows to the preview as the rules leave them', async () => {
      const wizard = renderWizard()
      await act(() => wizard.result.current.handleFileSelect(csvFile('Date,Amount,Note,Currency,Typ\n2026-03-01,10.00,Coffee,,D\n2026-03-02,10.00,,JPY,D\n2026-03-03,5.00,,,K')))
      await act(() => wizard.result.current.handleAccountSelect(1))
      act(() => wizard.result.current.goToMapping())
      act(() => wizard.result.current.setRules([{ type: 'sign_from_column', sortOrder: 0, typeColumn: 'Typ', negativeType: 'D' }]))
      await act(() => wizard.result.current.goToReview())

      const none = {
        counterpartAccountId: null,
        namesUnknownAccount: false,
        originalCurrencyId: null,
        originalAmountMinor: null,
        fxRateMantissa: null,
        fxRateExponent: null,
        card: null,
        pending: false,
      }
      expect(previewCsvImport).toHaveBeenCalledWith({
        accountId: 1,
        rows: [
          { index: 0, eventDate: '2026-03-01T12:00:00', amountMinor: -1000, note: 'Coffee', ...none },
          { index: 1, eventDate: '2026-03-02T12:00:00', amountMinor: -1000, note: null, ...none, originalCurrencyId: 3, originalAmountMinor: -10 },
          { index: 2, eventDate: '2026-03-03T12:00:00', amountMinor: 500, note: null, ...none },
        ],
      })
      expect(wizard.result.current.wizardState.step).toBe('review')
    })

    it('tells the preview which rows name an IBAN that matches no account in Saldo', async () => {
      const savings = makeAccount({ accountId: 2, accountName: 'Savings', iban: 'SK0000000000000000000002' })
      await renderAtReview('Date,Amount,IBAN\n2026-03-03,-50.00,DE89370400440532013000\n2026-03-02,-50.00,SK0000000000000000000002\n2026-03-01,-5.00,', [...SNAPSHOT, savings])

      expect(previewedRows(0).map((r: IncomingRowInput) => [r.counterpartAccountId, r.namesUnknownAccount])).toEqual([
        [null, true],
        [2, false],
        [null, false],
      ])
    })

    it('ticks new rows and rows that complete a transfer, and leaves rows already in Saldo unticked', async () => {
      answerPreviews({ verdicts: [{ kind: 'new' }, COMPLETES_TRANSFER, IN_SALDO, TRANSFER_LEG], balanceUpdateDays: ['2026-03-02'] })
      const { result } = await renderAtReview(`${THREE_ROWS}\n2026-03-04,30.00`)

      expect(result.current.wizardState.importRows.map((r) => [r.verdict.kind, r.isSelected])).toEqual([
        ['new', true],
        ['completesTransfer', true],
        ['inSaldo', false],
        ['transferLeg', false],
      ])
      expect(result.current.selectedCount).toBe(2)
      expect(result.current.balanceWarningDates).toEqual(['2026-03-02'])
    })

    it('selects and deselects only the new rows', async () => {
      answerPreviews({ verdicts: [{ kind: 'new' }, IN_SALDO, TRANSFER_LEG] })
      const { result } = await renderAtReview(THREE_ROWS)
      act(() => result.current.handleToggleRow(1))

      act(() => result.current.handleDeselectAll())
      expect(result.current.wizardState.importRows.map((r) => r.isSelected)).toEqual([false, true, false])

      act(() => result.current.handleSelectAll())
      expect(result.current.wizardState.importRows.map((r) => r.isSelected)).toEqual([true, true, false])
    })

    it('stays before the review when the preview fails', async () => {
      ;(previewCsvImport as Mock).mockRejectedValue({ code: 'DB_ERROR', message: 'locked' })
      const { result } = await renderAtReview(THREE_ROWS)

      expect(result.current.wizardState.step).toBe('mapping')
      expect(toast.error).toHaveBeenCalledWith('errors.loadData')
    })
  })

  describe('card holds', () => {
    const CARD_EXPORT = [
      'Dátum transakcie,Suma EUR,Názov obchodníka,Číslo karty,Stav transakcie',
      '14.09.2026,-78.96,Tchibo,433703******2103,Autorizovaná',
      '20.09.2026,-0.50,ZSSK,433703******2103,Zamietnutá',
      '15.09.2026,-0.50,ZSSK,433703******9999,Zúčtovaná',
    ].join('\n')
    const STATE_RULES: ImportRule[] = [
      { type: 'pending_from_column', sortOrder: 0, column: 'Stav transakcie', pattern: '^Autorizovaná$' },
      { type: 'declined_from_column', sortOrder: 1, column: 'Stav transakcie', pattern: '^Zamietnutá$' },
    ]
    const HOLD_POSTED: ImportVerdict = { kind: 'holdPosted', eventId: 60, eventDate: '2026-09-14T12:00:00', note: 'Tchibo' }
    const HOLD_AT_OTHER_AMOUNT: ImportVerdict = { kind: 'holdPostedAtOtherAmount', eventId: 61, eventDate: '2026-04-30T12:00:00', note: 'GitHub', holdAmountMinor: -3300 }
    const cancelledHold = (eventId: number): CancelledHold => ({ eventId, eventDate: '2026-09-10T12:00:00', amountMinor: -1250, note: 'Shop' })

    const renderCardExportAtReview = async () => {
      const wizard = renderWizard()
      await act(() => wizard.result.current.handleFileSelect(csvFile(CARD_EXPORT)))
      await act(() => wizard.result.current.handleAccountSelect(1))
      act(() => wizard.result.current.goToMapping())
      act(() => wizard.result.current.setRules(STATE_RULES))
      await act(() => wizard.result.current.goToReview())
      return wizard
    }

    it('sends each row with its card and whether it is pending, and skips declined rows before the preview', async () => {
      const { result } = await renderCardExportAtReview()

      expect(previewedRows(0).map((r: IncomingRowInput) => [r.index, r.eventDate, r.card, r.pending])).toEqual([
        [0, '2026-09-14T12:00:00', '433703******2103', true],
        [2, '2026-09-15T12:00:00', '433703******9999', false],
      ])
      expect(result.current.wizardState.declinedRowCount).toBe(1)
      expect(result.current.wizardState.unparseableRowCount).toBe(0)
      expect(result.current.wizardState.importRows.map((r) => r.index)).toEqual([0, 2])
    })

    it('leaves a hold posted at the same amount unticked and ticks a hold posted at another amount', async () => {
      answerPreviews({ verdicts: [HOLD_POSTED, HOLD_AT_OTHER_AMOUNT] })
      const { result } = await renderCardExportAtReview()

      expect(result.current.wizardState.importRows.map((r) => [r.verdict.kind, r.isSelected])).toEqual([
        ['holdPosted', false],
        ['holdPostedAtOtherAmount', true],
      ])
    })

    it('sends one removal choice per cancelled hold, keeping a choice across a new preview and ticking a hold new to it', async () => {
      answerPreviews(
        { verdicts: [], cancelledHolds: [cancelledHold(70), cancelledHold(71)], token: 'old' },
        { verdicts: [], cancelledHolds: [cancelledHold(71), cancelledHold(72)], token: 'new' },
      )
      ;(applyCsvImport as Mock).mockRejectedValueOnce({ code: 'IMPORT_CHANGED', message: 'Saldo changed' })
      const { result } = await renderCardExportAtReview()
      expect(result.current.wizardState.cancelledHolds.map((h) => [h.eventId, h.remove])).toEqual([
        [70, true],
        [71, true],
      ])

      act(() => result.current.handleToggleCancelledHold(71))
      await act(() => result.current.handleImport())

      expect((applyCsvImport as Mock).mock.calls[0][0]).toMatchObject({
        reviewToken: 'old',
        cancelledHolds: [
          { eventId: 70, remove: true },
          { eventId: 71, remove: false },
        ],
      })

      await act(() => result.current.handleImport())

      expect((applyCsvImport as Mock).mock.calls[1][0]).toMatchObject({
        reviewToken: 'new',
        cancelledHolds: [
          { eventId: 71, remove: false },
          { eventId: 72, remove: true },
        ],
      })
    })

    it('says what an import that only removed cancelled holds did instead of nothing new', async () => {
      const profile: ImportProfileRow = {
        id: 8,
        name: 'Card',
        columnMappingJson: JSON.stringify({ date: 'Dátum transakcie', amount: 'Suma EUR', partner: null, note: null, currency: null, fxRate: null, card: null }),
        rules: [],
        createdAt: '',
        updatedAt: '',
      }
      ;(getPreferredProfile as Mock).mockResolvedValue(profile)
      ;(applyCsvImport as Mock).mockResolvedValue({ imported: 0, removedHolds: 2 })
      answerPreviews({ verdicts: [IN_SALDO, IN_SALDO, IN_SALDO], cancelledHolds: [cancelledHold(70), cancelledHold(71)] })
      const { result, onClose } = await renderAtReview(CARD_EXPORT)

      await act(() => result.current.handleImport())

      expect(toast.info).not.toHaveBeenCalled()
      expect(toast.success).toHaveBeenCalledWith('import.removedHolds')
      expect(onClose).toHaveBeenCalled()
    })

    it('shows the removed holds on the save-profile step', async () => {
      ;(applyCsvImport as Mock).mockResolvedValue({ imported: 1, removedHolds: 2 })
      const { result } = await renderAtReview('Date,Amount\n2026-03-01,-10.00')

      await act(() => result.current.handleImport())

      expect(result.current.wizardState).toMatchObject({ step: 'save-profile', importedCount: 1, removedHoldCount: 2 })
    })
  })

  describe('counterpart changes', () => {
    it("previews again after a counterpart change, keeping a row's choice while its verdict kind stays and resetting it when the kind changes", async () => {
      answerPreviews({ verdicts: [{ kind: 'new' }, { kind: 'new' }] }, { verdicts: [{ kind: 'new' }, COMPLETES_TRANSFER], token: 'token-after' })
      const { result } = await renderAtReview('Date,Amount\n2026-03-01,-50.00\n2026-03-02,-10.00', [...SNAPSHOT, SAVINGS])
      act(() => result.current.handleBucketChange(0, 10))
      act(() => result.current.handleToggleRow(0))
      act(() => result.current.handleBucketChange(1, 10))

      act(() => result.current.handleCounterpartChange(1, 2))

      await waitFor(() => expect(result.current.wizardState.reviewToken).toBe('token-after'))
      expect(previewedRows(1).map((r: { counterpartAccountId: number | null }) => r.counterpartAccountId)).toEqual([null, 2])
      expect(result.current.wizardState.importRows.map((r) => [r.verdict.kind, r.isSelected, r.bucketId])).toEqual([
        ['new', false, 10],
        ['completesTransfer', true, null],
      ])
    })

    it('previews again with a partner created for an unmatched IBAN', async () => {
      ;(createPartnerAccount as Mock).mockResolvedValue(50)
      const { result } = await renderAtReview('Date,Amount,IBAN\n2026-03-01,-10.00,SK3112000000198742637541')

      await act(async () => {
        await result.current.handleCreatePartner('SK31 1200 0000 1987 4263 7541', 'Shop')
      })

      await waitFor(() => expect(result.current.wizardState.reviewToken).toBe('token-2'))
      expect(previewedRows(1)[0].counterpartAccountId).toBe(50)
      expect(result.current.wizardState.importRows[0].ibanMatch).toEqual({ type: 'partner', accountId: 50, accountName: 'Shop' })
    })
  })

  describe('import', () => {
    it("applies once with every row's choice and the token of the reviewed preview", async () => {
      answerPreviews({ verdicts: [{ kind: 'new' }, IN_SALDO, TRANSFER_LEG], token: 'reviewed' })
      const { result } = await renderAtReview(THREE_ROWS)
      act(() => result.current.handleSplitOpen(0))
      act(() => result.current.handleLegAmountChange(0, 0, -2000))
      act(() => result.current.handleAddLeg(0))
      act(() => result.current.handleToggleRow(1))
      act(() => result.current.handleBucketChange(1, 10))

      await act(() => result.current.handleImport())

      expect(applyCsvImport).toHaveBeenCalledTimes(1)
      expect(applyCsvImport).toHaveBeenCalledWith({
        accountId: 1,
        rows: previewedRows(0),
        choices: [
          {
            index: 0,
            selected: true,
            bucketId: null,
            split: [
              { amountMinor: -2000, note: null, counterpartAccountId: null, bucketId: null },
              { amountMinor: -3000, note: null, counterpartAccountId: null, bucketId: null },
            ],
          },
          { index: 1, selected: true, bucketId: 10, split: null },
          { index: 2, selected: false, bucketId: null, split: null },
        ],
        cancelledHolds: [],
        reviewToken: 'reviewed',
      })
    })

    it('finishes an import with nothing ticked and says nothing new was imported', async () => {
      const PROFILE: ImportProfileRow = {
        id: 7,
        name: 'Bank',
        columnMappingJson: JSON.stringify({ date: 'Date', amount: 'Amount', partner: null, note: null, currency: null, fxRate: null }),
        rules: [],
        createdAt: '',
        updatedAt: '',
      }
      ;(getPreferredProfile as Mock).mockResolvedValue(PROFILE)
      ;(applyCsvImport as Mock).mockResolvedValue({ imported: 0, removedHolds: 0 })
      answerPreviews({ verdicts: [IN_SALDO, TRANSFER_LEG] })
      const { result, onClose, onSuccess } = await renderAtReview('Date,Amount\n2026-03-02,-10.00\n2026-03-01,20.00')
      expect(result.current.selectedCount).toBe(0)
      expect(result.current.canApplyReview).toBe(true)

      await act(() => result.current.handleImport())

      expect((applyCsvImport as Mock).mock.calls[0][0].choices.map((c: { selected: boolean }) => c.selected)).toEqual([false, false])
      expect(onSuccess).toHaveBeenCalledTimes(1)
      expect(toast.info).toHaveBeenCalledWith('import.nothingNew')
      expect(toast.success).not.toHaveBeenCalled()
      expect(onClose).toHaveBeenCalled()
    })

    it('previews again and stays on the review when Saldo changed during the review', async () => {
      answerPreviews({ verdicts: [{ kind: 'new' }], token: 'old' }, { verdicts: [IN_SALDO], token: 'new' })
      ;(applyCsvImport as Mock).mockRejectedValueOnce({ code: 'IMPORT_CHANGED', message: 'Saldo changed' })
      const { result, onSuccess } = await renderAtReview('Date,Amount\n2026-03-02,-10.00')

      await act(() => result.current.handleImport())

      expect(toast.warning).toHaveBeenCalledWith('import.reviewStep.changedDuringReview')
      expect(onSuccess).not.toHaveBeenCalled()
      expect(result.current.wizardState.step).toBe('review')
      expect(result.current.wizardState.importRows[0]).toMatchObject({ verdict: IN_SALDO, isSelected: false })

      await act(() => result.current.handleImport())

      expect((applyCsvImport as Mock).mock.calls[1][0]).toMatchObject({ reviewToken: 'new', choices: [{ index: 0, selected: false }] })
    })

    it('reports a split that has only one leg', async () => {
      const { result } = await renderAtReview('Date,Amount\n2026-03-01,-50.00')

      act(() => result.current.handleSplitOpen(0))

      expect(result.current.splitValidationErrors).toEqual(['import.reviewStep.split.minLegsError'])
      expect(result.current.canApplyReview).toBe(false)
    })

    it('refreshes data before switching to the save-profile step', async () => {
      const { result, onSuccess, onClose } = await renderAtReview('Date,Amount\n2026-03-01,-10.00')

      await act(() => result.current.handleImport())

      expect(onSuccess).toHaveBeenCalledTimes(1)
      expect(result.current.wizardState).toMatchObject({ step: 'save-profile', importedCount: 1 })
      expect(onClose).not.toHaveBeenCalled()
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

    it('loads a profile saved before the card field existed and matches its hold rules to the file headers', async () => {
      const savedBeforeCards: ImportProfileRow = {
        id: 9,
        name: 'Tatra card',
        columnMappingJson: JSON.stringify({ date: 'Dátum transakcie', amount: 'Suma EUR', partner: null, note: 'Názov obchodníka', currency: null, fxRate: null }),
        rules: [
          { id: 2, profileId: 9, ruleType: 'pending_from_column', sortOrder: 0, paramsJson: JSON.stringify({ column: 'stav transakcie', pattern: '^Autorizovaná$' }) },
          { id: 3, profileId: 9, ruleType: 'declined_from_column', sortOrder: 1, paramsJson: JSON.stringify({ column: 'Gone', pattern: '^Zamietnutá$' }) },
        ],
        createdAt: '',
        updatedAt: '',
      }
      ;(getPreferredProfile as Mock).mockResolvedValue(savedBeforeCards)
      const { result } = renderWizard()

      await act(() => result.current.handleAccountSelect(1))
      await act(() =>
        result.current.handleFileSelect(csvFile('Dátum transakcie,Suma EUR,Názov obchodníka,Číslo karty,Stav transakcie\n14.09.2026,-1.00,Shop,433703******2103,Autorizovaná')),
      )
      act(() => result.current.goToMapping())

      expect(result.current.wizardState.columnMapping).toEqual({
        date: 'Dátum transakcie',
        amount: 'Suma EUR',
        partner: null,
        note: 'Názov obchodníka',
        currency: null,
        fxRate: null,
        card: null,
      })
      expect(result.current.wizardState.rules).toEqual([
        { type: 'pending_from_column', sortOrder: 0, column: 'Stav transakcie', pattern: '^Autorizovaná$' },
        { type: 'declined_from_column', sortOrder: 1, column: '', pattern: '^Zamietnutá$' },
      ])
    })
  })

  describe('account switch', () => {
    it('picks a preselected account on open and loads its preferred profile once', async () => {
      const PROFILE = { id: 7, name: 'Bank', columnMappingJson: JSON.stringify({ date: 'Date', amount: 'Amount' }), rules: [], createdAt: '', updatedAt: '' }
      ;(getPreferredProfile as Mock).mockResolvedValue(PROFILE)
      const props = { onClose: vi.fn(), onSuccess: vi.fn().mockResolvedValue(undefined) }
      const { result, rerender } = renderHook(({ snapshot }) => useImportWizard({ snapshot, preselectedAccountId: 2, ...props }), {
        initialProps: { snapshot: [...SNAPSHOT, SAVINGS] },
      })

      await waitFor(() => expect(result.current.wizardState.loadedProfile?.id).toBe(7))
      expect(result.current.wizardState.selectedAccountId).toBe(2)

      rerender({ snapshot: [...SNAPSHOT, { ...SAVINGS, balanceMinor: 500 }] })

      expect(getPreferredProfile).toHaveBeenCalledTimes(1)
    })

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
