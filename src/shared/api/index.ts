import { invoke } from '@tauri-apps/api/core';
import type {
  SnapshotRow,
  ListEventsResult,
  Currency,
  FxRateRow,
  BucketLink,
  DbLocationInfo,
  PickDbFolderResult,
  AccountAssetLink,
  PartnerAccount,
  EventWithData,
  ImportProfileRow,
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

export interface ListEventsFilter {
  accountId?: number;
  accountIds?: number[];
  bucketIds?: number[];
  beforeDate?: string;
  fromDate?: string;
  limit?: number;
  eventTypes?: string[];
}

export async function listEvents(filter?: ListEventsFilter): Promise<ListEventsResult> {
  return invoke('list_events', {
    filter: {
      accountId: filter?.accountId ?? null,
      accountIds: filter?.accountIds ?? null,
      bucketIds: filter?.bucketIds ?? null,
      beforeDate: filter?.beforeDate ?? null,
      fromDate: filter?.fromDate ?? null,
      limit: filter?.limit ?? null,
      eventTypes: filter?.eventTypes ?? null,
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
  iban?: string,
): Promise<number> {
  return invoke('create_account', {
    input: {
      name,
      currencyId,
      initialBalanceMinor: initialBalanceMinor ?? null,
      accountType: accountType ?? null,
      pricePerUnit: pricePerUnit ?? null,
      linkedAssetIds: linkedAssetIds ?? null,
      iban: iban ?? null,
    },
  });
}

export async function updateAccount(
  accountId: number,
  name: string,
  iban?: string | null,
): Promise<void> {
  return invoke('update_account', {
    input: { accountId, name, iban: iban ?? null },
  });
}

export async function createPartnerAccount(
  name: string,
  currencyId: number,
  iban: string,
): Promise<number> {
  return invoke('create_partner_account', { input: { name, currencyId, iban } });
}

export async function listPartnerAccounts(): Promise<PartnerAccount[]> {
  return invoke('list_partner_accounts');
}

export async function updatePartnerAccount(
  accountId: number,
  name: string,
  iban: string,
): Promise<void> {
  return invoke('update_partner_account', { input: { accountId, name, iban } });
}

export async function deletePartnerAccount(accountId: number): Promise<void> {
  return invoke('delete_partner_account', { accountId });
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

export async function getEventById(eventId: number): Promise<EventWithData | null> {
  return invoke('get_event_by_id', { eventId });
}

export async function updateTransfer(payload: {
  fromEventId: number;
  toEventId: number;
  fromDate: string;
  toDate: string;
  amountMinor: number;
  toAmountMinor: number;
  note: string | null;
  originalCurrencyId: number | null;
  fxRateMantissa: number | null;
  fxRateExponent: number | null;
}): Promise<void> {
  return invoke('update_transfer', { input: payload });
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
  isDirect: boolean,
): Promise<void> {
  return invoke('set_fx_rate_manual', {
    input: { fromCurrencyId, toCurrencyId, date, rateMantissa, rateExponent, isDirect },
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

export async function setAppSetting(key: string, value: string): Promise<void> {
  return invoke('set_app_setting', { key, value });
}

export async function createBucketBalanceUpdate(
  accountId: number,
  amountMinor: number,
  eventDate: string,
  note: string | null,
  linkedAccountIds: number[],
): Promise<number> {
  return invoke<number>('create_bucket_balance_update', {
    input: { accountId, amountMinor, eventDate, note, linkedAccountIds },
  });
}

export async function updateBucketBalanceUpdate(
  eventId: number,
  amountMinor: number,
  eventDate: string,
  note: string | null,
  linkedAccountIds: number[],
): Promise<void> {
  return invoke<void>('update_bucket_balance_update', {
    input: { eventId, amountMinor, eventDate, note, linkedAccountIds },
  });
}

export async function listLinksForEvent(eventId: number): Promise<BucketLink[]> {
  return invoke<BucketLink[]>('list_links_for_event', { eventId });
}

export async function getLatestBucketLinks(
  bucketAccountId: number,
  asOfDate: string,
): Promise<BucketLink[]> {
  return invoke<BucketLink[]>('get_latest_bucket_links', { bucketAccountId, asOfDate });
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

export async function getBulkUpdateExclusions(): Promise<number[]> {
  return invoke('get_bulk_update_exclusions');
}

export async function setBulkUpdateExclusions(accountIds: number[]): Promise<void> {
  return invoke('set_bulk_update_exclusions', { accountIds });
}

export async function createCashflow(input: {
  accountId: number;
  amountMinor: number;
  eventDate: string;
  note?: string;
  counterpartAccountId?: number;
  bucketId?: number;
  originalCurrencyId?: number;
  originalAmountMinor?: number;
  fxRateMantissa?: number;
  fxRateExponent?: number;
}): Promise<number> {
  return invoke('create_cashflow', {
    input: {
      accountId: input.accountId,
      amountMinor: input.amountMinor,
      eventDate: input.eventDate,
      note: input.note ?? null,
      counterpartAccountId: input.counterpartAccountId ?? null,
      bucketId: input.bucketId ?? null,
      originalCurrencyId: input.originalCurrencyId ?? null,
      originalAmountMinor: input.originalAmountMinor ?? null,
      fxRateMantissa: input.fxRateMantissa ?? null,
      fxRateExponent: input.fxRateExponent ?? null,
    },
  });
}

export async function bulkCreateCashflows(input: {
  entries: Array<{
    accountId: number;
    amountMinor: number;
    eventDate: string;
    note?: string;
    counterpartAccountId?: number;
    bucketId?: number;
    originalCurrencyId?: number;
    originalAmountMinor?: number;
    fxRateMantissa?: number;
    fxRateExponent?: number;
  }>;
}): Promise<number[]> {
  return invoke('bulk_create_cashflows', {
    input: {
      entries: input.entries.map((e) => ({
        accountId: e.accountId,
        amountMinor: e.amountMinor,
        eventDate: e.eventDate,
        note: e.note ?? null,
        counterpartAccountId: e.counterpartAccountId ?? null,
        bucketId: e.bucketId ?? null,
        originalCurrencyId: e.originalCurrencyId ?? null,
        originalAmountMinor: e.originalAmountMinor ?? null,
        fxRateMantissa: e.fxRateMantissa ?? null,
        fxRateExponent: e.fxRateExponent ?? null,
      })),
    },
  });
}

export interface SplitGroupLeg {
  amountMinor: number;
  eventDate: string;
  note: string | null;
  counterpartAccountId: number | null;
  bucketId: number | null;
  originalCurrencyId: number | null;
  originalAmountMinor: number | null;
  fxRateMantissa: number | null;
  fxRateExponent: number | null;
}

export async function createSplitGroup(input: {
  accountId: number;
  groupNote: string | null;
  legs: SplitGroupLeg[];
}): Promise<number> {
  return invoke('create_split_group', { input });
}

export async function updateSplitGroupDate(splitGroupId: number, newDate: string): Promise<void> {
  return invoke('update_split_group_date', { input: { splitGroupId, newDate } });
}

export async function listImportProfiles(): Promise<ImportProfileRow[]> {
  return invoke('list_import_profiles');
}

export async function createImportProfile(
  name: string,
  columnMappingJson: string,
  rules: Array<{ ruleType: string; sortOrder: number; paramsJson: string }>,
): Promise<number> {
  return invoke('create_import_profile', { name, columnMappingJson, rules });
}

export async function updateImportProfile(
  profileId: number,
  name: string,
  columnMappingJson: string,
  rules: Array<{ ruleType: string; sortOrder: number; paramsJson: string }>,
): Promise<void> {
  return invoke('update_import_profile', { profileId, name, columnMappingJson, rules });
}

export async function deleteImportProfile(profileId: number): Promise<void> {
  return invoke('delete_import_profile', { profileId });
}

export async function getPreferredProfile(accountId: number): Promise<ImportProfileRow | null> {
  return invoke('get_preferred_profile', { accountId });
}

export async function setPreferredProfile(
  accountId: number,
  profileId: number | null,
): Promise<void> {
  return invoke('set_preferred_profile', { accountId, profileId });
}
