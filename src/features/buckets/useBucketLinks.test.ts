import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { useBucketLinks } from './useBucketLinks'

vi.mock('../../shared/api', () => ({
  listLinksForEvent: vi.fn(),
  getLatestBucketLinks: vi.fn(),
}))

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))

import { toast } from 'sonner'
import { listLinksForEvent, getLatestBucketLinks } from '../../shared/api'

describe('useBucketLinks', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('returns the loaded link account ids', async () => {
    ;(listLinksForEvent as Mock).mockResolvedValue([{ sourceAccountId: 7 }])
    const { result } = renderHook(() => useBucketLinks({ isBucket: true, eventId: 1, allAccounts: [] }))
    await waitFor(() => expect(result.current.loadingLinks).toBe(false))

    expect(result.current.getLinkedAccountIds()).toEqual([7])
  })

  it('toasts a failed edit-mode load and refuses to hand back an empty link set', async () => {
    ;(listLinksForEvent as Mock).mockRejectedValue({ code: 'DB', message: 'boom' })
    const { result } = renderHook(() => useBucketLinks({ isBucket: true, eventId: 1, allAccounts: [] }))
    await waitFor(() => expect(result.current.loadingLinks).toBe(false))

    expect(toast.error).toHaveBeenCalledWith('boom')
    expect(() => result.current.getLinkedAccountIds()).toThrow('boom')
  })

  it('toasts a failed create-mode pre-load and refuses to hand back an empty link set', async () => {
    ;(getLatestBucketLinks as Mock).mockRejectedValue(new Error('offline'))
    const { result } = renderHook(() => useBucketLinks({ isBucket: true, eventId: null, bucketAccountId: 3, asOfDate: '2026-01-01', allAccounts: [] }))

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('offline'))
    await waitFor(() => expect(() => result.current.getLinkedAccountIds()).toThrow('offline'))
  })
})
