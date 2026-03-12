import { describe, it, expect } from 'vitest';
import {
  formatAmount,
  formatDate,
  toEndOfDay,
  todayIso,
  formatDisplayDate,
  toMinorUnits,
  fromMinorUnits,
  getMinorUnitsStep,
} from './format';

describe('formatAmount', () => {
  it('formats zero', () => {
    expect(formatAmount(0)).toBe('0.00 €');
  });

  it('formats small amounts', () => {
    expect(formatAmount(1)).toBe('0.01 €');
    expect(formatAmount(100)).toBe('1.00 €');
  });

  it('formats positive amounts', () => {
    expect(formatAmount(5000)).toBe('50.00 €');
  });

  it('formats with thousands separators', () => {
    expect(formatAmount(123456)).toBe('1 234.56 €');
    expect(formatAmount(100000000)).toBe('1 000 000.00 €');
  });

  it('formats negative amounts', () => {
    expect(formatAmount(-5000)).toBe('-50.00 €');
    expect(formatAmount(-1)).toBe('-0.01 €');
    expect(formatAmount(-123456)).toBe('-1 234.56 €');
  });

  it('supports custom config override', () => {
    const config = {
      currencySymbol: '$',
      currencyPosition: 'left' as const,
      thousandsSeparator: ',',
      decimalSeparator: '.',
    };
    expect(formatAmount(123456, 2, config)).toBe('$ 1,234.56');
    expect(formatAmount(-5000, 2, config)).toBe('-$ 50.00');
  });

  it('supports minorUnits=0 (no decimals)', () => {
    expect(formatAmount(1234, 0)).toBe('1 234 €');
    expect(formatAmount(0, 0)).toBe('0 €');
  });

  it('supports minorUnits=3 (3 decimal places)', () => {
    expect(formatAmount(123456, 3)).toBe('123.456 €');
    expect(formatAmount(1000, 3)).toBe('1.000 €');
    expect(formatAmount(1, 3)).toBe('0.001 €');
  });

  it('overrides symbol with currencyCode (USD, minorUnits=2)', () => {
    expect(formatAmount(100000, 2, undefined, 'USD')).toBe('1 000.00 USD');
  });

  it('overrides symbol with currencyCode for zero-decimal currency (JPY)', () => {
    expect(formatAmount(15723, 0, undefined, 'JPY')).toBe('15 723 JPY');
  });

  it('overrides symbol with currencyCode for 8-decimal currency (BTC)', () => {
    expect(formatAmount(50000000, 8, undefined, 'BTC')).toBe('0.50000000 BTC');
  });

  it('overrides symbol with currencyCode for negative amounts', () => {
    expect(formatAmount(-5000, 2, undefined, 'EUR')).toBe('-50.00 EUR');
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
    expect(formatDisplayDate('2026-03-05')).toBe('5 March 2026');
  });

  it('handles single-digit months and days', () => {
    expect(formatDisplayDate('2026-01-01')).toBe('1 January 2026');
  });

  it('formats December correctly', () => {
    expect(formatDisplayDate('2025-12-25')).toBe('25 December 2025');
  });
});

describe('toMinorUnits', () => {
  it('converts a 2-decimal string to minor units', () => {
    expect(toMinorUnits('12.34', 2)).toBe(1234);
    expect(toMinorUnits('0.01', 2)).toBe(1);
    expect(toMinorUnits('1000.00', 2)).toBe(100000);
  });

  it('handles zero-decimal currencies', () => {
    expect(toMinorUnits('150', 0)).toBe(150);
    expect(toMinorUnits('0', 0)).toBe(0);
  });

  it('handles 8-decimal (BTC) precision', () => {
    expect(toMinorUnits('0.50000000', 8)).toBe(50000000);
    expect(toMinorUnits('1.00000001', 8)).toBe(100000001);
  });

  it('round-trips with fromMinorUnits', () => {
    expect(toMinorUnits(fromMinorUnits(1234, 2), 2)).toBe(1234);
    expect(toMinorUnits(fromMinorUnits(0, 2), 2)).toBe(0);
    expect(toMinorUnits(fromMinorUnits(15723, 0), 0)).toBe(15723);
    expect(toMinorUnits(fromMinorUnits(50000000, 8), 8)).toBe(50000000);
  });
});

describe('fromMinorUnits', () => {
  it('converts minor units to a decimal string', () => {
    expect(fromMinorUnits(1234, 2)).toBe('12.34');
    expect(fromMinorUnits(1, 2)).toBe('0.01');
    expect(fromMinorUnits(100000, 2)).toBe('1000.00');
  });

  it('handles zero-decimal currencies', () => {
    expect(fromMinorUnits(15723, 0)).toBe('15723');
    expect(fromMinorUnits(0, 0)).toBe('0');
  });

  it('handles 8-decimal (BTC) precision', () => {
    expect(fromMinorUnits(50000000, 8)).toBe('0.50000000');
    expect(fromMinorUnits(100000001, 8)).toBe('1.00000001');
  });
});

describe('getMinorUnitsStep', () => {
  it('returns "1" for zero-decimal currencies', () => {
    expect(getMinorUnitsStep(0)).toBe('1');
  });

  it('returns "0.01" for 2-decimal currencies', () => {
    expect(getMinorUnitsStep(2)).toBe('0.01');
  });

  it('returns "0.001" for 3-decimal currencies', () => {
    expect(getMinorUnitsStep(3)).toBe('0.001');
  });

  it('returns correct step for 8-decimal (BTC)', () => {
    expect(getMinorUnitsStep(8)).toBe('0.00000001');
  });

  it('returns "0.1" for 1-decimal currency', () => {
    expect(getMinorUnitsStep(1)).toBe('0.1');
  });
});
