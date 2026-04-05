import { defaultNumberFormat, type NumberFormatConfig } from '../config/numberFormat'

export const formatAmount = (amountMinor: number, minorUnits: number = 2, config: NumberFormatConfig = defaultNumberFormat, currencyCode?: string): string => {
  const isNegative = amountMinor < 0
  const abs = Math.abs(amountMinor)
  const divisor = Math.pow(10, minorUnits)
  const integerPart = Math.floor(abs / divisor)
  const fractionalPart = abs % divisor

  // Insert thousands separator
  const intStr = integerPart.toString()
  let withSeparators = ''
  for (let i = 0; i < intStr.length; i++) {
    if (i > 0 && (intStr.length - i) % 3 === 0) {
      withSeparators += config.thousandsSeparator
    }
    withSeparators += intStr[i]
  }

  let numberStr = withSeparators
  if (minorUnits > 0) {
    numberStr += config.decimalSeparator + fractionalPart.toString().padStart(minorUnits, '0')
  }

  let result: string
  const symbol = currencyCode ?? config.currencySymbol
  if (config.currencyPosition === 'left') {
    result = `${symbol} ${numberStr}`
  } else {
    result = `${numberStr} ${symbol}`
  }

  return isNegative ? `-${result}` : result
}

export const formatDate = (isoDatetime: string): string => {
  return isoDatetime.substring(0, 10)
}

export const toEndOfDay = (dateStr: string): string => {
  return `${dateStr}T23:59:59`
}

export const todayIso = (): string => {
  const now = new Date()
  const y = now.getFullYear()
  const m = String(now.getMonth() + 1).padStart(2, '0')
  const d = String(now.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}

export const toMinorUnits = (decimalStr: string, minorUnits: number): number => {
  return Math.round(parseFloat(decimalStr) * Math.pow(10, minorUnits))
}

export const fromMinorUnits = (amountMinor: number, minorUnits: number): string => {
  return (amountMinor / Math.pow(10, minorUnits)).toFixed(minorUnits)
}

export const getMinorUnitsStep = (minorUnits: number): string => {
  return minorUnits === 0 ? '1' : '0.' + '0'.repeat(minorUnits - 1) + '1'
}

export const formatDisplayDate = (dateStr: string, locale = 'en-GB'): string => {
  const [y, m, d] = dateStr.split('-').map(Number)
  const date = new Date(y, m - 1, d)
  return date.toLocaleDateString(locale, {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  })
}
