import { describe, it, expect } from 'vitest';
import { computeDashboardMetrics } from './dashboardMetrics';
import type { SnapshotRow } from '../../shared/types';

function makeSnapshot(overrides?: Partial<SnapshotRow>): SnapshotRow {
  return {
    accountId: 1,
    accountName: 'Checking',
    accountType: 'account',
    balanceMinor: 100000,
    currencyCode: 'EUR',
    currencyMinorUnits: 2,
    isCustom: false,
    convertedBalanceMinor: 100000,
    fxRateMissing: false,
    allocatedTotalMinor: 0,
    linkedAllocationsBalanceMinor: 0,
    overAllocationBuckets: [],
    linkedAllocations: [],
    linkedAllocationsFromAssetsMinor: 0,
    isLinkedToAsset: false,
    linkedAssetIds: [],
    ...overrides,
  };
}

function makeBucket(overrides?: Partial<SnapshotRow>): SnapshotRow {
  return makeSnapshot({
    accountId: 10,
    accountName: 'Vacation',
    accountType: 'bucket',
    balanceMinor: 50000,
    convertedBalanceMinor: 50000,
    ...overrides,
  });
}

function makeAsset(overrides?: Partial<SnapshotRow>): SnapshotRow {
  return makeSnapshot({
    accountId: 20,
    accountName: 'House',
    accountType: 'asset',
    balanceMinor: 30000000,
    convertedBalanceMinor: 30000000,
    ...overrides,
  });
}

describe('computeDashboardMetrics', () => {
  it('returns empty arrays and zeros for empty snapshot', () => {
    const result = computeDashboardMetrics([]);
    expect(result.accounts).toEqual([]);
    expect(result.buckets).toEqual([]);
    expect(result.assets).toEqual([]);
    expect(result.hasAssets).toBe(false);
    expect(result.liquidMinor).toBe(0);
    expect(result.leftToSpendMinor).toBe(0);
    expect(result.netWorthMinor).toBe(0);
  });

  it('splits snapshot into accounts, buckets, and assets', () => {
    const acct = makeSnapshot({ accountId: 1 });
    const bucket = makeBucket({ accountId: 10 });
    const asset = makeAsset({ accountId: 20 });

    const result = computeDashboardMetrics([acct, bucket, asset]);
    expect(result.accounts).toEqual([acct]);
    expect(result.buckets).toEqual([bucket]);
    expect(result.assets).toEqual([asset]);
  });

  it('sets hasAssets to true when assets exist', () => {
    const result = computeDashboardMetrics([makeAsset()]);
    expect(result.hasAssets).toBe(true);
  });

  it('sets hasAssets to false when no assets exist', () => {
    const result = computeDashboardMetrics([makeSnapshot()]);
    expect(result.hasAssets).toBe(false);
  });

  it('computes liquidMinor from non-asset-linked accounts only', () => {
    const acct1 = makeSnapshot({
      accountId: 1,
      convertedBalanceMinor: 100000,
      isLinkedToAsset: false,
    });
    const acct2 = makeSnapshot({
      accountId: 2,
      convertedBalanceMinor: 50000,
      isLinkedToAsset: true,
    });

    const result = computeDashboardMetrics([acct1, acct2]);
    expect(result.liquidMinor).toBe(100000);
  });

  it('computes leftToSpendMinor = liquid - buckets + assetAllocationsInBuckets', () => {
    const acct = makeSnapshot({
      accountId: 1,
      convertedBalanceMinor: 200000,
      isLinkedToAsset: false,
    });
    const bucket = makeBucket({
      accountId: 10,
      convertedBalanceMinor: 80000,
      linkedAllocationsFromAssetsMinor: 20000,
    });

    const result = computeDashboardMetrics([acct, bucket]);
    expect(result.leftToSpendMinor).toBe(140000); // 200000 - 80000 + 20000
  });

  it('computes netWorthMinor as all accounts + assets', () => {
    const acct = makeSnapshot({ accountId: 1, convertedBalanceMinor: 100000 });
    const asset = makeAsset({ accountId: 20, convertedBalanceMinor: 500000 });

    const result = computeDashboardMetrics([acct, asset]);
    expect(result.netWorthMinor).toBe(600000);
  });

  it('includes asset-linked accounts in netWorthMinor', () => {
    const linked = makeSnapshot({
      accountId: 1,
      convertedBalanceMinor: 50000,
      isLinkedToAsset: true,
    });
    const asset = makeAsset({ accountId: 20, convertedBalanceMinor: 300000 });

    const result = computeDashboardMetrics([linked, asset]);
    expect(result.netWorthMinor).toBe(350000); // 50000 + 300000
  });

  it('handles multiple accounts, buckets, and assets', () => {
    const acct1 = makeSnapshot({
      accountId: 1,
      convertedBalanceMinor: 100000,
      isLinkedToAsset: false,
    });
    const acct2 = makeSnapshot({
      accountId: 2,
      convertedBalanceMinor: 200000,
      isLinkedToAsset: false,
    });
    const bucket1 = makeBucket({
      accountId: 10,
      convertedBalanceMinor: 30000,
      linkedAllocationsFromAssetsMinor: 0,
    });
    const bucket2 = makeBucket({
      accountId: 11,
      convertedBalanceMinor: 20000,
      linkedAllocationsFromAssetsMinor: 5000,
    });
    const asset = makeAsset({ accountId: 20, convertedBalanceMinor: 1000000 });

    const result = computeDashboardMetrics([acct1, acct2, bucket1, bucket2, asset]);

    expect(result.accounts).toHaveLength(2);
    expect(result.buckets).toHaveLength(2);
    expect(result.assets).toHaveLength(1);
    expect(result.hasAssets).toBe(true);
    expect(result.liquidMinor).toBe(300000); // 100000 + 200000
    expect(result.leftToSpendMinor).toBe(255000); // 300000 - 50000 + 5000
    expect(result.netWorthMinor).toBe(1300000); // 300000 + 1000000
  });
});
