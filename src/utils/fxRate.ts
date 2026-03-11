import Decimal from 'decimal.js';
import type { FxRateRow, Currency } from '../types';

export const formatRate = (r: FxRateRow): string =>
  new Decimal(`${r.rateMantissa}e${r.rateExponent}`).toString();

export function parseRateInput(input: string): { mantissa: number; exponent: number } | null {
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

/**
 * Build the pivot-table data for the FX rates page.
 * Filters out rates whose toCurrencyCode is not in the known currencies list,
 * then returns sorted dates (most recent first), sorted target currency codes,
 * and a lookup map keyed by `${date}:${toCurrencyCode}`.
 */
export function buildRatePivot(
  rates: FxRateRow[],
  currencies: Currency[],
): {
  dates: string[];
  targetCurrencies: string[];
  rateMap: Map<string, FxRateRow>;
} {
  // Gate on currencies.length > 0 to avoid filtering everything while currencies are loading.
  const validCodes = currencies.length > 0 ? new Set(currencies.map((c) => c.code)) : null;
  const filtered = validCodes ? rates.filter((r) => validCodes.has(r.toCurrencyCode)) : rates;

  const dates = [...new Set(filtered.map((r) => r.date))].sort((a, b) => b.localeCompare(a));
  const targetCurrencies = [...new Set(filtered.map((r) => r.toCurrencyCode))].sort();

  const rateMap = new Map<string, FxRateRow>();
  for (const r of filtered) {
    rateMap.set(`${r.date}:${r.toCurrencyCode}`, r);
  }

  return { dates, targetCurrencies, rateMap };
}
