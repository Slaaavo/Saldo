import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useEkasaImportWizard } from './useEkasaImportWizard'
import type { EkasaRuleMatches, MatchEkasaRulesPayload, ProcessReceiptResult } from './types'

vi.mock('../../shared/api', () => ({
  getEkasaProfile: vi.fn(),
  upsertEkasaProfile: vi.fn(),
  processReceiptFile: vi.fn(),
  matchEkasaRules: vi.fn(),
}))

vi.mock('sonner', () => ({ toast: { error: vi.fn(), warning: vi.fn() } }))

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

import { toast } from 'sonner'
import { matchEkasaRules, processReceiptFile, upsertEkasaProfile } from '../../shared/api'

const OFFLINE_RESULT: ProcessReceiptResult = {
  receiptData: null,
  processedItems: [],
  qrContent: 'O-ABC:1:2:3',
  offlineFallback: { eventDate: '2025-04-12T10:30:00', totalAmountMinor: 1250 },
  receiptDate: '2025-04-12',
  sourceRef: 'offline:O-ABC:1:2:3',
  alreadyImported: false,
}

const ONLINE_RESULT: ProcessReceiptResult = {
  receiptData: {
    id: 'r1',
    receiptId: 'r1',
    issueDate: '03.05.2025 14:20:00',
    totalPrice: 5,
    organization: { name: 'Shop', ico: '123' },
    items: [],
    vatSummary: [],
  },
  processedItems: [{ name: 'Milk', amountMinor: 500, vatRateBps: 2000 }],
  qrContent: 'abc',
  offlineFallback: null,
  receiptDate: '2025-05-03',
  sourceRef: 'online:abc',
  alreadyImported: false,
}

const noRuleMatches = (payload: MatchEkasaRulesPayload): EkasaRuleMatches => ({
  items: payload.itemNames.map(() => ({ matchedRule: null })),
  invalidRules: [],
})

const deferred = <T>() => {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

describe('useEkasaImportWizard', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(matchEkasaRules as Mock).mockImplementation(async (payload: MatchEkasaRulesPayload) => noRuleMatches(payload))
  })

  it('goes straight to review with a single total leg for an offline receipt', async () => {
    ;(processReceiptFile as Mock).mockResolvedValue(OFFLINE_RESULT)
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() => result.current.handleFileSelect('/receipt.png'))
    await act(() => result.current.handleRulesConfirm())

    expect(result.current.state.step).toBe('review')
    expect(result.current.state.processedLegs).toHaveLength(1)
    expect(result.current.state.processedLegs[0]).toMatchObject({ amount: '12.50', eventDate: '2025-04-12T10:30:00' })
    expect(result.current.state.receiptDate).toBe('2025-04-12T10:30:00')
    expect(matchEkasaRules).not.toHaveBeenCalled()
  })

  const MULTI_ITEM_RESULT: ProcessReceiptResult = {
    ...ONLINE_RESULT,
    processedItems: [
      { name: 'KÁVA', amountMinor: 1599, vatRateBps: 2300 },
      { name: 'MLIEKO', amountMinor: 129, vatRateBps: 500 },
    ],
  }

  it('takes each item’s shares from the rule Saldo matched, and the defaults when none matched', async () => {
    ;(processReceiptFile as Mock).mockResolvedValue(MULTI_ITEM_RESULT)
    ;(matchEkasaRules as Mock).mockResolvedValue({
      items: [{ matchedRule: { position: 2, pattern: 'kava', deductiblePctBps: 5000, vatReclaimablePctBps: 0 } }, { matchedRule: null }],
      invalidRules: [],
    })
    const { result } = renderHook(() => useEkasaImportWizard())
    act(() => {
      result.current.setRules([
        { id: 'a', namePattern: '', deductiblePct: '100', vatReclaimablePct: '100' },
        { id: 'b', namePattern: 'kava', deductiblePct: '50', vatReclaimablePct: '0' },
      ])
      result.current.setDefaultDeductiblePct('80')
    })

    act(() => result.current.handleFileSelect('/receipt.png'))
    await act(() => result.current.handleRulesConfirm())

    expect(matchEkasaRules).toHaveBeenCalledWith({
      itemNames: ['KÁVA', 'MLIEKO'],
      rules: [
        { namePattern: '', deductiblePctBps: 10000, vatReclaimablePctBps: 10000 },
        { namePattern: 'kava', deductiblePctBps: 5000, vatReclaimablePctBps: 0 },
      ],
    })
    expect(result.current.state.processedLegs.map((leg) => [leg.note, leg.expenseDeductiblePct, leg.vatReclaimablePct])).toEqual([
      ['KÁVA', '50', '0'],
      ['MLIEKO', '80', '100'],
    ])
  })

  it('warns about a rule Saldo could not use', async () => {
    ;(processReceiptFile as Mock).mockResolvedValue(ONLINE_RESULT)
    ;(matchEkasaRules as Mock).mockResolvedValue({
      items: [{ matchedRule: null }],
      invalidRules: [{ position: 1, pattern: 'ka(va', message: 'unclosed group' }],
    })
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() => result.current.handleFileSelect('/receipt.png'))
    await act(() => result.current.handleRulesConfirm())

    expect(toast.warning).toHaveBeenCalledWith('ekasaImport.rulesStep.invalidRule')
    expect(result.current.state.step).toBe('review')
  })

  it('stays on the rules step when Saldo refuses the rules', async () => {
    ;(processReceiptFile as Mock).mockResolvedValue(ONLINE_RESULT)
    ;(matchEkasaRules as Mock).mockRejectedValue({ code: 'VALIDATION', message: 'deductible_pct_bps must be in range [0, 10000] bps' })
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() => result.current.handleFileSelect('/receipt.png'))
    await act(() => result.current.handleRulesConfirm())

    expect(toast.error).toHaveBeenCalledWith('deductible_pct_bps must be in range [0, 10000] bps')
    expect(result.current.state.step).toBe('rules')
  })

  it('reviews the receipt items for an online receipt', async () => {
    ;(processReceiptFile as Mock).mockResolvedValue(ONLINE_RESULT)
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() => result.current.handleFileSelect('/receipt.png'))
    await act(() => result.current.handleRulesConfirm())

    expect(result.current.state.step).toBe('review')
    expect(result.current.state.processedLegs).toHaveLength(1)
    expect(result.current.state.processedLegs[0]).toMatchObject({ amount: '5.00', note: 'Milk', eventDate: '2025-05-03' })
    expect(result.current.state.vendorName).toBe('Shop')
    expect(result.current.state.offlineFallback).toBeNull()
  })

  it.each([
    ['online', ONLINE_RESULT],
    ['offline', OFFLINE_RESULT],
  ])('keeps the reference and the already-imported flag of an %s receipt for the review', async (_, receipt) => {
    ;(processReceiptFile as Mock).mockResolvedValue({ ...receipt, alreadyImported: true })
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() => result.current.handleFileSelect('/receipt.png'))
    await act(() => result.current.handleRulesConfirm())

    expect(result.current.state.step).toBe('review')
    expect(result.current.state.sourceRef).toBe(receipt.sourceRef)
    expect(result.current.state.alreadyImported).toBe(true)
  })

  it('does not flag a receipt that was not imported before', async () => {
    ;(processReceiptFile as Mock).mockResolvedValue(ONLINE_RESULT)
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() => result.current.handleFileSelect('/receipt.png'))
    await act(() => result.current.handleRulesConfirm())

    expect(result.current.state.alreadyImported).toBe(false)
  })

  it('does not reuse an earlier offline receipt when the next file fails', async () => {
    ;(processReceiptFile as Mock).mockResolvedValueOnce(OFFLINE_RESULT).mockRejectedValueOnce({ code: 'EKASA', message: 'Receipt not found' })
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() => result.current.handleFileSelect('/offline.png'))
    await act(() => result.current.handleRulesConfirm())
    expect(result.current.state.offlineFallback).not.toBeNull()

    act(() => result.current.handleBack())
    act(() => result.current.handleBack())
    await act(async () => result.current.handleFileSelect('/broken.png'))

    expect(result.current.state.step).toBe('error')
    expect(result.current.state.offlineFallback).toBeNull()
  })

  it('clears the offline receipt on try again', async () => {
    ;(processReceiptFile as Mock).mockResolvedValue(OFFLINE_RESULT)
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() => result.current.handleFileSelect('/offline.png'))
    await act(() => result.current.handleRulesConfirm())
    act(() => result.current.handleTryAgain())

    expect(result.current.state.step).toBe('upload')
    expect(result.current.state.offlineFallback).toBeNull()
  })

  it('reports the message of a structured backend error', async () => {
    ;(processReceiptFile as Mock).mockRejectedValue({ code: 'EKASA', message: 'Receipt not found' })
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() => result.current.handleFileSelect('/broken.png'))
    await act(() => result.current.handleRulesConfirm())

    expect(result.current.state.step).toBe('error')
    expect(result.current.state.receiptError).toBe('Receipt not found')
  })

  it('ignores a failure from a file that is no longer the current one', async () => {
    const stale = deferred<ProcessReceiptResult>()
    ;(processReceiptFile as Mock).mockReturnValueOnce(stale.promise)
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() => result.current.handleFileSelect('/slow.png'))
    act(() => result.current.handleTryAgain())
    await act(async () => {
      stale.reject({ code: 'EKASA', message: 'late failure' })
      await stale.promise.catch(() => {})
    })

    expect(result.current.state.step).toBe('upload')
    expect(result.current.state.receiptError).toBeNull()
  })

  it('ignores the result of a file that is no longer the current one when confirming rules', async () => {
    const stale = deferred<ProcessReceiptResult>()
    ;(processReceiptFile as Mock).mockReturnValueOnce(stale.promise)
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() => result.current.handleFileSelect('/slow.png'))
    let confirm!: Promise<void>
    act(() => {
      confirm = result.current.handleRulesConfirm()
    })
    act(() => result.current.handleTryAgain())
    await act(async () => {
      stale.resolve(OFFLINE_RESULT)
      await confirm
    })

    expect(result.current.state.step).toBe('upload')
    expect(result.current.state.processedLegs).toEqual([])
  })

  it('does not save rules with a blank name pattern', async () => {
    ;(upsertEkasaProfile as Mock).mockResolvedValue({})
    const { result } = renderHook(() => useEkasaImportWizard())

    act(() =>
      result.current.setRules([
        { id: 'a', namePattern: '', deductiblePct: '100', vatReclaimablePct: '100' },
        { id: 'b', namePattern: 'coffee', deductiblePct: '50', vatReclaimablePct: '0' },
        { id: 'c', namePattern: '  ', deductiblePct: '100', vatReclaimablePct: '100' },
      ]),
    )
    await act(() => result.current.saveProfile(1))

    expect(upsertEkasaProfile).toHaveBeenCalledWith(
      expect.objectContaining({
        rules: [{ sortOrder: 0, namePattern: 'coffee', deductiblePctBps: 5000, vatReclaimablePctBps: 0 }],
      }),
    )
  })
})
