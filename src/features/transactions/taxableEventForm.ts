import { pctToBps } from '../../shared/utils/format'

export const VAT_QUICK_FILLS = ['0', '5', '10', '20', '23']

export const showsPrivateUse = (vatRate: string, vatReclaimablePct: string, reclaimedVat: boolean): boolean => {
  const reclaimable = pctToBps(vatReclaimablePct) ?? 0
  const someVatComesBack = reclaimedVat && (pctToBps(vatRate) ?? 0) > 0 && reclaimable > 0
  const someVatIsLost = reclaimable < 10000
  return someVatComesBack && someVatIsLost
}

export const validatePrepaidUntil = (expenseDate: string, until: string): boolean => {
  if (!until) return true
  const expenseYear = parseInt(expenseDate.slice(0, 4), 10)
  const untilYear = parseInt(until.slice(0, 4), 10)
  return untilYear > expenseYear
}
