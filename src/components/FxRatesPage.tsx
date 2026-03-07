import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import Decimal from 'decimal.js';
import type { FxRateRow, Currency } from '../types';
import {
  listFxRates,
  fetchFxRates,
  listCurrencies,
  getConsolidationCurrency,
  setFxRateManual,
  getMissingRateDates,
} from '../api';
import { cn } from '@/lib/utils';
import { Button } from './ui/button';
import { DatePicker } from './ui/date-picker';
import { Label } from './ui/label';
import { todayIso } from '../utils/format';

const formatRate = (r: FxRateRow): string =>
  new Decimal(`${r.rateMantissa}e${r.rateExponent}`).toString();

function parseRateInput(input: string): { mantissa: number; exponent: number } | null {
  try {
    const d = new Decimal(input);
    if (d.isZero() || d.isNegative()) return null;
    let str = d.toFixed();
    // Strip trailing zeros from decimal part so "1.0" → "1", "1.0842" unchanged
    if (str.includes('.')) {
      str = str.replace(/0+$/, '').replace(/\.$/, '');
    }
    const parts = str.split('.');
    const mantissa = parseInt(parts.join(''), 10);
    const exponent = parts[1] ? -parts[1].length : 0;
    if (!Number.isFinite(mantissa) || mantissa === 0) return null;
    return { mantissa, exponent };
  } catch {
    return null;
  }
}

export default function FxRatesPage() {
  const { t } = useTranslation();
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
      setError(String(err));
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
    listCurrencies()
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
      await fetchFxRates(fetchDate);
      await loadRates();
      await loadMissingDates();
    } catch (err) {
      setError(String(err));
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
      setError(String(err));
    } finally {
      setIsBackfilling(false);
    }
  };

  const codeToId = new Map<string, number>(currencies.map((c) => [c.code, c.id]));

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
      setError(t('fxRates.invalidRate'));
      return;
    }
    const fromId = codeToId.get(consolidationCode);
    const toId = codeToId.get(code);
    if (fromId == null || toId == null) {
      setError(t('fxRates.currencyNotFound'));
      return;
    }
    try {
      await setFxRateManual(fromId, toId, date, parsed.mantissa, parsed.exponent);
      await loadRates();
    } catch (err) {
      setError(String(err));
    }
  };

  // Pivot table construction
  // dates → rows (most recent first)
  const dates = [...new Set(rates.map((r) => r.date))].sort((a, b) => b.localeCompare(a));
  // toCurrencyCode → columns (target/non-consolidation currencies)
  const targetCurrencies = [...new Set(rates.map((r) => r.toCurrencyCode))].sort();

  // Key: `${date}:${toCurrencyCode}`
  const rateMap = new Map<string, FxRateRow>();
  for (const r of rates) {
    rateMap.set(`${r.date}:${r.toCurrencyCode}`, r);
  }

  return (
    <div className="px-4 md:px-10 py-8">
      <div className="mb-6">
        {/* Row 1: Summary */}
        <div className="mb-4">
          <h2 className="text-2xl font-bold">{t('fxRates.title')}</h2>
          {consolidationCode && (
            <p className="text-sm text-muted-foreground">
              {t('fxRates.subtitle', { currency: consolidationCode })}
            </p>
          )}
        </div>
        {/* Row 2: Actions */}
        <div className="flex items-center gap-3">
          {missingDates.length > 0 && (
            <Button variant="outline" onClick={handleBackfill} disabled={isBackfilling}>
              {isBackfilling
                ? t('fxRates.backfilling')
                : t('fxRates.backfill', { count: missingDates.length })}
            </Button>
          )}
          <Label className="text-sm font-medium text-muted-foreground">
            {t('fxRates.fetchHistorical')}
          </Label>
          <DatePicker value={fetchDate} onChange={setFetchDate} className="w-44" />
          <Button onClick={handleRefresh} disabled={isRefreshing}>
            {isRefreshing ? '…' : t('fxRates.refreshRates')}
          </Button>
        </div>
      </div>

      {error && (
        <div className="mb-4 rounded-md border border-destructive bg-destructive/10 px-4 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {dates.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('fxRates.noRates')}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm border-collapse">
            <thead>
              <tr className="border-b border-border">
                <th className="sticky left-0 z-[1] bg-card text-left py-2 pr-4 font-semibold text-muted-foreground">
                  <div>{t('fxRates.date')}</div>
                  {consolidationCode && (
                    <div className="text-xs font-normal">
                      {t('fxRates.baseCurrencyPrefix', { currency: consolidationCode })}
                    </div>
                  )}
                </th>
                {targetCurrencies.map((code) => (
                  <th key={code} className="text-right py-2 px-3 font-semibold">
                    {code}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {dates.map((date) => (
                <tr
                  key={date}
                  className="group border-b border-border last:border-0 even:bg-muted/50 hover:bg-muted/30"
                >
                  <td className="sticky left-0 z-[1] bg-card group-even:bg-muted/50 py-2 pr-4 text-muted-foreground font-mono text-sm">
                    {date}
                  </td>
                  {targetCurrencies.map((code) => {
                    const row = rateMap.get(`${date}:${code}`);
                    const isEditing = editingCell?.date === date && editingCell?.code === code;
                    return (
                      <td
                        key={code}
                        className={cn(
                          'text-right py-1 px-3 font-mono text-sm cursor-pointer',
                          row?.isManual && 'font-bold bg-amber-50 dark:bg-amber-900/20',
                        )}
                        onClick={() => {
                          if (!isEditing) handleCellClick(date, code, row);
                        }}
                      >
                        {isEditing ? (
                          <input
                            autoFocus
                            type="text"
                            value={editValue}
                            onChange={(e) => setEditValue(e.target.value)}
                            onBlur={(e) => handleCellSave(date, code, e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') {
                                e.preventDefault();
                                handleCellSave(date, code, editValue);
                              } else if (e.key === 'Escape') {
                                setEditingCell(null);
                              }
                            }}
                            className="w-24 text-right font-mono text-sm border border-border rounded px-1 bg-background"
                          />
                        ) : (
                          <>
                            {row ? formatRate(row) : '—'}
                            {row?.isManual && (
                              <span className="ml-1 text-amber-600 text-xs">
                                ({t('fxRates.isManual')})
                              </span>
                            )}
                          </>
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
