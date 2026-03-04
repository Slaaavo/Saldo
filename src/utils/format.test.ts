import { describe, it, expect } from 'vitest';
import { formatEur, formatDate, toEndOfDay, todayIso } from './format';

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
