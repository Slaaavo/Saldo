export interface Currency {
  id: number;
  code: string;
  name: string;
  minorUnits: number;
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
  convertedBalanceMinor: number;
  fxRateMissing: boolean;
  allocatedTotalMinor: number;
  linkedAllocationsBalanceMinor: number;
  overAllocationBuckets: AllocationDetail[];
  linkedAllocations: BucketAllocation[];
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

export interface AppError {
  code: string;
  message: string;
}
