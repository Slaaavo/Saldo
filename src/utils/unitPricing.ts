import Decimal from 'decimal.js';
import type { FxRateRow } from '../types';

/**
 * Display the stored rate as a price (inverted): price = 1 / rate.
 * rate is stored as mantissa × 10^exponent.
 */
export function formatPrice(r: FxRateRow): string {
  try {
    return new Decimal(1)
      .div(new Decimal(`${r.rateMantissa}e${r.rateExponent}`))
      .toSignificantDigits(6)
      .toString();
  } catch {
    return '—';
  }
}

/**
 * Parse a price string entered by the user and return the inverted rate
 * as {mantissa, exponent} for storage.
 */
export function parsePriceAsRate(input: string): { mantissa: number; exponent: number } | null {
  try {
    const price = new Decimal(input);
    if (price.isZero() || price.isNegative()) return null;
    // rate = 1 / price — limit to 15 significant digits so the mantissa fits in
    // a JS safe integer and a Rust i64 (toFixed() on a full Decimal can produce 20+ digits).
    const rate = new Decimal(1).div(price).toSignificantDigits(15);
    let str = rate.toFixed();
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
