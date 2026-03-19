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
  | {
      type: 'editAccount';
      accountId: number;
      currentName: string;
      accountType: string;
      currentIban?: string | null;
    }
  | {
      type: 'confirmDeleteAccount';
      accountId: number;
      name: string;
      accountType?: 'account' | 'bucket' | 'asset';
    }
  | { type: 'confirmDeleteEvent'; eventId: number }
  | { type: 'confirmDeleteTransferEvent'; eventId: number; linkedEventId: number }
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
  | { type: 'manageLinkedAssets'; accountId: number; accountName: string }
  | { type: 'csvImport' };

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
  counterpartAccountId: number | null;
  counterpartAccountName: string | null;
  bucketId: number | null;
  bucketName: string | null;
  originalCurrencyId: number | null;
  originalCurrencyCode: string | null;
  originalAmountMinor: number | null;
  originalCurrencyMinorUnits: number | null;
  fxRateMantissa: number | null;
  fxRateExponent: number | null;
  linkedEventId: number | null;
  splitGroupId: number | null;
  splitGroupNote: string | null;
}

export interface ListEventsResult {
  events: EventWithData[];
  totalCount: number;
}

export interface BucketLink {
  id: number;
  eventId: number;
  sourceAccountId: number;
  sourceAccountName: string;
  sourceCurrencyId: number;
  sourceCurrencyCode: string;
  sourceCurrencyMinorUnits: number;
}

export interface PartnerAccount {
  id: number;
  name: string;
  iban: string | null;
  currencyCode: string;
  createdAt: string;
}

export interface SnapshotRow {
  accountId: number;
  accountName: string;
  accountType: string;
  iban: string | null;
  balanceMinor: number;
  currencyCode: string;
  currencyMinorUnits: number;
  isCustom: boolean;
  convertedBalanceMinor: number;
  fxRateMissing: boolean;
  isLinkedToAsset: boolean;
  linkedAssetIds: number[];
  isBucketLinked: boolean;
  bucketLinks: BucketLink[];
  linkedBalanceMinor: number;
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
