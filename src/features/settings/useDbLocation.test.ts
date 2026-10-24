import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
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
vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
    warning: (...args: unknown[]) => mockToastWarning(...args),
  },
}))

vi.mock('../../shared/api', () => ({
  getDbLocation: vi.fn(),
  pickDbFolder: vi.fn(),
  changeDbLocation: vi.fn(),
  resetDbLocation: vi.fn(),
  checkDefaultDb: vi.fn(),
}))

import { getDbLocation, changeDbLocation } from '../../shared/api'

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
    ;(getDbLocation as Mock).mockResolvedValue(location())
    ;(changeDbLocation as Mock).mockResolvedValue(undefined)
  })

  it('shows the new path in every instance after a switch', async () => {
    const client = createClient()
    const wrapper = wrapperFor(client)
    const options = { setModalState: vi.fn(), closeModal: vi.fn(), onAfterDbChange: async () => client.clear() }
    const appShell = renderHook(() => useDbLocation(options), { wrapper })
    const settingsPage = renderHook(() => useDbLocation(options), { wrapper })

    await waitFor(() => expect(settingsPage.result.current.path).toBe('/default/saldo.db'))
    ;(getDbLocation as Mock).mockResolvedValue(location({ currentPath: '/custom/saldo.db', isDefault: false }))

    await act(async () => {
      await appShell.result.current.handleConfirmSwitch('/custom')
    })
    // closeModal re-renders Settings in the app, which is what re-attaches it to the refreshed query
    settingsPage.rerender()

    await waitFor(() => {
      expect(settingsPage.result.current.path).toBe('/custom/saldo.db')
      expect(settingsPage.result.current.isDefault).toBe(false)
    })
    expect(appShell.result.current.path).toBe('/custom/saldo.db')
  })

  it('does not toast the fallback warning from useDbLocation, however many instances mount', async () => {
    ;(getDbLocation as Mock).mockResolvedValue(location({ fallbackWarning: true }))
    const client = createClient()
    const options = { setModalState: vi.fn(), closeModal: vi.fn(), onAfterDbChange: async () => {} }

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
