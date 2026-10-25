import type { CountedLine, Metric, MetricKind, Money, RateUsed, RateWarning, SkippedLine, Snapshot, SnapshotRow, TaggedCashflows, Valuation, ValuedBucketLink } from '../types'

export const makeMoney = (amountMinor: number, currencyCode = 'EUR', minorUnits = 2): Money => ({ amountMinor, currencyCode, minorUnits })

export const makeRate = (overrides?: Partial<RateUsed>): RateUsed => ({
  mantissa: 10842,
  exponent: -4,
  isDirect: false,
  date: '2026-10-20',
  isManual: false,
  ageDays: 0,
  stale: false,
  ...overrides,
})

export const sameCurrency = (valueMinor: number): Valuation => ({ kind: 'sameCurrency', valueMinor })

export const converted = (valueMinor: number, rate?: Partial<RateUsed>): Valuation => ({ kind: 'converted', valueMinor, rate: makeRate(rate) })

export const NO_RATE: Valuation = { kind: 'noRate' }

export const makeRow = (overrides?: Partial<SnapshotRow>): SnapshotRow => {
  const balanceMinor = overrides?.balanceMinor ?? 100000
  return {
    accountId: 1,
    accountName: 'Checking',
    accountType: 'account',
    iban: null,
    balanceMinor,
    currencyCode: 'EUR',
    currencyMinorUnits: 2,
    isCustom: false,
    valuation: sameCurrency(balanceMinor),
    isLinkedToAsset: false,
    linkedAssetIds: [],
    isBucketLinked: false,
    bucketLinks: [],
    taggedCashflows: [],
    countedMinor: balanceMinor,
    equityMinor: null,
    personId: null,
    purchasePriceMinor: null,
    purchaseDate: null,
    depreciationPeriodMonths: null,
    ...overrides,
  }
}

export const makeBucketLink = (overrides?: Partial<ValuedBucketLink>): ValuedBucketLink => ({
  id: 1,
  eventId: 1,
  sourceAccountId: 5,
  sourceAccountName: 'Savings',
  sourceCurrencyId: 1,
  sourceCurrencyCode: 'EUR',
  sourceCurrencyMinorUnits: 2,
  balanceMinor: 100000,
  valuation: sameCurrency(100000),
  ...overrides,
})

export const makeTaggedCashflows = (overrides?: Partial<TaggedCashflows>): TaggedCashflows => ({
  sourceAccountId: 6,
  sourceAccountName: 'Revolut',
  currencyCode: 'EUR',
  currencyMinorUnits: 2,
  isCustom: false,
  amountMinor: 3000,
  valuation: sameCurrency(3000),
  ...overrides,
})

export const makeCountedLine = (overrides?: Partial<CountedLine>): CountedLine => ({
  accountId: 1,
  accountName: 'Checking',
  accountType: 'account',
  part: { kind: 'balance' },
  native: makeMoney(100000),
  rate: null,
  signedMinor: 100000,
  ...overrides,
})

export const makeSkippedLine = (overrides?: Partial<SkippedLine>): SkippedLine => ({
  accountId: 2,
  accountName: 'Dollar account',
  accountType: 'account',
  part: { kind: 'balance' },
  amount: makeMoney(5000, 'USD'),
  reason: 'noRate',
  ...overrides,
})

export const makeMetric = (kind: MetricKind, overrides?: Partial<Metric>): Metric => {
  const counted = overrides?.counted ?? [makeCountedLine()]
  return {
    kind,
    totalMinor: counted.reduce((sum, line) => sum + line.signedMinor, 0),
    counted,
    skipped: [],
    ...overrides,
  }
}

export const makeRateWarning = (overrides?: Partial<RateWarning>): RateWarning => ({
  accountId: 2,
  accountName: 'Dollar account',
  native: makeMoney(5000, 'USD'),
  problem: { kind: 'noRate' },
  ...overrides,
})

export const makeSnapshot = (overrides?: Partial<Snapshot>): Snapshot => ({
  rows: [],
  metrics: [],
  rateWarnings: [],
  ...overrides,
})
