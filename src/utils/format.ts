import { defaultNumberFormat, type NumberFormatConfig } from '../config/numberFormat';

/**
 * Format a minor-unit integer amount as a display string.
 * Example: 123456 (minorUnits=2) → "1 234.56 €"
 */
export function formatAmount(
  amountMinor: number,
  minorUnits: number = 2,
  config: NumberFormatConfig = defaultNumberFormat,
): string {
  const isNegative = amountMinor < 0;
  const abs = Math.abs(amountMinor);
  const divisor = Math.pow(10, minorUnits);
  const integerPart = Math.floor(abs / divisor);
  const fractionalPart = abs % divisor;

  // Insert thousands separator
  const intStr = integerPart.toString();
  let withSeparators = '';
  for (let i = 0; i < intStr.length; i++) {
    if (i > 0 && (intStr.length - i) % 3 === 0) {
      withSeparators += config.thousandsSeparator;
    }
    withSeparators += intStr[i];
  }

  let numberStr = withSeparators;
  if (minorUnits > 0) {
    numberStr += config.decimalSeparator + fractionalPart.toString().padStart(minorUnits, '0');
  }

  let result: string;
  if (config.currencyPosition === 'left') {
    result = `${config.currencySymbol} ${numberStr}`;
  } else {
    result = `${numberStr} ${config.currencySymbol}`;
  }

  return isNegative ? `-${result}` : result;
}

/**
 * Format an ISO datetime string to display only the date portion.
 * Example: "2026-03-01T12:00:00" → "2026-03-01"
 */
export function formatDate(isoDatetime: string): string {
  return isoDatetime.substring(0, 10);
}

/**
 * Convert a date string (YYYY-MM-DD) to end-of-day datetime.
 */
export function toEndOfDay(dateStr: string): string {
  return `${dateStr}T23:59:59`;
}

/**
 * Get today's date as YYYY-MM-DD.
 */
export function todayIso(): string {
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, '0');
  const d = String(now.getDate()).padStart(2, '0');
  return `${y}-${m}-${d}`;
}

/**
 * Format a YYYY-MM-DD date string as a human-readable date.
 * Example: "2026-03-05" → "March 5, 2026"
 */
export function formatDisplayDate(dateStr: string): string {
  const [y, m, d] = dateStr.split('-').map(Number);
  const date = new Date(y, m - 1, d);
  return date.toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  });
}
