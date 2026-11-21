import type { BrokenCheck } from '../types'

export const hasErrorCode = (err: unknown, code: string): boolean => {
  return typeof err === 'object' && err !== null && 'code' in err && (err as { code: unknown }).code === code
}

export const extractErrorMessage = (err: unknown): string => {
  if (typeof err === 'object' && err !== null && 'message' in err && typeof (err as { message: unknown }).message === 'string') {
    return (err as { message: string }).message
  }
  return String(err)
}

export interface BreakingChecksError {
  code: 'BREAKS_VERIFIED_CHECKS'
  message: string
  checks: BrokenCheck[]
}

const isBrokenCheck = (value: unknown): value is BrokenCheck => {
  if (typeof value !== 'object' || value === null) return false
  const check = value as Record<string, unknown>
  return (
    typeof check.accountId === 'number' &&
    typeof check.accountName === 'string' &&
    typeof check.day === 'string' &&
    typeof check.differenceMinor === 'number' &&
    typeof check.currencyCode === 'string' &&
    typeof check.currencyMinorUnits === 'number'
  )
}

export const isBreakingChecksError = (err: unknown): err is BreakingChecksError => {
  if (!hasErrorCode(err, 'BREAKS_VERIFIED_CHECKS')) return false
  const { message, checks } = err as { message: unknown; checks: unknown }
  return typeof message === 'string' && Array.isArray(checks) && checks.every(isBrokenCheck)
}
