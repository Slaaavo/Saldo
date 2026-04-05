import Decimal from 'decimal.js'
import type { FxRateRow } from '../../shared/types'

export const formatPrice = (r: FxRateRow): string => {
  try {
    const rateDecimal = new Decimal(`${r.rateMantissa}e${r.rateExponent}`)
    if (r.isDirect) {
      return rateDecimal.toSignificantDigits(6).toString()
    }
    return new Decimal(1).div(rateDecimal).toSignificantDigits(6).toString()
  } catch {
    return '—'
  }
}

export const parsePriceAsRate = (input: string): { mantissa: number; exponent: number } | null => {
  try {
    const price = new Decimal(input)
    if (price.isZero() || price.isNegative()) return null
    // Store the price directly — limit to 15 significant digits so the mantissa fits in
    // a JS safe integer and a Rust i64.
    const priceDecimal = price.toSignificantDigits(15)
    let str = priceDecimal.toFixed()
    if (str.includes('.')) {
      str = str.replace(/0+$/, '').replace(/\.$/, '')
    }
    const parts = str.split('.')
    const mantissa = parseInt(parts.join(''), 10)
    const exponent = parts[1] ? -parts[1].length : 0
    if (!Number.isFinite(mantissa) || mantissa === 0) return null
    return { mantissa, exponent }
  } catch {
    return null
  }
}
