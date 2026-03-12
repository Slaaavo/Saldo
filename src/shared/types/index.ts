export interface AccountAssetLink {
  id: number;
  accountId: number;
  accountName: string;
  assetId: number;
  assetName: string;
}

export type ModalState =
  | { type: 'none' }
  | { type: 'createBalanceUpdate'; preselectedAccountId?: number }
  | { type: 'editBalanceUpdate'; event: EventWithData }
  | { type: 'createAccount'; accountType?: 'account' | 'bucket' }
  | { type: 'createAsset' }
  | { type: 'renameAccount'; accountId: number; currentName: string }
  | {
      type: 'confirmDeleteAccount';
      accountId: number;
      name: string;
      accountType?: 'account' | 'bucket' | 'asset';
    }
  | { type: 'confirmDeleteEvent'; eventId: number }
  | { type: 'bulkUpdateBalance' }
  | { type: 'fetchFxRatePrompt'; date: string }
  | { type: 'reorderAccounts' }
  | { type: 'reorderBuckets' }
  | { type: 'reorderAssets' }
  | { type: 'confirmSwitchDb'; folder: string }
  | { type: 'dbLocationChoice'; folder: string; isReset: boolean }
  | { type: 'confirmResetDbLocation' }
  | {
      type: 'updateAssetValue';
      accountId: number;
      accountName: string;
      currencyCode: string;
      currencyMinorUnits: number;
      isCustomUnit: boolean;
      balanceMinor: number;
    }
  | { type: 'manageLinkedAssets'; accountId: number; accountName: string };

export interface Currency {
  id: number;
  code: string;
  name: string;
  minorUnits: number;
  isCustom: boolean;
}

export interface Account {
  id: number;
  name: string;
  currencyId: number;
  createdAt: string;
}

export interface EventWithData {
  id: number;
  accountId: number;
  accountName: string;
  accountType: string;
  eventType: string;
  eventDate: string;
  amountMinor: number;
  note: string | null;
  createdAt: string;
  currencyCode: string;
  currencyMinorUnits: number;
}

export interface AllocationDetail {
  bucketId: number;
  bucketName: string;
  amountMinor: number;
}

export interface BucketAllocation {
  id: number;
  bucketId: number;
  sourceAccountId: number;
  sourceAccountName: string;
  sourceAccountType: string;
  sourceCurrencyId: number;
  sourceCurrencyCode: string;
  sourceCurrencyMinorUnits: number;
  amountMinor: number;
  effectiveDate: string;
}

export interface OverAllocationWarning {
  sourceAccountId: number;
  sourceAccountName: string;
  currencyCode: string;
  currencyMinorUnits: number;
  balanceMinor: number;
  totalAllocatedMinor: number;
  overAllocationMinor: number;
  allocations: AllocationDetail[];
}

export interface SnapshotRow {
  accountId: number;
  accountName: string;
  accountType: string;
  balanceMinor: number;
  currencyCode: string;
  currencyMinorUnits: number;
  isCustom: boolean;
  convertedBalanceMinor: number;
  fxRateMissing: boolean;
  allocatedTotalMinor: number;
  linkedAllocationsBalanceMinor: number;
  overAllocationBuckets: AllocationDetail[];
  linkedAllocations: BucketAllocation[];
  linkedAllocationsFromAssetsMinor: number;
  isLinkedToAsset: boolean;
  linkedAssetIds: number[];
}

export interface FxRateRow {
  id: number;
  date: string;
  fromCurrencyCode: string;
  toCurrencyCode: string;
  rateMantissa: number;
  rateExponent: number;
  isManual: boolean;
  fetchedAt: string;
}

export interface DbLocationInfo {
  currentPath: string;
  isDefault: boolean;
  isDemoMode: boolean;
  fallbackWarning: boolean;
}

export interface PickDbFolderResult {
  folder: string;
  dbExists: boolean;
}

export interface AppError {
  code: string;
  message: string;
}
