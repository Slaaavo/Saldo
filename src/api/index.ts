import { invoke } from '@tauri-apps/api/core';
import type {
  SnapshotRow,
  EventWithData,
  Currency,
  FxRateRow,
  BucketAllocation,
  OverAllocationWarning,
  DbLocationInfo,
  PickDbFolderResult,
  AccountAssetLink,
} from '../types';

export async function createBalanceUpdate(
  accountId: number,
  amountMinor: number,
  eventDate: string,
  note?: string,
): Promise<number> {
  return invoke('create_balance_update', {
    input: {
      accountId,
      amountMinor,
      eventDate,
      note: note ?? null,
    },
  });
}

export async function getAccountsSnapshot(dateIso: string): Promise<SnapshotRow[]> {
  return invoke('get_accounts_snapshot', { dateIso });
}

export async function listEvents(
  accountId?: number,
  beforeDate?: string,
): Promise<EventWithData[]> {
  return invoke('list_events', {
    filter: {
      accountId: accountId ?? null,
      beforeDate: beforeDate ?? null,
    },
  });
}

export async function createAccount(
  name: string,
  currencyId: number,
  initialBalanceMinor?: number,
  accountType?: string,
  pricePerUnit?: string,
  linkedAssetIds?: number[],
): Promise<number> {
  return invoke('create_account', {
    input: {
      name,
      currencyId,
      initialBalanceMinor: initialBalanceMinor ?? null,
      accountType: accountType ?? null,
      pricePerUnit: pricePerUnit ?? null,
      linkedAssetIds: linkedAssetIds ?? null,
    },
  });
}

export async function updateAccount(accountId: number, name: string): Promise<void> {
  return invoke('update_account', {
    input: { accountId, name },
  });
}

export async function deleteAccount(accountId: number): Promise<void> {
  return invoke('delete_account', { accountId });
}

export async function updateEvent(
  eventId: number,
  amountMinor: number,
  eventDate: string,
  note?: string,
): Promise<void> {
  return invoke('update_event', {
    input: {
      eventId,
      amountMinor,
      eventDate,
      note: note ?? null,
    },
  });
}

export async function deleteEvent(eventId: number): Promise<void> {
  return invoke('delete_event', { eventId });
}

export async function bulkCreateBalanceUpdates(
  entries: { accountId: number; amountMinor: number }[],
  eventDate: string,
  note?: string,
): Promise<number[]> {
  return invoke('bulk_create_balance_updates', {
    input: { entries, eventDate, note: note ?? null },
  });
}

export async function listCurrencies(includeCustom?: boolean): Promise<Currency[]> {
  return invoke('list_currencies', { includeCustom: includeCustom ?? null });
}

export async function getConsolidationCurrency(): Promise<Currency> {
  return invoke('get_consolidation_currency');
}

export async function setConsolidationCurrency(currencyId: number): Promise<void> {
  return invoke('set_consolidation_currency', { input: { currencyId } });
}

export async function setFxRateManual(
  fromCurrencyId: number,
  toCurrencyId: number,
  date: string,
  rateMantissa: number,
  rateExponent: number,
): Promise<void> {
  return invoke('set_fx_rate_manual', {
    input: { fromCurrencyId, toCurrencyId, date, rateMantissa, rateExponent },
  });
}

export async function listFxRates(date?: string): Promise<FxRateRow[]> {
  return invoke('list_fx_rates', { date: date ?? null });
}

export async function fetchFxRates(date?: string, force?: boolean): Promise<FxRateRow[]> {
  return invoke('fetch_fx_rates', { dateIso: date ?? null, force: force ?? null });
}

export async function getMissingRateDates(): Promise<string[]> {
  return invoke('get_missing_rate_dates');
}

export async function getAppSetting(key: string): Promise<string | null> {
  return invoke('get_app_setting', { key });
}

export async function createBucketAllocation(
  bucketId: number,
  sourceAccountId: number,
  amountMinor: number,
  effectiveDate: string,
): Promise<number> {
  return invoke('create_bucket_allocation', {
    input: { bucketId, sourceAccountId, amountMinor, effectiveDate },
  });
}

export async function listBucketAllocations(
  bucketId: number,
  asOfDate: string,
): Promise<BucketAllocation[]> {
  return invoke('list_bucket_allocations', { bucketId, asOfDate });
}

export async function getAccountAllocatedTotal(
  sourceAccountId: number,
  asOfDate: string,
): Promise<number> {
  return invoke('get_account_allocated_total', { sourceAccountId, asOfDate });
}

export async function checkOverAllocation(
  sourceAccountId: number,
  asOfDate: string,
): Promise<OverAllocationWarning | null> {
  return invoke('check_over_allocation', { sourceAccountId, asOfDate });
}

export async function setAppSetting(key: string, value: string): Promise<void> {
  return invoke('set_app_setting', { key, value });
}

export async function updateSortOrder(
  entries: { accountId: number; sortOrder: number }[],
): Promise<void> {
  return invoke('update_sort_order', { input: { entries } });
}

export async function enterDemoMode(): Promise<void> {
  return invoke('enter_demo_mode');
}

export async function exitDemoMode(): Promise<void> {
  return invoke('exit_demo_mode');
}

export async function isDemoMode(): Promise<boolean> {
  return invoke('is_demo_mode');
}

export async function getDbLocation(): Promise<DbLocationInfo> {
  return invoke('get_db_location');
}

export async function pickDbFolder(): Promise<PickDbFolderResult | null> {
  return invoke('pick_db_folder');
}

export async function changeDbLocation(folder: string, action: string): Promise<void> {
  return invoke('change_db_location', { folder, action });
}

export async function resetDbLocation(action: string): Promise<void> {
  return invoke('reset_db_location', { action });
}

export async function checkDefaultDb(): Promise<boolean> {
  return invoke('check_default_db');
}

export async function createCustomUnit(name: string, minorUnits: number): Promise<number> {
  return invoke('create_custom_unit', { input: { name, minorUnits } });
}

export async function listCustomUnits(): Promise<Currency[]> {
  return invoke('list_custom_units');
}

export async function updateCustomUnit(currencyId: number, name: string): Promise<void> {
  return invoke('update_custom_unit', { input: { currencyId, name } });
}

export async function updateAssetValue(
  accountId: number,
  amountMinor: number | null,
  pricePerUnit: string | null,
  eventDate: string,
  note: string | null,
): Promise<void> {
  return invoke('update_asset_value', {
    input: { accountId, amountMinor, pricePerUnit, eventDate, note },
  });
}

export async function listAccountAssetLinks(accountId?: number): Promise<AccountAssetLink[]> {
  return invoke('list_account_asset_links', { accountId: accountId ?? null });
}

export async function setAccountAssetLinks(accountId: number, assetIds: number[]): Promise<void> {
  return invoke('set_account_asset_links', { input: { accountId, assetIds } });
}
