import type { SnapshotRow, PartnerAccount } from '../../../shared/types/index';
import type { IbanMatchResult } from './types';

export interface IbanLookupEntry {
  accountId: number;
  accountName: string;
  accountType: string;
  iban: string;
}

export function normalizeIban(raw: string): string {
  return raw.replace(/\s/g, '').toUpperCase();
}

export function buildIbanLookup(
  snapshot: SnapshotRow[],
  partners: PartnerAccount[],
): Map<string, IbanLookupEntry> {
  const lookup = new Map<string, IbanLookupEntry>();

  for (const row of snapshot) {
    if (row.accountType === 'account' && row.iban) {
      const key = normalizeIban(row.iban);
      if (key) {
        lookup.set(key, {
          accountId: row.accountId,
          accountName: row.accountName,
          accountType: 'account',
          iban: key,
        });
      }
    }
  }

  for (const partner of partners) {
    if (partner.iban) {
      const key = normalizeIban(partner.iban);
      if (key) {
        lookup.set(key, {
          accountId: partner.id,
          accountName: partner.name,
          accountType: 'partner',
          iban: key,
        });
      }
    }
  }

  return lookup;
}

export function matchIban(
  normalizedIban: string,
  lookup: Map<string, IbanLookupEntry>,
): IbanMatchResult {
  const entry = lookup.get(normalizedIban);

  if (!entry) {
    return { type: 'unmatched', rawIban: normalizedIban };
  }

  if (entry.accountType === 'account') {
    return { type: 'ownAccount', accountId: entry.accountId, accountName: entry.accountName };
  }

  return { type: 'partner', accountId: entry.accountId, accountName: entry.accountName };
}
