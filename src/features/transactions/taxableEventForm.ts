export const VAT_QUICK_FILLS = ['0', '5', '10', '20', '23']

export const validatePrepaidUntil = (expenseDate: string, until: string): boolean => {
  if (!until) return true
  const expenseYear = parseInt(expenseDate.slice(0, 4), 10)
  const untilYear = parseInt(until.slice(0, 4), 10)
  return untilYear > expenseYear
}
