import { describe, it, expect } from 'vitest';
import { normalizeIban, buildIbanLookup, matchIban } from './ibanMatcher';
import type { SnapshotRow } from '../../../shared/types/index';
import type { PartnerAccount } from '../../../shared/types/index';

function makeSnapshotRow(overrides?: Partial<SnapshotRow>): SnapshotRow {
  return {
    accountId: 1,
    accountName: 'Checking',
    accountType: 'account',
    iban: null,
    balanceMinor: 100000,
    currencyCode: 'EUR',
    currencyMinorUnits: 2,
    isCustom: false,
    convertedBalanceMinor: 100000,
    fxRateMissing: false,
    isBucketLinked: false,
    bucketLinks: [],
    linkedBalanceMinor: 0,
    cashflowTaggedMinor: 0,
    isLinkedToAsset: false,
    linkedAssetIds: [],
    ...overrides,
  };
}

function makePartner(overrides?: Partial<PartnerAccount>): PartnerAccount {
  return {
    id: 10,
    name: 'Acme Corp',
    iban: null,
    currencyCode: 'EUR',
    createdAt: '2026-01-01T00:00:00',
    ...overrides,
  };
}

describe('normalizeIban', () => {
  it('strips whitespace and uppercases', () => {
    expect(normalizeIban('SK12 3456 7890')).toBe('SK1234567890');
  });

  it('uppercases lowercase letters', () => {
    expect(normalizeIban('de89 3704 0044 0000')).toBe('DE89370400440000');
  });

  it('handles already normalized IBAN', () => {
    expect(normalizeIban('SK1234567890')).toBe('SK1234567890');
  });
});

describe('buildIbanLookup', () => {
  it('includes snapshot rows with accountType === account and non-null IBAN', () => {
    const snapshot = [makeSnapshotRow({ accountId: 1, iban: 'SK12 3456 7890' })];
    const lookup = buildIbanLookup(snapshot, []);

    expect(lookup.has('SK1234567890')).toBe(true);
    const entry = lookup.get('SK1234567890')!;
    expect(entry.accountId).toBe(1);
    expect(entry.accountType).toBe('account');
  });

  it('excludes snapshot rows with null IBAN', () => {
    const snapshot = [makeSnapshotRow({ accountId: 1, iban: null })];
    const lookup = buildIbanLookup(snapshot, []);
    expect(lookup.size).toBe(0);
  });

  it('excludes snapshot rows with accountType !== account', () => {
    const snapshot = [
      makeSnapshotRow({ accountId: 1, accountType: 'bucket', iban: 'SK12 3456 7890' }),
    ];
    const lookup = buildIbanLookup(snapshot, []);
    expect(lookup.size).toBe(0);
  });

  it('includes partner accounts with non-null IBAN', () => {
    const partners = [makePartner({ id: 10, iban: 'DE89 3704 0044 0000' })];
    const lookup = buildIbanLookup([], partners);

    expect(lookup.has('DE89370400440000')).toBe(true);
    const entry = lookup.get('DE89370400440000')!;
    expect(entry.accountId).toBe(10);
    expect(entry.accountType).toBe('partner');
  });

  it('excludes partner accounts with null IBAN', () => {
    const partners = [makePartner({ id: 10, iban: null })];
    const lookup = buildIbanLookup([], partners);
    expect(lookup.size).toBe(0);
  });

  it('combines snapshot and partner IBANs in the same lookup', () => {
    const snapshot = [makeSnapshotRow({ accountId: 1, iban: 'SK1234567890' })];
    const partners = [makePartner({ id: 10, iban: 'DE89370400440000' })];
    const lookup = buildIbanLookup(snapshot, partners);

    expect(lookup.size).toBe(2);
    expect(lookup.has('SK1234567890')).toBe(true);
    expect(lookup.has('DE89370400440000')).toBe(true);
  });
});

describe('matchIban', () => {
  it('returns ownAccount for a known account-type IBAN', () => {
    const snapshot = [
      makeSnapshotRow({ accountId: 1, accountName: 'Checking', iban: 'SK1234567890' }),
    ];
    const lookup = buildIbanLookup(snapshot, []);
    const result = matchIban('SK1234567890', lookup);

    expect(result.type).toBe('ownAccount');
    if (result.type === 'ownAccount') {
      expect(result.accountId).toBe(1);
      expect(result.accountName).toBe('Checking');
    }
  });

  it('returns partner for a known partner-type IBAN', () => {
    const partners = [makePartner({ id: 10, name: 'Acme Corp', iban: 'DE89370400440000' })];
    const lookup = buildIbanLookup([], partners);
    const result = matchIban('DE89370400440000', lookup);

    expect(result.type).toBe('partner');
    if (result.type === 'partner') {
      expect(result.accountId).toBe(10);
      expect(result.accountName).toBe('Acme Corp');
    }
  });

  it('returns unmatched for an unknown IBAN', () => {
    const lookup = buildIbanLookup([], []);
    const result = matchIban('XX0000000000', lookup);

    expect(result.type).toBe('unmatched');
    if (result.type === 'unmatched') {
      expect(result.rawIban).toBe('XX0000000000');
    }
  });
});
