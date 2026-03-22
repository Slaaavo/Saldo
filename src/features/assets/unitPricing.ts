import Decimal from 'decimal.js';
import type { FxRateRow } from '../../shared/types';

/**
 * Display the stored rate as a price.
 * If isDirect, the rate value IS the price. Otherwise invert: price = 1 / rate.
 */
export function formatPrice(r: FxRateRow): string {
  try {
    const rateDecimal = new Decimal(`${r.rateMantissa}e${r.rateExponent}`);
    if (r.isDirect) {
      return rateDecimal.toSignificantDigits(6).toString();
    }
    return new Decimal(1).div(rateDecimal).toSignificantDigits(6).toString();
  } catch {
    return '—';
  }
}

/**
 * Parse a price string entered by the user and return it as {mantissa, exponent} for storage.
 * Stored directly (no inversion) — the stored rate IS the price.
 */
export function parsePriceAsRate(input: string): { mantissa: number; exponent: number } | null {
  try {
    const price = new Decimal(input);
    if (price.isZero() || price.isNegative()) return null;
    // Store the price directly — limit to 15 significant digits so the mantissa fits in
    // a JS safe integer and a Rust i64.
    const priceDecimal = price.toSignificantDigits(15);
    let str = priceDecimal.toFixed();
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
