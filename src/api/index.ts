import { invoke } from '@tauri-apps/api/core';
import type { SnapshotRow, EventWithData } from '../types';

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

export async function listEvents(accountId?: number): Promise<EventWithData[]> {
  return invoke('list_events', {
    filter: { accountId: accountId ?? null },
  });
}

export async function createAccount(
  name: string,
  currencyId: number,
  initialBalanceMinor?: number,
): Promise<number> {
  return invoke('create_account', {
    input: {
      name,
      currencyId,
      initialBalanceMinor: initialBalanceMinor ?? null,
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
