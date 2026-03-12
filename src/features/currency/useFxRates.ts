import { useState, useEffect, useCallback, useMemo } from 'react';
import type { FxRateRow, Currency } from '../../shared/types';
import {
  listFxRates,
  fetchFxRates,
  listCurrencies,
  getConsolidationCurrency,
  setFxRateManual,
  getMissingRateDates,
} from '../../shared/api';
import { extractErrorMessage } from '../../shared/utils/errors';
import { formatRate, parseRateInput, buildRatePivot } from './fxRate';
import { todayIso } from '../../shared/utils/format';

export function useFxRates() {
  const [rates, setRates] = useState<FxRateRow[]>([]);
  const [currencies, setCurrencies] = useState<Currency[]>([]);
  const [consolidationCode, setConsolidationCode] = useState('');
  const [fetchDate, setFetchDate] = useState(todayIso());
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editingCell, setEditingCell] = useState<{ date: string; code: string } | null>(null);
  const [editValue, setEditValue] = useState('');
  const [missingDates, setMissingDates] = useState<string[]>([]);
  const [isBackfilling, setIsBackfilling] = useState(false);

  const loadRates = useCallback(async () => {
    try {
      const data = await listFxRates();
      setRates(data);
    } catch (err) {
      setError(extractErrorMessage(err));
    }
  }, []);

  const loadMissingDates = useCallback(async () => {
    try {
      const dates = await getMissingRateDates();
      setMissingDates(dates);
    } catch {
      // Non-critical — silently ignore
    }
  }, []);

  useEffect(() => {
    loadRates();
    loadMissingDates();
    listCurrencies(false)
      .then(setCurrencies)
      .catch(() => {});
    getConsolidationCurrency()
      .then((c) => setConsolidationCode(c.code))
      .catch(() => {});
  }, [loadRates, loadMissingDates]);

  const handleRefresh = async () => {
    setIsRefreshing(true);
    setError(null);
    try {
      await fetchFxRates(fetchDate, true);
      await loadRates();
      await loadMissingDates();
    } catch (err) {
      setError(extractErrorMessage(err));
    } finally {
      setIsRefreshing(false);
    }
  };

  const handleBackfill = async () => {
    setIsBackfilling(true);
    setError(null);
    try {
      for (const date of missingDates) {
        await fetchFxRates(date);
      }
      await loadRates();
      await loadMissingDates();
    } catch (err) {
      setError(extractErrorMessage(err));
    } finally {
      setIsBackfilling(false);
    }
  };

  const codeToId = useMemo(
    () => new Map<string, number>(currencies.map((c) => [c.code, c.id])),
    [currencies],
  );

  const { dates, targetCurrencies, rateMap } = useMemo(
    () => buildRatePivot(rates, currencies),
    [rates, currencies],
  );

  const handleCellClick = (date: string, code: string, row: FxRateRow | undefined) => {
    setEditingCell({ date, code });
    setEditValue(row ? formatRate(row) : '');
  };

  const handleCellSave = async (date: string, code: string, value: string) => {
    setEditingCell(null);
    const trimmed = value.trim();
    if (!trimmed) return;
    const parsed = parseRateInput(trimmed);
    if (!parsed) {
      setError('fxRates.invalidRate');
      return;
    }
    const fromId = codeToId.get(consolidationCode);
    const toId = codeToId.get(code);
    if (fromId == null || toId == null) {
      setError('fxRates.currencyNotFound');
      return;
    }
    try {
      await setFxRateManual(fromId, toId, date, parsed.mantissa, parsed.exponent);
      await loadRates();
    } catch (err) {
      setError(extractErrorMessage(err));
    }
  };

  const cancelEdit = () => setEditingCell(null);

  return {
    // Data
    dates,
    targetCurrencies,
    rateMap,
    consolidationCode,
    missingDates,
    error,
    fetchDate,
    isRefreshing,
    isBackfilling,
    editingCell,
    editValue,

    // Actions
    setFetchDate,
    setEditValue,
    handleRefresh,
    handleBackfill,
    handleCellClick,
    handleCellSave,
    cancelEdit,
  };
}
