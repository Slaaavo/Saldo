import { describe, it, expect } from 'vitest';
import { formatEur, formatDate, toEndOfDay, todayIso, formatDisplayDate } from './format';

describe('formatEur', () => {
  it('formats zero', () => {
    expect(formatEur(0)).toBe('€0.00');
  });

  it('formats positive amounts', () => {
    expect(formatEur(5000)).toBe('€50.00');
    expect(formatEur(1)).toBe('€0.01');
    expect(formatEur(100)).toBe('€1.00');
    expect(formatEur(123456)).toBe('€1234.56');
  });

  it('formats negative amounts', () => {
    expect(formatEur(-5000)).toBe('-€50.00');
    expect(formatEur(-1)).toBe('-€0.01');
  });
});

describe('formatDate', () => {
  it('extracts date portion from datetime', () => {
    expect(formatDate('2026-03-01T12:30:00')).toBe('2026-03-01');
  });

  it('handles date-only strings', () => {
    expect(formatDate('2026-03-01')).toBe('2026-03-01');
  });
});

describe('toEndOfDay', () => {
  it('appends T23:59:59', () => {
    expect(toEndOfDay('2026-03-01')).toBe('2026-03-01T23:59:59');
  });
});

describe('todayIso', () => {
  it('returns a YYYY-MM-DD string', () => {
    const result = todayIso();
    expect(result).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });
});

describe('formatDisplayDate', () => {
  it('formats a date string as a readable date', () => {
    expect(formatDisplayDate('2026-03-05')).toBe('March 5, 2026');
  });

  it('handles single-digit months and days', () => {
    expect(formatDisplayDate('2026-01-01')).toBe('January 1, 2026');
  });

  it('formats December correctly', () => {
    expect(formatDisplayDate('2025-12-25')).toBe('December 25, 2025');
  });
});
