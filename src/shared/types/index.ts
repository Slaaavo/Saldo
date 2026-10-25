export interface PersonRow {
  id: number
  name: string
  personType: string
  isDefault: boolean
  createdAt: string
  defaultRevenueAccountId: number
  defaultExpenseAccountId: number
  vatPayer: boolean
}

export interface AccountAssetLink {
  id: number
  accountId: number
  accountName: string
  assetId: number
  assetName: string
}

export interface TaxableCashflowLink {
  id: number
  taxableEventId: number
  cashflowEventId: number
  createdAt: string
}

export type ModalState =
  | { type: 'none' }
  | { type: 'createBalanceUpdate'; preselectedAccountId?: number }
  | { type: 'editBalanceUpdate'; event: EventWithData }
  | { type: 'editTransfer'; fromEvent: EventWithData; toEvent: EventWithData }
  | { type: 'createAccount'; accountType?: 'account' | 'bucket' }
  | { type: 'createAsset' }
  | {
      type: 'editAccount'
      accountId: number
      currentName: string
      accountType: string
      currentIban?: string | null
      currentPersonId?: number | null
      isCustomUnit: boolean
      currencyCode: string
      currencyMinorUnits: number
      currentPurchasePriceMinor: number | null
      currentPurchaseDate: string | null
      currentDepreciationPeriodMonths: number | null
    }
  | {
      type: 'confirmDeleteAccount'
      accountId: number
      name: string
      accountType?: 'account' | 'bucket' | 'asset'
    }
  | { type: 'confirmDeleteEvent'; eventId: number; eventType?: string }
  | { type: 'confirmDeleteSplitGroup'; splitGroupId: number }
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
      type: 'updateAssetValue'
      accountId: number
      accountName: string
      currencyCode: string
      currencyMinorUnits: number
      isCustomUnit: boolean
      balanceMinor: number
    }
  | { type: 'manageLinkedAssets'; accountId: number; accountName: string }
  | { type: 'csvImport' }
  | { type: 'ekasaImport' }
  | { type: 'addEvents' }
  | { type: 'createRevenue' }
  | { type: 'createExpense' }
  | { type: 'createPrepaidExpense' }
  | { type: 'editTaxableEvent'; event: EventWithData }
  | {
      type: 'editTaxableSplitGroup'
      splitGroupId: number
      eventType: 'revenue' | 'expense'
      legs: EventWithData[]
      groupNote: string | null
      accountId: number
    }

export interface Currency {
  id: number
  code: string
  name: string
  minorUnits: number
  isCustom: boolean
}

export interface Account {
  id: number
  name: string
  currencyId: number
  createdAt: string
}

export interface EventWithData {
  id: number
  accountId: number
  accountName: string
  accountType: string
  eventType: string
  eventDate: string
  amountMinor: number
  note: string | null
  createdAt: string
  currencyCode: string
  currencyMinorUnits: number
  counterpartAccountId: number | null
  counterpartAccountName: string | null
  bucketId: number | null
  bucketName: string | null
  originalCurrencyId: number | null
  originalCurrencyCode: string | null
  originalAmountMinor: number | null
  originalCurrencyMinorUnits: number | null
  fxRateMantissa: number | null
  fxRateExponent: number | null
  linkedEventId: number | null
  splitGroupId: number | null
  splitGroupNote: string | null
  vatRateBps: number | null
  vatReclaimablePctBps: number | null
  expenseDeductiblePctBps: number | null
  prepaidUntil: string | null
  isLinkedToTaxable: boolean
  linkedTaxableEventId: number | null
  hasLinkedCashflows: boolean
  linkedCashflowCount: number
  linkedAssetId: number | null
  linkedPrepaidEventId: number | null
  isSystemGenerated: boolean
  reclaimedVat: boolean | null
}

export interface ListEventsResult {
  events: EventWithData[]
  totalCount: number
}

export interface BucketLink {
  id: number
  eventId: number
  sourceAccountId: number
  sourceAccountName: string
  sourceCurrencyId: number
  sourceCurrencyCode: string
  sourceCurrencyMinorUnits: number
}

export interface PartnerAccount {
  id: number
  name: string
  iban: string | null
  currencyCode: string
  createdAt: string
}

export type AccountKind = 'account' | 'bucket' | 'asset'

/** Live shows everything Saldo knows. A moment shows the books as they stood then. `until` is YYYY-MM-DD (end of day) or YYYY-MM-DDTHH:MM:SS. */
export type AsOf = { kind: 'live' } | { kind: 'at'; until: string }

export interface RateUsed {
  mantissa: number
  exponent: number
  /** False: 1 consolidation unit = rate held units. True (custom unit): 1 held unit = rate consolidation units. */
  isDirect: boolean
  date: string
  isManual: boolean
  ageDays: number
  stale: boolean
}

/** An amount's value in the consolidation currency. Without a stored rate there is no value. */
export type Valuation = { kind: 'sameCurrency'; valueMinor: number } | { kind: 'converted'; valueMinor: number; rate: RateUsed } | { kind: 'noRate' }

export interface ValuedBucketLink extends BucketLink {
  balanceMinor: number
  valuation: Valuation
}

export interface TaggedCashflows {
  sourceAccountId: number
  sourceAccountName: string
  currencyCode: string
  currencyMinorUnits: number
  isCustom: boolean
  amountMinor: number
  valuation: Valuation
}

export interface SnapshotRow {
  accountId: number
  accountName: string
  accountType: AccountKind
  iban: string | null
  balanceMinor: number
  currencyCode: string
  currencyMinorUnits: number
  isCustom: boolean
  valuation: Valuation
  isLinkedToAsset: boolean
  linkedAssetIds: number[]
  isBucketLinked: boolean
  bucketLinks: ValuedBucketLink[]
  taggedCashflows: TaggedCashflows[]
  /** The row's value in the consolidation currency, summed over its valued parts (a bucket's own balance, links and tagged cashflows). */
  countedMinor: number
  /** Set only on an asset whose parts and linked accounts all have a rate. */
  equityMinor: number | null
  personId: number | null
  purchasePriceMinor: number | null
  purchaseDate: string | null
  depreciationPeriodMonths: number | null
}

export interface Money {
  amountMinor: number
  currencyCode: string
  minorUnits: number
}

export type LinePart =
  | { kind: 'balance' }
  | { kind: 'linkedAccount'; sourceAccountId: number; sourceAccountName: string }
  | { kind: 'taggedCashflows'; sourceAccountId: number; sourceAccountName: string }
  | { kind: 'bucketTotal' }

export interface LineSubject {
  accountId: number
  accountName: string
  accountType: AccountKind
  part: LinePart
}

export interface CountedLine extends LineSubject {
  native: Money
  /** Null for an amount already in the consolidation currency. */
  rate: RateUsed | null
  /** This line's contribution to the metric's total, sign included. */
  signedMinor: number
}

export type NotCountedReason = 'noRate' | 'linkedToAsset' | 'earmark' | 'notLiquid'

export interface SkippedLine extends LineSubject {
  amount: Money
  reason: NotCountedReason
}

export type MetricKind = 'netWorth' | 'liquid' | 'totalBalance' | 'leftToSpend'

export interface Metric {
  kind: MetricKind
  totalMinor: number
  counted: CountedLine[]
  skipped: SkippedLine[]
}

export type RateProblem = { kind: 'noRate' } | { kind: 'stale'; rate: RateUsed; limitDays: number }

export interface RateWarning {
  accountId: number
  accountName: string
  native: Money
  problem: RateProblem
}

export interface Snapshot {
  rows: SnapshotRow[]
  metrics: Metric[]
  rateWarnings: RateWarning[]
}

export interface FxRateRow {
  id: number
  date: string
  fromCurrencyCode: string
  toCurrencyCode: string
  rateMantissa: number
  rateExponent: number
  isManual: boolean
  fetchedAt: string
  isDirect: boolean
}

export interface DbLocationInfo {
  currentPath: string
  isDefault: boolean
  isDemoMode: boolean
  fallbackWarning: boolean
}

export interface PickDbFolderResult {
  folder: string
  dbExists: boolean
}

export interface AppError {
  code: string
  message: string
}

export interface ImportProfileRuleRow {
  id: number
  profileId: number
  ruleType: string
  sortOrder: number
  paramsJson: string
}

export interface ImportProfileRow {
  id: number
  name: string
  columnMappingJson: string
  rules: ImportProfileRuleRow[]
  createdAt: string
  updatedAt: string
}

export interface SignFromColumnParams {
  typeColumn: string
  negativeType: string
}

export interface OverrideDateFromDescriptionParams {
  descriptionColumn: string
  conditionRegex: string
  dateRegex: string
}

export type ImportRule =
  | ({ type: 'sign_from_column'; sortOrder: number } & SignFromColumnParams)
  | ({
      type: 'override_date_from_description'
      sortOrder: number
    } & OverrideDateFromDescriptionParams)

export interface TaxModelRow {
  id: number
  name: string
  calendarYear: number
  personId: number
  personName: string
  personType: string
  vatStatus: string
  vatFromDate: string | null
  reserveFundCurrentMinor: number | null
  reserveFundPctBps: number | null
  reserveFundMaxMinor: number | null
  dividendTaxRateBps: number | null
  createdAt: string
  updatedAt: string
}

export interface TaxModelBracketRow {
  id: number
  sortOrder: number
  lowerBoundMinor: number
  rateType: string
  flatRateBps: number | null
  tiersJson: string | null
}

export interface TaxModelDetail extends TaxModelRow {
  brackets: TaxModelBracketRow[]
}

export interface TaxEventBreakdown {
  eventId: number
  eventType: string
  eventDate: string
  note: string | null
  amountMinor: number
  netAmountMinor: number
  vatAmountMinor: number
  reclaimableVatMinor: number
  nonReclaimableVatMinor: number
  taxDeductibleCostMinor: number
  nonTaxDeductibleCostMinor: number
}

export interface TaxCalculationResult {
  totalIncomeMinor: number
  totalTaxDeductibleExpensesMinor: number
  totalNonTaxDeductibleExpensesMinor: number
  taxBasisMinor: number
  taxAmountMinor: number
  totalProfitMinor: number
  reserveFundGenerationMinor: number
  dividendMinor: number
  withholdingTaxMinor: number
  netDividendMinor: number
  monthlyTaxBurdenMinor: number
  personType: string
  eventBreakdowns: TaxEventBreakdown[]
}
