import { describe, it, expect } from 'vitest'
import { formatMonth, isMonthKey, monthKey, monthName, monthsOfYears } from './month'

describe('isMonthKey', () => {
  it('accepts a zero-padded year and month', () => {
    expect(isMonthKey('2026-11')).toBe(true)
    expect(isMonthKey('2026-01')).toBe(true)
  })

  it.each(['2026-1', '2026-13', '2026-00', '26-11', '2026-11-01', ' 2026-11'])('refuses %j', (text) => {
    expect(isMonthKey(text)).toBe(false)
  })
})

describe('monthsOfYears', () => {
  it('lists every month of the years, January to December', () => {
    const months = monthsOfYears(2025, 2026)

    expect(months).toHaveLength(24)
    expect(months.slice(0, 2)).toEqual(['2025-01', '2025-02'])
    expect(months.slice(-2)).toEqual(['2026-11', '2026-12'])
  })
})

describe('formatMonth', () => {
  it('names a month the way the table headers and cell labels read in English', () => {
    expect(formatMonth(monthKey(2026, 11), 'en', 'short')).toBe('Nov 2026')
    expect(formatMonth(monthKey(2026, 9), 'en', 'short')).toBe('Sep 2026')
    expect(formatMonth(monthKey(2026, 11), 'en', 'long')).toBe('November 2026')
  })

  it('keeps the month a word in Slovak', () => {
    expect(formatMonth(monthKey(2026, 11), 'sk', 'short')).toBe('nov 2026')
    expect(formatMonth(monthKey(2026, 11), 'sk', 'long')).toBe('november 2026')
  })

  it('names the month alone for a sentence', () => {
    expect(monthName(monthKey(2026, 11), 'en')).toBe('November')
  })
})
