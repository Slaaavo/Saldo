import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import { useSettings } from './useSettings';
import type { Currency } from '../types';

// ── Mock the API layer ──────────────────────────────────────────────────────
vi.mock('../api', () => ({
  listCurrencies: vi.fn(),
  getConsolidationCurrency: vi.fn(),
  setConsolidationCurrency: vi.fn(),
  getAppSetting: vi.fn(),
  setAppSetting: vi.fn(),
}));

import {
  listCurrencies,
  getConsolidationCurrency,
  setConsolidationCurrency,
  getAppSetting,
  setAppSetting,
} from '../api';

// ── Test data ───────────────────────────────────────────────────────────────
const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false };
const USD: Currency = { id: 2, code: 'USD', name: 'US Dollar', minorUnits: 2, isCustom: false };
const ALL_CURRENCIES = [EUR, USD];

function setupMocks(overrides?: {
  currencies?: Currency[];
  consolidation?: Currency;
  apiKey?: string | null;
}) {
  (listCurrencies as Mock).mockResolvedValue(overrides?.currencies ?? ALL_CURRENCIES);
  (getConsolidationCurrency as Mock).mockResolvedValue(overrides?.consolidation ?? EUR);
  (getAppSetting as Mock).mockResolvedValue(overrides?.apiKey ?? null);
  (setConsolidationCurrency as Mock).mockResolvedValue(undefined);
  (setAppSetting as Mock).mockResolvedValue(undefined);
}

describe('useSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // ── Loading ───────────────────────────────────────────────────────────

  it('loads currencies, consolidation currency, and API key on mount', async () => {
    setupMocks({ apiKey: 'stored-key' });
    const onConsolidationCurrencyChange = vi.fn();

    const { result } = renderHook(() => useSettings({ onConsolidationCurrencyChange }));

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES);
      expect(result.current.selectedCurrency).toEqual(EUR);
      expect(result.current.apiKey).toBe('stored-key');
    });

    expect(listCurrencies).toHaveBeenCalledOnce();
    expect(getConsolidationCurrency).toHaveBeenCalledOnce();
    expect(getAppSetting).toHaveBeenCalledWith('oxr_app_id');
  });

  it('does not set API key when stored value is null', async () => {
    setupMocks({ apiKey: null });

    const { result } = renderHook(() => useSettings({ onConsolidationCurrencyChange: vi.fn() }));

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES);
    });

    expect(result.current.apiKey).toBe('');
  });

  it('handles load failure gracefully', async () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    (listCurrencies as Mock).mockRejectedValue(new Error('Network error'));
    (getConsolidationCurrency as Mock).mockRejectedValue(new Error('Network error'));
    (getAppSetting as Mock).mockRejectedValue(new Error('Network error'));

    const { result } = renderHook(() => useSettings({ onConsolidationCurrencyChange: vi.fn() }));

    await waitFor(() => {
      expect(consoleSpy).toHaveBeenCalledWith('Failed to load settings:', expect.any(Error));
    });

    // State remains at defaults
    expect(result.current.currencies).toEqual([]);
    expect(result.current.selectedCurrency).toBeNull();
    expect(result.current.apiKey).toBe('');

    consoleSpy.mockRestore();
  });

  // ── handleCurrencySelect ──────────────────────────────────────────────

  it('saves the selected currency and calls the callback', async () => {
    setupMocks();
    const onConsolidationCurrencyChange = vi.fn();

    const { result } = renderHook(() => useSettings({ onConsolidationCurrencyChange }));

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES);
    });

    await act(async () => {
      await result.current.handleCurrencySelect(USD);
    });

    expect(setConsolidationCurrency).toHaveBeenCalledWith(2);
    expect(onConsolidationCurrencyChange).toHaveBeenCalledOnce();
    expect(result.current.selectedCurrency).toEqual(USD);
  });

  it('flashes currencySaved flag for 2 seconds after selecting currency', async () => {
    setupMocks();

    const { result } = renderHook(() => useSettings({ onConsolidationCurrencyChange: vi.fn() }));

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES);
    });

    await act(async () => {
      await result.current.handleCurrencySelect(USD);
    });

    expect(result.current.currencySaved).toBe(true);

    act(() => {
      vi.advanceTimersByTime(2100);
    });

    expect(result.current.currencySaved).toBe(false);
  });

  it('handles currency save failure gracefully', async () => {
    setupMocks();
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    (setConsolidationCurrency as Mock).mockRejectedValue(new Error('Save failed'));

    const onConsolidationCurrencyChange = vi.fn();
    const { result } = renderHook(() => useSettings({ onConsolidationCurrencyChange }));

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES);
    });

    await act(async () => {
      await result.current.handleCurrencySelect(USD);
    });

    expect(consoleSpy).toHaveBeenCalledWith(
      'Failed to set consolidation currency:',
      expect.any(Error),
    );
    expect(onConsolidationCurrencyChange).not.toHaveBeenCalled();
    expect(result.current.currencySaved).toBe(false);

    consoleSpy.mockRestore();
  });

  // ── handleSaveApiKey ──────────────────────────────────────────────────

  it('saves the trimmed API key', async () => {
    setupMocks();

    const { result } = renderHook(() => useSettings({ onConsolidationCurrencyChange: vi.fn() }));

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES);
    });

    act(() => {
      result.current.setApiKey('  my-key  ');
    });

    await act(async () => {
      await result.current.handleSaveApiKey();
    });

    expect(setAppSetting).toHaveBeenCalledWith('oxr_app_id', 'my-key');
  });

  it('flashes apiKeySaved flag for 2 seconds after saving', async () => {
    setupMocks();

    const { result } = renderHook(() => useSettings({ onConsolidationCurrencyChange: vi.fn() }));

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES);
    });

    act(() => {
      result.current.setApiKey('key123');
    });

    await act(async () => {
      await result.current.handleSaveApiKey();
    });

    expect(result.current.apiKeySaved).toBe(true);

    act(() => {
      vi.advanceTimersByTime(2100);
    });

    expect(result.current.apiKeySaved).toBe(false);
  });

  it('handles API key save failure gracefully', async () => {
    setupMocks();
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    (setAppSetting as Mock).mockRejectedValue(new Error('Save failed'));

    const { result } = renderHook(() => useSettings({ onConsolidationCurrencyChange: vi.fn() }));

    await waitFor(() => {
      expect(result.current.currencies).toEqual(ALL_CURRENCIES);
    });

    act(() => {
      result.current.setApiKey('key');
    });

    await act(async () => {
      await result.current.handleSaveApiKey();
    });

    expect(consoleSpy).toHaveBeenCalledWith('Failed to save API key:', expect.any(Error));
    expect(result.current.apiKeySaved).toBe(false);

    consoleSpy.mockRestore();
  });
});
