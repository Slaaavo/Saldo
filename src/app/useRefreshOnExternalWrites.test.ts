import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest'
import React from 'react'
import { renderHook, waitFor, act } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useRefreshOnExternalWrites } from './useRefreshOnExternalWrites'

vi.mock('react-i18next', () => {
  const t = (key: string) => key
  return { useTranslation: () => ({ t }) }
})

const mockToastInfo = vi.fn()
vi.mock('sonner', () => ({
  toast: { info: (...args: unknown[]) => mockToastInfo(...args) },
}))

vi.mock('../shared/api', () => ({ getDbDataVersion: vi.fn() }))

type FocusListener = (event: { payload: boolean }) => void
let focusListener: FocusListener | undefined
const mockUnlisten = vi.fn()
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onFocusChanged: async (listener: FocusListener) => {
      focusListener = listener
      return mockUnlisten
    },
  }),
}))

import { getDbDataVersion } from '../shared/api'

const versions = (...values: number[]) => {
  const mock = getDbDataVersion as Mock
  for (const value of values) mock.mockResolvedValueOnce(value)
}

const mount = async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const invalidate = vi.spyOn(client, 'invalidateQueries')
  const wrapper = ({ children }: { children: React.ReactNode }) => React.createElement(QueryClientProvider, { client }, children)
  const hook = renderHook(() => useRefreshOnExternalWrites(), { wrapper })
  await waitFor(() => expect(focusListener).toBeDefined())
  await waitFor(() => expect(getDbDataVersion).toHaveBeenCalledTimes(1))
  return { invalidate, unmount: hook.unmount }
}

const focus = async (focused = true) => {
  await act(async () => focusListener?.({ payload: focused }))
}

const becomeVisible = async () => {
  await act(async () => {
    document.dispatchEvent(new Event('visibilitychange'))
  })
}

describe('useRefreshOnExternalWrites', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    focusListener = undefined
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('takes the first version as the baseline without refreshing', async () => {
    versions(4)

    const { invalidate } = await mount()

    expect(invalidate).not.toHaveBeenCalled()
    expect(mockToastInfo).not.toHaveBeenCalled()
  })

  it('leaves the cache alone when nothing else wrote', async () => {
    versions(4, 4)
    const { invalidate } = await mount()

    await focus()

    expect(getDbDataVersion).toHaveBeenCalledTimes(2)
    expect(invalidate).not.toHaveBeenCalled()
  })

  it('refreshes every query and tells the user when another process wrote', async () => {
    versions(4, 5)
    const { invalidate } = await mount()

    await focus()

    await waitFor(() => expect(invalidate).toHaveBeenCalledTimes(1))
    expect(invalidate).toHaveBeenCalledWith()
    expect(mockToastInfo).toHaveBeenCalledWith('dataStorage.toasts.externalChanges')
  })

  it('compares later checks with the newest version it saw', async () => {
    versions(4, 5, 5)
    const { invalidate } = await mount()

    await focus()
    await focus()

    await waitFor(() => expect(getDbDataVersion).toHaveBeenCalledTimes(3))
    expect(invalidate).toHaveBeenCalledTimes(1)
  })

  it('checks when the page becomes visible again', async () => {
    versions(4, 5)
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible')
    const { invalidate } = await mount()

    await becomeVisible()

    await waitFor(() => expect(invalidate).toHaveBeenCalledTimes(1))
  })

  it('ignores the window losing focus', async () => {
    versions(4)
    await mount()

    await focus(false)

    expect(getDbDataVersion).toHaveBeenCalledTimes(1)
  })

  it('runs one check at a time', async () => {
    versions(4)
    const { invalidate } = await mount()
    let finish!: (value: number) => void
    ;(getDbDataVersion as Mock).mockReturnValueOnce(new Promise<number>((resolve) => (finish = resolve)))

    await focus()
    await focus()
    await act(async () => finish(5))

    expect(getDbDataVersion).toHaveBeenCalledTimes(2)
    await waitFor(() => expect(invalidate).toHaveBeenCalledTimes(1))
  })

  it('stays quiet when a check fails and checks again on the next focus', async () => {
    versions(4)
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    const { invalidate } = await mount()
    ;(getDbDataVersion as Mock).mockRejectedValueOnce(new Error('locked'))

    await focus()
    expect(invalidate).not.toHaveBeenCalled()
    expect(mockToastInfo).not.toHaveBeenCalled()

    versions(6)
    await focus()
    await waitFor(() => expect(invalidate).toHaveBeenCalledTimes(1))
  })

  it('stops listening when unmounted', async () => {
    versions(4)
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible')
    const { unmount } = await mount()

    unmount()
    await becomeVisible()

    expect(mockUnlisten).toHaveBeenCalledTimes(1)
    expect(getDbDataVersion).toHaveBeenCalledTimes(1)
  })
})
