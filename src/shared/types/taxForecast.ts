export const MAX_AMOUNT_MINOR = 10_000_000_000_000

export const FORECAST_LINES = ['revenue', 'payroll', 'depreciation', 'deductible_costs', 'non_deductible_costs'] as const
export type ForecastLine = (typeof FORECAST_LINES)[number]

export const VALUE_SOURCES = ['typed', 'entries', 'schedule', 'average', 'monthly_amount', 'nothing'] as const
export type ValueSource = (typeof VALUE_SOURCES)[number]

export const ENTRY_KINDS = ['revenue', 'depreciation', 'prepaid', 'expense'] as const
export type EntryKind = (typeof ENTRY_KINDS)[number]

export const ENTRY_NOT_COUNTED_REASONS = ['typed_value', 'after_actual_months'] as const
export type EntryNotCountedReason = (typeof ENTRY_NOT_COUNTED_REASONS)[number]

export interface TypedPair {
  soFarMinor: number | null
  restMinor: number | null
}

export interface EarlierProfit {
  profitYear: number
  amountMinor: number
  dividendTaxRateBps: number
}

export interface ForecastFigures {
  actualsThroughMonth: number
  payrollMonthlyMinor: number
  typedValues: Partial<Record<ForecastLine, TypedPair>>
  earlierProfits: EarlierProfit[]
}

export interface MinimumTaxBand {
  lowerBoundMinor: number
  amountMinor: number
}

export interface ForecastCell {
  amountMinor: number
  source: ValueSource
  untypedMinor: number
}

export interface ForecastLineResult {
  line: ForecastLine
  soFar: ForecastCell
  rest: ForecastCell
  yearMinor: number
}

export interface EarlierProfitResult {
  profitYear: number
  amountMinor: number
  payableMinor: number
  dividendTaxRateBps: number
  dividendTaxMinor: number
}
