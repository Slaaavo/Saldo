import type { SnapshotRow } from '@/shared/types'

interface AccountCardEquity {
  equityMinor: number
  equityLinkedRows: SnapshotRow[]
}

const useAccountCardEquity = (row: SnapshotRow, allAccounts: SnapshotRow[] | undefined): AccountCardEquity | null => {
  if (row.accountType !== 'asset' || row.equityMinor === null || !allAccounts) return null
  return { equityMinor: row.equityMinor, equityLinkedRows: allAccounts.filter((a) => row.linkedAssetIds.includes(a.accountId)) }
}

export { useAccountCardEquity }
