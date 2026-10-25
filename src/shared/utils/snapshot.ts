import type { AsOf, Snapshot, SnapshotRow, Valuation } from '../types'

export const LIVE: AsOf = { kind: 'live' }

export const EMPTY_SNAPSHOT: Snapshot = { rows: [], metrics: [], rateWarnings: [] }

export const valueOf = (valuation: Valuation): number | null => (valuation.kind === 'noRate' ? null : valuation.valueMinor)

export const groupRowsByKind = (rows: SnapshotRow[]) => ({
  accounts: rows.filter((r) => r.accountType === 'account'),
  buckets: rows.filter((r) => r.accountType === 'bucket'),
  assets: rows.filter((r) => r.accountType === 'asset'),
})
