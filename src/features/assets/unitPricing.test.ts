import { describe, it, expect } from 'vitest'
import { computeUnitTotalMinor, parsePositivePrice } from './unitPricing'

describe('computeUnitTotalMinor', () => {
  it('rounds half up where float math falls just below the half (0.1 x 0.35 = 0.035 -> 0.04)', () => {
    // quantity 0.1 at 1 decimal = 1 minor unit; EUR has 2 decimals
    expect(computeUnitTotalMinor(1, 1, '0.35', 2)).toBe(4)
  })

  it('multiplies quantity by price for whole units', () => {
    // 2.50 units (250 minor at 2 decimals) x 10.20 = 25.50 -> 2550 minor
    expect(computeUnitTotalMinor(250, 2, '10.2', 2)).toBe(2550)
  })

  it('handles units with more decimals than the consolidation currency', () => {
    // 0.12345678 BTC (8 decimals) x 50000 = 6172.839 -> 617284 minor at 2 decimals
    expect(computeUnitTotalMinor(12345678, 8, '50000', 2)).toBe(617284)
  })

  it('handles zero-decimal units', () => {
    expect(computeUnitTotalMinor(3, 0, '1.5', 2)).toBe(450)
  })

  it('returns 0 for a zero quantity', () => {
    expect(computeUnitTotalMinor(0, 2, '10', 2)).toBe(0)
  })

  it.each(['', '0', '-1', 'abc', 'Infinity'])('returns null for the invalid price %j', (price) => {
    expect(computeUnitTotalMinor(100, 2, price, 2)).toBeNull()
  })
})

describe('parsePositivePrice', () => {
  it('returns the price as a plain decimal string', () => {
    expect(parsePositivePrice('0.005')).toBe('0.005')
  })

  it.each(['', '0', '-3', 'abc'])('returns null for %j', (input) => {
    expect(parsePositivePrice(input)).toBeNull()
  })
})
