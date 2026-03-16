import { describe, it, expect } from 'vitest';
import { parseCsvFile, autoDetectMapping, parseAmount, parseDateString } from './csvParser';

describe('parseCsvFile', () => {
  it('parses a valid CSV and returns headers and rows', async () => {
    const file = new File(['Date,Amount\n2026-01-01,100'], 'test.csv', { type: 'text/csv' });
    const result = await parseCsvFile(file);
    expect(result.headers).toEqual(['Date', 'Amount']);
    expect(result.rows).toHaveLength(1);
    expect(result.rows[0]).toMatchObject({ Date: '2026-01-01', Amount: '100' });
  });

  it('throws when the CSV has headers only and no data rows', async () => {
    const file = new File(['Date,Amount\n'], 'test.csv', { type: 'text/csv' });
    await expect(parseCsvFile(file)).rejects.toThrow();
  });

  it('throws when the CSV is completely empty', async () => {
    const file = new File([''], 'empty.csv', { type: 'text/csv' });
    await expect(parseCsvFile(file)).rejects.toThrow();
  });
});

describe('autoDetectMapping', () => {
  it('detects English date header', () => {
    const result = autoDetectMapping(['Date', 'Amount', 'Note']);
    expect(result.date).toBe('Date');
  });

  it('detects Slovak "datum" header', () => {
    const result = autoDetectMapping(['Datum', 'Suma']);
    expect(result.date).toBe('Datum');
    expect(result.amount).toBe('Suma');
  });

  it('detects diacritical Slovak "dátum"', () => {
    const result = autoDetectMapping(['Dátum transakcie', 'Čiastka']);
    expect(result.date).toBe('Dátum transakcie');
    expect(result.amount).toBe('Čiastka');
  });

  it('detects IBAN header as partner', () => {
    const result = autoDetectMapping(['IBAN protistrany', 'Suma']);
    expect(result.partner).toBe('IBAN protistrany');
  });

  it('detects note via Slovak "popis"', () => {
    const result = autoDetectMapping(['Popis platby']);
    expect(result.note).toBe('Popis platby');
  });

  it('detects note via Slovak "poznámka"', () => {
    const result = autoDetectMapping(['Poznámka']);
    expect(result.note).toBe('Poznámka');
  });

  it('detects currency header', () => {
    const result = autoDetectMapping(['Currency']);
    expect(result.currency).toBe('Currency');
  });

  it('detects Slovak "mena" as currency', () => {
    const result = autoDetectMapping(['Mena']);
    expect(result.currency).toBe('Mena');
  });

  it('detects fxRate via "rate"', () => {
    const result = autoDetectMapping(['Exchange Rate']);
    expect(result.fxRate).toBe('Exchange Rate');
  });

  it('detects Slovak "kurz" as fxRate', () => {
    const result = autoDetectMapping(['Kurz']);
    expect(result.fxRate).toBe('Kurz');
  });

  it('returns null for unmatched headers', () => {
    const result = autoDetectMapping(['Foo', 'Bar']);
    expect(result.date).toBeNull();
    expect(result.amount).toBeNull();
    expect(result.partner).toBeNull();
    expect(result.note).toBeNull();
    expect(result.currency).toBeNull();
    expect(result.fxRate).toBeNull();
  });

  it('handles empty headers array', () => {
    const result = autoDetectMapping([]);
    expect(result.date).toBeNull();
    expect(result.amount).toBeNull();
  });
});

describe('parseAmount', () => {
  it('parses plain US decimal: 1234.56', () => {
    expect(parseAmount('1234.56')).toBe(1234.56);
  });

  it('parses US thousands separator: 1,234.56', () => {
    expect(parseAmount('1,234.56')).toBe(1234.56);
  });

  it('parses European format: 1.234,56', () => {
    expect(parseAmount('1.234,56')).toBe(1234.56);
  });

  it('parses negative value: -45.00', () => {
    expect(parseAmount('-45.00')).toBe(-45);
  });

  it('parses leading plus sign: +100', () => {
    expect(parseAmount('+100')).toBe(100);
  });

  it('strips euro symbol: €1,234.56', () => {
    expect(parseAmount('€1,234.56')).toBe(1234.56);
  });

  it('parses space-separated thousands: 1 234,56', () => {
    expect(parseAmount('1 234,56')).toBe(1234.56);
  });

  it('parses integer with no separator', () => {
    expect(parseAmount('500')).toBe(500);
  });

  it('returns null for empty string', () => {
    expect(parseAmount('')).toBeNull();
  });

  it('returns null for non-numeric string', () => {
    expect(parseAmount('abc')).toBeNull();
  });

  it('returns null for just a minus sign', () => {
    expect(parseAmount('-')).toBeNull();
  });
});

describe('parseDateString', () => {
  it('parses ISO format YYYY-MM-DD', () => {
    expect(parseDateString('2026-03-14')).toBe('2026-03-14T12:00:00');
  });

  it('parses DD.MM.YYYY', () => {
    expect(parseDateString('14.03.2026')).toBe('2026-03-14T12:00:00');
  });

  it('parses DD/MM/YYYY', () => {
    expect(parseDateString('14/03/2026')).toBe('2026-03-14T12:00:00');
  });

  it('parses D.M.YYYY (single-digit day and month)', () => {
    expect(parseDateString('3.1.2026')).toBe('2026-01-03T12:00:00');
  });

  it('parses DD-MM-YYYY', () => {
    expect(parseDateString('14-03-2026')).toBe('2026-03-14T12:00:00');
  });

  it('returns null for invalid string', () => {
    expect(parseDateString('invalid')).toBeNull();
  });

  it('returns null for empty string', () => {
    expect(parseDateString('')).toBeNull();
  });

  it('returns null for invalid month', () => {
    expect(parseDateString('2026-13-01')).toBeNull();
  });

  it('returns null for invalid day', () => {
    expect(parseDateString('2026-01-32')).toBeNull();
  });
});
