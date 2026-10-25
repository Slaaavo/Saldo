import { describe, it, expect } from 'vitest'
import { formatRate } from './fxRate'

describe('formatRate', () => {
  it('formats integer rate (exponent 0)', () => {
    expect(formatRate(2, 0)).toBe('2')
  })

  it('formats rate with negative exponent', () => {
    expect(formatRate(10842, -4)).toBe('1.0842')
  })

  it('formats rate with positive exponent', () => {
    expect(formatRate(15, 2)).toBe('1500')
  })

  it('formats rate of 1', () => {
    expect(formatRate(1, 0)).toBe('1')
  })

  it('formats small fractional rate', () => {
    expect(formatRate(5, -3)).toBe('0.005')
  })

  it('formats large mantissa with large negative exponent', () => {
    expect(formatRate(123456789, -8)).toBe('1.23456789')
  })

  it('formats a custom-unit price quoted in the consolidation currency', () => {
    expect(formatRate(2340, -2)).toBe('23.4')
  })
})
