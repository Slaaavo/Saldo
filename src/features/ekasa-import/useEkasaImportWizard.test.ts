import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useEkasaImportWizard } from './useEkasaImportWizard'
import type { ProcessReceiptResult } from './types'

vi.mock('../../shared/api', () => ({
  getEkasaProfile: vi.fn(),
  upsertEkasaProfile: vi.fn(),
  processReceiptFile: vi.fn(),
}))

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))

import { processReceiptFile, upsertEkasaProfile } from '../../shared/api'

const OFFLINE_RESULT: ProcessReceiptResult = {
  receiptData: null,
  processedItems: [],
  qrContent: 'O-ABC:1:2:3',
  offlineFallback: { eventDate: '2025-04-12T10:30:00', totalAmountMinor: 1250 },
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
  sourceRef: 'online:abc',
  alreadyImported: false,
}

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
