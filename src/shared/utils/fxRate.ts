import Decimal from 'decimal.js'

export const formatRate = (mantissa: number, exponent: number): string => new Decimal(`${mantissa}e${exponent}`).toString()
