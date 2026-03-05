/**
 * Format a minor-unit integer amount as a EUR display string.
 * Example: 5000 → "€50.00", -1234 → "-€12.34"
 */
export function formatEur(amountMinor: number): string {
  const isNegative = amountMinor < 0;
  const abs = Math.abs(amountMinor);
  const euros = Math.floor(abs / 100);
  const cents = abs % 100;
  const formatted = `€${euros}.${cents.toString().padStart(2, '0')}`;
  return isNegative ? `-${formatted}` : formatted;
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
