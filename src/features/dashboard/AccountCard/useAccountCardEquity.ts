import type { SnapshotRow, Currency } from '@/shared/types'

const useAccountCardEquity = (
  row: SnapshotRow,
  allAccounts: SnapshotRow[] | undefined,
  consolidationCurrency: Currency | null | undefined,
): { hasEquityTooltip: boolean; equityLinkedRows: SnapshotRow[]; equityMinor: number } => {
  const hasEquityTooltip = row.accountType === 'asset' && row.linkedAssetIds.length > 0 && !!allAccounts && !!consolidationCurrency
  const equityLinkedRows = hasEquityTooltip ? allAccounts!.filter((a) => row.linkedAssetIds.includes(a.accountId)) : []
  const equityMinor = hasEquityTooltip ? row.convertedBalanceMinor + equityLinkedRows.reduce((sum, a) => sum + a.convertedBalanceMinor, 0) : 0

  return { hasEquityTooltip, equityLinkedRows, equityMinor }
}

export { useAccountCardEquity }
