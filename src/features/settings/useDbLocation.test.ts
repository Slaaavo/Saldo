import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest'
import React from 'react'
import { renderHook, waitFor, act } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useDbLocation, useDbFallbackWarning } from './useDbLocation'
import type { DbLocationInfo } from '../../shared/types'

vi.mock('react-i18next', () => {
  const t = (key: string) => key
  return { useTranslation: () => ({ t }) }
})

const mockToastWarning = vi.fn()
const mockToastError = vi.fn()
vi.mock('sonner', () => ({
  toast: {
    error: (...args: unknown[]) => mockToastError(...args),
    warning: (...args: unknown[]) => mockToastWarning(...args),
  },
}))

vi.mock('../../shared/api', () => ({
  getDbLocation: vi.fn(),
  getAppSetting: vi.fn(),
  pickDbFolder: vi.fn(),
  changeDbLocation: vi.fn(),
  resetDbLocation: vi.fn(),
  checkDefaultDb: vi.fn(),
}))

import { getDbLocation, getAppSetting, changeDbLocation } from '../../shared/api'

const reload = vi.fn()

const location = (overrides?: Partial<DbLocationInfo>): DbLocationInfo => ({
  currentPath: '/default/saldo.db',
  isDefault: true,
  isDemoMode: false,
  fallbackWarning: false,
  ...overrides,
})

const createClient = () => new QueryClient({ defaultOptions: { queries: { retry: 0 } } })

const wrapperFor = (client: QueryClient) => {
  const Wrapper = ({ children }: { children: React.ReactNode }) => React.createElement(QueryClientProvider, { client }, children)
  return Wrapper
}

describe('useDbLocation', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    ;(getDbLocation as Mock).mockResolvedValue(location())
    ;(changeDbLocation as Mock).mockResolvedValue(undefined)
    vi.stubGlobal('location', { ...window.location, reload })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('caches the new database theme and reloads the window after a switch', async () => {
    ;(getAppSetting as Mock).mockResolvedValue('dark')
    const { result } = renderHook(() => useDbLocation({ setModalState: vi.fn() }), { wrapper: wrapperFor(createClient()) })

    await act(async () => {
      await result.current.handleConfirmSwitch('/custom')
    })

    expect(changeDbLocation).toHaveBeenCalledWith('/custom', 'switch')
    expect(localStorage.getItem('theme')).toBe('dark')
    expect(reload).toHaveBeenCalledTimes(1)
  })

  it('stays on the page and reports the error when the switch fails', async () => {
    ;(changeDbLocation as Mock).mockRejectedValue({ code: 'IO_ERROR', message: 'disk full' })
    const { result } = renderHook(() => useDbLocation({ setModalState: vi.fn() }), { wrapper: wrapperFor(createClient()) })

    await act(async () => {
      await result.current.handleConfirmSwitch('/custom')
    })

    expect(mockToastError).toHaveBeenCalledWith('dataStorage.errors.change')
    expect(reload).not.toHaveBeenCalled()
  })

  it('does not toast the fallback warning from useDbLocation, however many instances mount', async () => {
    ;(getDbLocation as Mock).mockResolvedValue(location({ fallbackWarning: true }))
    const client = createClient()
    const options = { setModalState: vi.fn() }

    const first = renderHook(() => useDbLocation(options), { wrapper: wrapperFor(client) })
    await waitFor(() => expect(first.result.current.path).toBe('/default/saldo.db'))
    first.unmount()
    const second = renderHook(() => useDbLocation(options), { wrapper: wrapperFor(client) })
    await waitFor(() => expect(second.result.current.path).toBe('/default/saldo.db'))

    expect(mockToastWarning).not.toHaveBeenCalled()
  })
})

describe('useDbFallbackWarning', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('toasts once while mounted, even when the location is fetched again', async () => {
    ;(getDbLocation as Mock).mockResolvedValue(location({ fallbackWarning: true }))
    const client = createClient()
    renderHook(() => useDbFallbackWarning(), { wrapper: wrapperFor(client) })

    await waitFor(() => expect(mockToastWarning).toHaveBeenCalledWith('dataStorage.toasts.fallbackWarning'))
    await act(async () => {
      await client.refetchQueries({ queryKey: ['db-location'] })
    })

    expect(mockToastWarning).toHaveBeenCalledTimes(1)
  })

  it('stays silent when there is no fallback warning', async () => {
    ;(getDbLocation as Mock).mockResolvedValue(location())
    renderHook(() => useDbFallbackWarning(), { wrapper: wrapperFor(createClient()) })

    await waitFor(() => expect(getDbLocation).toHaveBeenCalled())
    expect(mockToastWarning).not.toHaveBeenCalled()
  })
})
