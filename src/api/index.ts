import { invoke } from '@tauri-apps/api/core';
import type {
  SnapshotRow,
  EventWithData,
  Currency,
  FxRateRow,
  BucketAllocation,
  OverAllocationWarning,
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
): Promise<number> {
  return invoke('create_account', {
    input: {
      name,
      currencyId,
      initialBalanceMinor: initialBalanceMinor ?? null,
      accountType: accountType ?? null,
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

export async function listCurrencies(): Promise<Currency[]> {
  return invoke('list_currencies');
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
