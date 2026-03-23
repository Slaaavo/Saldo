import { describe, it, expect } from 'vitest';
import { applyRules } from './transformRules';
import type { ImportRule } from '../../../shared/types';
import type { CsvRow, ColumnMapping } from './types';

const emptyMapping: ColumnMapping = {
  date: null,
  amount: null,
  partner: null,
  note: null,
  currency: null,
  fxRate: null,
};

describe('applyRules', () => {
  it('returns rows with defaults when no rules are provided', () => {
    const rows: CsvRow[] = [{ Type: 'CREDIT', Desc: 'foo' }];
    const result = applyRules(rows, [], emptyMapping);
    expect(result).toHaveLength(1);
    expect(result[0].__negateAmount).toBe(false);
    expect(result[0].__overrideDateString).toBeNull();
  });

  it('does not mutate the original rows', () => {
    const rows: CsvRow[] = [{ Type: 'DEBIT' }];
    const rules: ImportRule[] = [
      { type: 'sign_from_column', sortOrder: 1, typeColumn: 'Type', negativeType: 'DEBIT' },
    ];
    applyRules(rows, rules, emptyMapping);
    expect((rows[0] as Record<string, unknown>).__negateAmount).toBeUndefined();
  });

  describe('sign_from_column', () => {
    it('sets __negateAmount to true when value matches negativeType', () => {
      const rows: CsvRow[] = [{ Type: 'DEBIT' }];
      const rules: ImportRule[] = [
        { type: 'sign_from_column', sortOrder: 1, typeColumn: 'Type', negativeType: 'DEBIT' },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      expect(result[0].__negateAmount).toBe(true);
    });

    it('leaves __negateAmount false when value does not match', () => {
      const rows: CsvRow[] = [{ Type: 'CREDIT' }];
      const rules: ImportRule[] = [
        { type: 'sign_from_column', sortOrder: 1, typeColumn: 'Type', negativeType: 'DEBIT' },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      expect(result[0].__negateAmount).toBe(false);
    });

    it('matches case-insensitively ("DEBIT" matches "debit")', () => {
      const rows: CsvRow[] = [{ Type: 'DEBIT' }];
      const rules: ImportRule[] = [
        { type: 'sign_from_column', sortOrder: 1, typeColumn: 'Type', negativeType: 'debit' },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      expect(result[0].__negateAmount).toBe(true);
    });

    it('does not toggle __negateAmount back to false once set', () => {
      const rows: CsvRow[] = [{ Type: 'DEBIT' }];
      const rules: ImportRule[] = [
        { type: 'sign_from_column', sortOrder: 1, typeColumn: 'Type', negativeType: 'DEBIT' },
        { type: 'sign_from_column', sortOrder: 2, typeColumn: 'Type', negativeType: 'CREDIT' },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      expect(result[0].__negateAmount).toBe(true);
    });

    it('silently skips when typeColumn does not exist in row', () => {
      const rows: CsvRow[] = [{ Other: 'DEBIT' }];
      const rules: ImportRule[] = [
        { type: 'sign_from_column', sortOrder: 1, typeColumn: 'Type', negativeType: 'DEBIT' },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      expect(result[0].__negateAmount).toBe(false);
    });
  });

  describe('override_date_from_description', () => {
    it('sets __overrideDateString when condition and date regex both match', () => {
      const rows: CsvRow[] = [{ Desc: 'Booked on 2026-03-14 end' }];
      const rules: ImportRule[] = [
        {
          type: 'override_date_from_description',
          sortOrder: 1,
          descriptionColumn: 'Desc',
          conditionRegex: 'Booked',
          dateRegex: '(\\d{4}-\\d{2}-\\d{2})',
        },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      expect(result[0].__overrideDateString).toBe('2026-03-14T12:00:00');
    });

    it('applies to all rows when conditionRegex is empty string', () => {
      const rows: CsvRow[] = [{ Desc: 'anything 20260101' }, { Desc: 'another row 20261231' }];
      const rules: ImportRule[] = [
        {
          type: 'override_date_from_description',
          sortOrder: 1,
          descriptionColumn: 'Desc',
          conditionRegex: '',
          dateRegex: '(\\d{8})',
        },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      expect(result[0].__overrideDateString).toBe('2026-01-01T12:00:00');
      expect(result[1].__overrideDateString).toBe('2026-12-31T12:00:00');
    });

    it('silently skips rule when conditionRegex is invalid', () => {
      const rows: CsvRow[] = [{ Desc: 'some text' }];
      const rules: ImportRule[] = [
        {
          type: 'override_date_from_description',
          sortOrder: 1,
          descriptionColumn: 'Desc',
          conditionRegex: '[invalid((',
          dateRegex: '(\\d{8})',
        },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      expect(result[0].__overrideDateString).toBeNull();
    });

    it('silently skips rule when dateRegex is invalid', () => {
      const rows: CsvRow[] = [{ Desc: 'some text 20260314' }];
      const rules: ImportRule[] = [
        {
          type: 'override_date_from_description',
          sortOrder: 1,
          descriptionColumn: 'Desc',
          conditionRegex: '',
          dateRegex: '[invalid((',
        },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      expect(result[0].__overrideDateString).toBeNull();
    });

    it('leaves __overrideDateString null when date capture does not produce a valid date', () => {
      const rows: CsvRow[] = [{ Desc: 'bad date 99991399' }];
      const rules: ImportRule[] = [
        {
          type: 'override_date_from_description',
          sortOrder: 1,
          descriptionColumn: 'Desc',
          conditionRegex: '',
          dateRegex: '(\\d{8})',
        },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      // month 13 is invalid
      expect(result[0].__overrideDateString).toBeNull();
    });

    it('leaves __overrideDateString null when condition does not match', () => {
      const rows: CsvRow[] = [{ Desc: 'unrelated text' }];
      const rules: ImportRule[] = [
        {
          type: 'override_date_from_description',
          sortOrder: 1,
          descriptionColumn: 'Desc',
          conditionRegex: 'Booked',
          dateRegex: '(\\d{8})',
        },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      expect(result[0].__overrideDateString).toBeNull();
    });

    it('leaves __overrideDateString null when descriptionColumn is missing from row', () => {
      const rows: CsvRow[] = [{ Other: 'some text 20260314' }];
      const rules: ImportRule[] = [
        {
          type: 'override_date_from_description',
          sortOrder: 1,
          descriptionColumn: 'Desc',
          conditionRegex: '',
          dateRegex: '(\\d{8})',
        },
      ];
      const result = applyRules(rows, rules, emptyMapping);
      expect(result[0].__overrideDateString).toBeNull();
    });
  });
});
