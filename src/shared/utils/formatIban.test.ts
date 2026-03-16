import { describe, it, expect } from 'vitest';
import { formatIbanSegments } from './formatIban';

describe('formatIbanSegments', () => {
  const SK_IBAN = 'SK1234567890123456789012';

  it('SK IBAN: country+check (chars 0-3) is font-normal', () => {
    const result = formatIbanSegments(SK_IBAN);
    expect(result[0]).toEqual({ text: 'SK12', weight: 'font-normal' });
  });

  it('SK IBAN: bank code (chars 4-7) is font-bold', () => {
    const result = formatIbanSegments(SK_IBAN);
    expect(result[2]).toEqual({ text: '3456', weight: 'font-bold' });
  });

  it('SK IBAN: prefix (chars 8-13) is font-normal', () => {
    const result = formatIbanSegments(SK_IBAN);
    expect(result[4]).toEqual({ text: '7890', weight: 'font-normal' });
    expect(result[6]).toEqual({ text: '12', weight: 'font-normal' });
  });

  it('SK IBAN: group 3 splits at prefix/account boundary (chars 12-13 font-normal, 14-15 font-bold)', () => {
    const result = formatIbanSegments(SK_IBAN);
    expect(result[6]).toEqual({ text: '12', weight: 'font-normal' });
    expect(result[7]).toEqual({ text: '34', weight: 'font-bold' });
  });

  it('SK IBAN: account number (chars 14-23) is font-bold', () => {
    const result = formatIbanSegments(SK_IBAN);
    expect(result[9]).toEqual({ text: '5678', weight: 'font-bold' });
    expect(result[11]).toEqual({ text: '9012', weight: 'font-bold' });
  });

  it('SK IBAN: space separators between groups have empty weight', () => {
    const result = formatIbanSegments(SK_IBAN);
    const spaces = result.filter((s) => s.text === ' ');
    expect(spaces).toHaveLength(5);
    spaces.forEach((s) => expect(s.weight).toBe(''));
  });

  it('non-SK IBAN: all non-space segments are font-bold', () => {
    const result = formatIbanSegments('DE89370400440532013000');
    const nonSpace = result.filter((s) => s.text !== ' ');
    nonSpace.forEach((s) => expect(s.weight).toBe('font-bold'));
  });

  it('non-SK IBAN: space separators between groups have empty weight', () => {
    const result = formatIbanSegments('DE89370400440532013000');
    const spaces = result.filter((s) => s.text === ' ');
    spaces.forEach((s) => expect(s.weight).toBe(''));
  });

  it('normalizes spaces in input', () => {
    const withSpaces = formatIbanSegments('SK12 3456 7890 1234 5678 9012');
    const withoutSpaces = formatIbanSegments('SK1234567890123456789012');
    expect(withSpaces).toEqual(withoutSpaces);
  });

  it('normalizes lowercase input', () => {
    const lower = formatIbanSegments('sk1234567890123456789012');
    const upper = formatIbanSegments('SK1234567890123456789012');
    expect(lower).toEqual(upper);
  });
});
