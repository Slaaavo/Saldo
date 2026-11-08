import type { MonthKey } from '@/shared/types'

const MONTH_KEY = /^\d{4}-(0[1-9]|1[0-2])$/

export const isMonthKey = (text: string): text is MonthKey => MONTH_KEY.test(text)

/** `month` is 1 to 12. */
export const monthKey = (year: number, month: number): MonthKey => {
  const text = `${String(year).padStart(4, '0')}-${String(month).padStart(2, '0')}`
  if (!isMonthKey(text)) throw new RangeError(`${year}-${month} is not a month`)
  return text
}

export const yearOf = (key: MonthKey): number => Number(key.slice(0, 4))

export const monthsOfYears = (fromYear: number, toYear: number): MonthKey[] => {
  const months: MonthKey[] = []
  for (let year = fromYear; year <= toYear; year++) {
    for (let month = 1; month <= 12; month++) months.push(monthKey(year, month))
  }
  return months
}

const formatters = new Map<string, Intl.DateTimeFormat>()

// Intl's short month-and-year pattern gives Slovak "11/2026", so the name is formatted alone and the year appended.
export const monthName = (key: MonthKey, language: string, width: 'short' | 'long' = 'long'): string => {
  const id = `${language}:${width}`
  let formatter = formatters.get(id)
  if (!formatter) {
    formatter = new Intl.DateTimeFormat(language, { month: width })
    formatters.set(id, formatter)
  }
  return formatter.format(new Date(yearOf(key), Number(key.slice(5, 7)) - 1, 1))
}

/** "Nov 2026" when short, "November 2026" when long. */
export const formatMonth = (key: MonthKey, language: string, width: 'short' | 'long'): string => `${monthName(key, language, width)} ${yearOf(key)}`
