import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest'
import { renderHook, waitFor, act } from '@testing-library/react'
import { createElement } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useSettings } from './useSettings'
import type { Currency } from '../../shared/types'

// ── Mock the API layer ──────────────────────────────────────────────────────
vi.mock('../../shared/api', () => ({
  listCurrencies: vi.fn(),
  getConsolidationCurrency: vi.fn(),
  setConsolidationCurrency: vi.fn(),
  getAppSetting: vi.fn(),
  setAppSetting: vi.fn(),
}))

import { listCurrencies, getConsolidationCurrency, setConsolidationCurrency, getAppSetting, setAppSetting } from '../../shared/api'

const mockToastError = vi.fn()
vi.mock('sonner', () => ({
  toast: { error: (...args: unknown[]) => mockToastError(...args) },
}))

// Stable `t` reference keeps the hook's useCallback deps from changing on every render
vi.mock('react-i18next', () => {
  const t = (key: string, opts?: Record<string, unknown>) => (opts ? `${key}:${JSON.stringify(opts)}` : key)
  return { useTranslation: () => ({ t }) }
})

const makeWrapper = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const Wrapper = ({ children }: { children: Parameters<typeof createElement>[2] }) => createElement(QueryClientProvider, { client: queryClient }, children)
  return Wrapper
}

// ── Test data ───────────────────────────────────────────────────────────────
const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }
const USD: Currency = { id: 2, code: 'USD', name: 'US Dollar', minorUnits: 2, isCustom: false }
const ALL_CURRENCIES = [EUR, USD]

const setupMocks = (overrides?: { currencies?: Currency[]; consolidation?: Currency; apiKey?: string | null }) => {
  ;(listCurrencies as Mock).mockResolvedValue(overrides?.currencies ?? ALL_CURRENCIES)
  ;(getConsolidationCurrency as Mock).mockResolvedValue(overrides?.consolidation ?? EUR)
  ;(getAppSetting as Mock).mockResolvedValue(overrides?.apiKey ?? null)
  ;(setConsolidationCurrency as Mock).mockResolvedValue(undefined)
  ;(setAppSetting as Mock).mockResolvedValue(undefined)
}

describe('useSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.useFakeTimers({ shouldAdvanceTime: true })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  // ── Loading ───────────────────────────────────────────────────────────

  it('loads currencies, consolidation currency, and API key on mount', async () => {
    setupMocks({ apiKey: 'stored-key' })

    const { result } = renderHook(() => useSettings(), { wrapper: makeWrapper() })

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES)
      expect(result.current.selectedCurrency).toEqual(EUR)
      expect(result.current.apiKey).toBe('stored-key')
    })

    expect(listCurrencies).toHaveBeenCalledOnce()
    expect(getConsolidationCurrency).toHaveBeenCalledOnce()
    expect(getAppSetting).toHaveBeenCalledWith('oxr_app_id')
  })

  it('does not set API key when stored value is null', async () => {
    setupMocks({ apiKey: null })

    const { result } = renderHook(() => useSettings(), { wrapper: makeWrapper() })

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES)
    })

    expect(result.current.apiKey).toBe('')
  })

  it('handles load failure gracefully', async () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    ;(listCurrencies as Mock).mockRejectedValue(new Error('Network error'))
    ;(getConsolidationCurrency as Mock).mockRejectedValue(new Error('Network error'))
    ;(getAppSetting as Mock).mockRejectedValue(new Error('Network error'))

    const { result } = renderHook(() => useSettings(), { wrapper: makeWrapper() })

    await waitFor(() => {
      expect(consoleSpy).toHaveBeenCalledWith('Failed to load settings:', expect.any(Error))
    })

    // State remains at defaults
    expect(result.current.currencies).toEqual([])
    expect(result.current.selectedCurrency).toBeNull()
    expect(result.current.apiKey).toBe('')

    consoleSpy.mockRestore()
  })

  // ── handleCurrencySelect ──────────────────────────────────────────────

  it('saves the selected currency and calls the callback', async () => {
    setupMocks()

    const { result } = renderHook(() => useSettings(), { wrapper: makeWrapper() })

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES)
    })

    await act(async () => {
      await result.current.handleCurrencySelect(USD)
    })

    expect(setConsolidationCurrency).toHaveBeenCalledWith(2)
    // TODO Phase 4: verify queryClient.invalidateQueries(['consolidation-currency']) is called
    expect(result.current.selectedCurrency).toEqual(USD)
  })

  it('flashes currencySaved flag for 2 seconds after selecting currency', async () => {
    setupMocks()

    const { result } = renderHook(() => useSettings(), { wrapper: makeWrapper() })

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES)
    })

    await act(async () => {
      await result.current.handleCurrencySelect(USD)
    })

    expect(result.current.currencySaved).toBe(true)

    act(() => {
      vi.advanceTimersByTime(2100)
    })

    expect(result.current.currencySaved).toBe(false)
  })

  it('toasts a currency save failure and reverts the selection', async () => {
    setupMocks()
    ;(setConsolidationCurrency as Mock).mockRejectedValue(new Error('Save failed'))

    const { result } = renderHook(() => useSettings(), { wrapper: makeWrapper() })

    await waitFor(() => {
      expect(result.current.selectedCurrency).toEqual(EUR)
    })

    await act(async () => {
      await result.current.handleCurrencySelect(USD)
    })

    expect(mockToastError).toHaveBeenCalledWith('settings.saveCurrencyError:{"error":"Save failed"}')
    expect(result.current.selectedCurrency).toEqual(EUR)
    expect(result.current.currencySaved).toBe(false)
  })

  // ── handleSaveApiKey ──────────────────────────────────────────────────

  it('saves the trimmed API key', async () => {
    setupMocks()

    const { result } = renderHook(() => useSettings(), { wrapper: makeWrapper() })

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES)
    })

    act(() => {
      result.current.setApiKey('  my-key  ')
    })

    await act(async () => {
      await result.current.handleSaveApiKey()
    })

    expect(setAppSetting).toHaveBeenCalledWith('oxr_app_id', 'my-key')
  })

  it('flashes apiKeySaved flag for 2 seconds after saving', async () => {
    setupMocks()

    const { result } = renderHook(() => useSettings(), { wrapper: makeWrapper() })

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES)
    })

    act(() => {
      result.current.setApiKey('key123')
    })

    await act(async () => {
      await result.current.handleSaveApiKey()
    })

    expect(result.current.apiKeySaved).toBe(true)

    act(() => {
      vi.advanceTimersByTime(2100)
    })

    expect(result.current.apiKeySaved).toBe(false)
  })

  it('toasts an API key save failure', async () => {
    setupMocks()
    ;(setAppSetting as Mock).mockRejectedValue(new Error('Save failed'))

    const { result } = renderHook(() => useSettings(), { wrapper: makeWrapper() })

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES)
    })

    act(() => {
      result.current.setApiKey('key')
    })

    await act(async () => {
      await result.current.handleSaveApiKey()
    })

    expect(mockToastError).toHaveBeenCalledWith('settings.saveApiKeyError:{"error":"Save failed"}')
    expect(result.current.apiKeySaved).toBe(false)
  })
})
