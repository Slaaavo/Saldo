import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { FxRateRow, Currency } from '../../shared/types';
import {
  listCustomUnits,
  listFxRates,
  getConsolidationCurrency,
  setFxRateManual,
} from '../../shared/api';
import { extractErrorMessage } from '../../shared/utils/errors';
import { formatPrice, parsePriceAsRate } from './unitPricing';
import { cn } from '@/shared/lib/utils';

export default function UnitsPage() {
  const { t } = useTranslation();
  const [units, setUnits] = useState<Currency[]>([]);
  const [rates, setRates] = useState<FxRateRow[]>([]);
  const [consolidationCurrency, setConsolidationCurrency] = useState<Currency | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editingCell, setEditingCell] = useState<{ date: string; code: string } | null>(null);
  const [editValue, setEditValue] = useState('');

  const fetchData = () =>
    Promise.all([listCustomUnits(), listFxRates(), getConsolidationCurrency()]);

  const applyData = ([u, r, c]: [Currency[], FxRateRow[], Currency]) => {
    setUnits(u);
    setRates(r);
    setConsolidationCurrency(c);
  };

  const loadData = async () => {
    try {
      applyData(await fetchData());
    } catch (err) {
      setError(extractErrorMessage(err));
    }
  };

  useEffect(() => {
    fetchData()
      .then(applyData)
      .catch((err) => setError(extractErrorMessage(err)));
  }, []);

  const codeToId = new Map<string, number>([
    ...(consolidationCurrency
      ? [[consolidationCurrency.code, consolidationCurrency.id] as [string, number]]
      : []),
    ...units.map((u) => [u.code, u.id] as [string, number]),
  ]);

  // Filter rates to only those involving custom units as toCurrency
  const unitCodes = new Set(units.map((u) => u.code));
  const unitRates = rates.filter((r) => unitCodes.has(r.toCurrencyCode));

  // Build pivot: dates × unit codes
  const dates = [...new Set(unitRates.map((r) => r.date))].sort((a, b) => b.localeCompare(a));

  // Key: `${date}:${toCurrencyCode}`
  const rateMap = new Map<string, FxRateRow>();
  for (const r of unitRates) {
    rateMap.set(`${r.date}:${r.toCurrencyCode}`, r);
  }

  const handleCellClick = (date: string, code: string, row: FxRateRow | undefined) => {
    setEditingCell({ date, code });
    setEditValue(row ? formatPrice(row) : '');
  };

  const handleCellSave = async (date: string, code: string, value: string) => {
    setEditingCell(null);
    const trimmed = value.trim();
    if (!trimmed) return;
    const parsed = parsePriceAsRate(trimmed);
    if (!parsed) {
      setError(t('units.invalidPrice'));
      return;
    }
    const fromId = consolidationCurrency ? codeToId.get(consolidationCurrency.code) : undefined;
    const toId = codeToId.get(code);
    if (fromId == null || toId == null) {
      setError(t('fxRates.currencyNotFound'));
      return;
    }
    try {
      await setFxRateManual(fromId, toId, date, parsed.mantissa, parsed.exponent);
      await loadData();
    } catch (err) {
      setError(extractErrorMessage(err));
    }
  };

  return (
    <div className="px-4 md:px-10 py-8">
      <div className="mb-6">
        <h2 className="text-2xl font-bold">{t('units.title')}</h2>
        {consolidationCurrency && (
          <p className="text-sm text-muted-foreground">
            {t('units.subtitle', { currency: consolidationCurrency.name })}
          </p>
        )}
      </div>

      {error && (
        <div className="mb-4 rounded-md border border-destructive bg-destructive/10 px-4 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {units.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('units.empty')}</p>
      ) : dates.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('units.noPrices')}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm border-collapse">
            <thead>
              <tr className="border-b border-border">
                <th className="sticky left-0 z-[1] bg-card text-left py-2 pr-4 font-semibold text-muted-foreground">
                  {t('fxRates.date')}
                </th>
                {units.map((u) => (
                  <th key={u.id} className="text-right py-2 px-3 font-semibold">
                    {u.code}
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
                  {units.map((u) => {
                    const row = rateMap.get(`${date}:${u.code}`);
                    const isEditing = editingCell?.date === date && editingCell?.code === u.code;
                    return (
                      <td
                        key={u.id}
                        className={cn(
                          'text-right py-1 px-3 font-mono text-sm cursor-pointer',
                          row?.isManual && 'font-bold bg-amber-50 dark:bg-amber-900/20',
                        )}
                        onClick={() => {
                          if (!isEditing) handleCellClick(date, u.code, row);
                        }}
                      >
                        {isEditing ? (
                          <input
                            autoFocus
                            type="text"
                            value={editValue}
                            onChange={(e) => setEditValue(e.target.value)}
                            onBlur={(e) => handleCellSave(date, u.code, e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') {
                                e.preventDefault();
                                handleCellSave(date, u.code, editValue);
                              } else if (e.key === 'Escape') {
                                setEditingCell(null);
                              }
                            }}
                            className="w-24 text-right font-mono text-sm border border-border rounded px-1 bg-background"
                          />
                        ) : (
                          <>
                            {row ? formatPrice(row) : '—'}
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
