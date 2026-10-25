import { describe, it, expect } from 'vitest'
import { parseRateInput, buildRatePivot } from './fxRate'
import { formatRate } from '../../shared/utils/fxRate'
import type { FxRateRow, Currency } from '../../shared/types'

const makeRow = (mantissa: number, exponent: number, overrides?: Partial<FxRateRow>): FxRateRow => ({
  id: 1,
  date: '2026-01-01',
  fromCurrencyCode: 'EUR',
  toCurrencyCode: 'USD',
  rateMantissa: mantissa,
  rateExponent: exponent,
  isManual: false,
  fetchedAt: '2026-01-01T00:00:00',
  isDirect: false,
  ...overrides,
})

const makeCurrency = (id: number, code: string): Currency => ({
  id,
  code,
  name: code,
  minorUnits: 2,
  isCustom: false,
})

describe('parseRateInput', () => {
  it('parses simple integer', () => {
    expect(parseRateInput('2')).toEqual({ mantissa: 2, exponent: 0 })
  })

  it('parses decimal value', () => {
    expect(parseRateInput('1.0842')).toEqual({ mantissa: 10842, exponent: -4 })
  })

  it('strips trailing zeros', () => {
    expect(parseRateInput('1.50')).toEqual({ mantissa: 15, exponent: -1 })
  })

  it('strips trailing zeros making it integer', () => {
    expect(parseRateInput('1.0')).toEqual({ mantissa: 1, exponent: 0 })
  })

  it('parses large number', () => {
    expect(parseRateInput('1500')).toEqual({ mantissa: 1500, exponent: 0 })
  })

  it('parses small fractional value', () => {
    expect(parseRateInput('0.005')).toEqual({ mantissa: 5, exponent: -3 })
  })

  it('returns null for zero', () => {
    expect(parseRateInput('0')).toBeNull()
  })

  it('returns null for negative value', () => {
    expect(parseRateInput('-1.5')).toBeNull()
  })

  it('returns null for empty string', () => {
    expect(parseRateInput('')).toBeNull()
  })

  it('returns null for non-numeric input', () => {
    expect(parseRateInput('abc')).toBeNull()
  })

  it('returns null for "0.0"', () => {
    expect(parseRateInput('0.0')).toBeNull()
  })

  it('returns null for string with whitespace', () => {
    // Decimal constructor throws on whitespace, so parseRateInput returns null
    expect(parseRateInput('  1.25  ')).toBeNull()
  })

  it('parses value with many decimal places', () => {
    expect(parseRateInput('1.23456789')).toEqual({ mantissa: 123456789, exponent: -8 })
  })

  it('roundtrips with formatRate', () => {
    const input = '1.0842'
    const parsed = parseRateInput(input)!
    expect(formatRate(parsed.mantissa, parsed.exponent)).toBe(input)
  })
})

describe('buildRatePivot', () => {
  const usd = makeCurrency(1, 'USD')
  const gbp = makeCurrency(2, 'GBP')
  const czk = makeCurrency(3, 'CZK')

  const rateUsd0101 = makeRow(10842, -4, { id: 1, date: '2026-01-01', toCurrencyCode: 'USD' })
  const rateGbp0101 = makeRow(8456, -4, { id: 2, date: '2026-01-01', toCurrencyCode: 'GBP' })
  const rateUsd0102 = makeRow(10900, -4, { id: 3, date: '2026-01-02', toCurrencyCode: 'USD' })
  const rateGbp0102 = makeRow(8500, -4, { id: 4, date: '2026-01-02', toCurrencyCode: 'GBP' })

  it('returns empty arrays when rates are empty', () => {
    const result = buildRatePivot([], [usd, gbp])
    expect(result.dates).toEqual([])
    expect(result.targetCurrencies).toEqual([])
    expect(result.rateMap.size).toBe(0)
  })

  it('returns all rates when currencies list is empty (loading state)', () => {
    const rates = [rateUsd0101, rateGbp0101]
    const result = buildRatePivot(rates, [])
    expect(result.dates).toEqual(['2026-01-01'])
    expect(result.targetCurrencies).toEqual(['GBP', 'USD'])
    expect(result.rateMap.size).toBe(2)
  })

  it('sorts dates most recent first', () => {
    const rates = [rateUsd0101, rateUsd0102]
    const result = buildRatePivot(rates, [usd])
    expect(result.dates).toEqual(['2026-01-02', '2026-01-01'])
  })

  it('sorts target currencies alphabetically', () => {
    const rates = [rateUsd0101, rateGbp0101]
    const result = buildRatePivot(rates, [usd, gbp])
    expect(result.targetCurrencies).toEqual(['GBP', 'USD'])
  })

  it('filters out rates for unknown currencies', () => {
    const rateCustom = makeRow(100, 0, { id: 5, date: '2026-01-01', toCurrencyCode: 'GOLD' })
    const rates = [rateUsd0101, rateCustom]
    const result = buildRatePivot(rates, [usd, gbp])
    expect(result.targetCurrencies).toEqual(['USD'])
    expect(result.rateMap.size).toBe(1)
    expect(result.rateMap.has('2026-01-01:GOLD')).toBe(false)
  })

  it('builds correct rateMap keys', () => {
    const rates = [rateUsd0101, rateGbp0101, rateUsd0102, rateGbp0102]
    const result = buildRatePivot(rates, [usd, gbp])
    expect(result.rateMap.get('2026-01-01:USD')).toBe(rateUsd0101)
    expect(result.rateMap.get('2026-01-01:GBP')).toBe(rateGbp0101)
    expect(result.rateMap.get('2026-01-02:USD')).toBe(rateUsd0102)
    expect(result.rateMap.get('2026-01-02:GBP')).toBe(rateGbp0102)
  })

  it('deduplicates dates', () => {
    const rates = [rateUsd0101, rateGbp0101] // same date, different currencies
    const result = buildRatePivot(rates, [usd, gbp])
    expect(result.dates).toEqual(['2026-01-01'])
  })

  it('deduplicates target currencies', () => {
    const rates = [rateUsd0101, rateUsd0102] // same currency, different dates
    const result = buildRatePivot(rates, [usd])
    expect(result.targetCurrencies).toEqual(['USD'])
  })

  it('handles many currencies and dates', () => {
    const rates = [rateUsd0101, rateGbp0101, rateUsd0102, rateGbp0102]
    const result = buildRatePivot(rates, [usd, gbp, czk])
    expect(result.dates).toEqual(['2026-01-02', '2026-01-01'])
    expect(result.targetCurrencies).toEqual(['GBP', 'USD'])
    expect(result.rateMap.size).toBe(4)
  })

  it('returns undefined for missing date:currency combinations', () => {
    const rates = [rateUsd0101] // only USD for 01-01
    const result = buildRatePivot(rates, [usd, gbp])
    expect(result.rateMap.get('2026-01-01:GBP')).toBeUndefined()
  })
})
