import type { SnapshotRow } from '../../shared/types';

export interface DashboardMetrics {
  accounts: SnapshotRow[];
  buckets: SnapshotRow[];
  assets: SnapshotRow[];
  hasAssets: boolean;
  liquidMinor: number;
  leftToSpendMinor: number;
  netWorthMinor: number;
}

export function computeDashboardMetrics(snapshot: SnapshotRow[]): DashboardMetrics {
  const accounts = snapshot.filter((r) => r.accountType === 'account');
  const buckets = snapshot.filter((r) => r.accountType === 'bucket');
  const assets = snapshot.filter((r) => r.accountType === 'asset');
  const hasAssets = assets.length > 0;
  const liquidMinor = accounts
    .filter((r) => !r.isLinkedToAsset)
    .reduce((sum, r) => sum + r.convertedBalanceMinor, 0);
  const allAccountsMinor = accounts.reduce((sum, r) => sum + r.convertedBalanceMinor, 0);
  const bucketsMinor = buckets.reduce((sum, r) => sum + r.convertedBalanceMinor, 0);
  const assetTotalMinor = assets.reduce((sum, r) => sum + r.convertedBalanceMinor, 0);
  const assetAllocationsInBuckets = buckets.reduce(
    (sum, r) => sum + r.linkedAllocationsFromAssetsMinor,
    0,
  );
  const leftToSpendMinor = liquidMinor - bucketsMinor + assetAllocationsInBuckets;
  const netWorthMinor = allAccountsMinor + assetTotalMinor;

  return { accounts, buckets, assets, hasAssets, liquidMinor, leftToSpendMinor, netWorthMinor };
}
