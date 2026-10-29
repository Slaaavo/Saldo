import { FORECAST_LINES, type ForecastCell, type ForecastFigures, type ForecastLine, type ForecastLineResult, type TaxCalculationResult, type TaxModelDetail } from '@/shared/types'

export const makeFigures = (overrides?: Partial<ForecastFigures>): ForecastFigures => ({
  actualsThroughMonth: 12,
  payrollMonthlyMinor: 0,
  typedValues: {},
  earlierProfits: [],
  ...overrides,
})

export const makeTaxModel = (overrides?: Partial<TaxModelDetail>): TaxModelDetail => ({
  id: 5,
  name: 'Model',
  calendarYear: 2026,
  personId: 1,
  personName: 'Alice',
  personType: 'physical',
  vatStatus: 'none',
  vatFromDate: null,
  reserveFundCurrentMinor: null,
  reserveFundPctBps: null,
  reserveFundMaxMinor: null,
  dividendTaxRateBps: null,
  createdAt: '2026-01-01T00:00:00',
  updatedAt: '2026-01-01T00:00:00',
  brackets: [{ id: 1, sortOrder: 0, lowerBoundMinor: 0, rateType: 'flat', flatRateBps: 1500, tiersJson: null }],
  minimumTaxBands: [],
  figures: makeFigures(),
  ...overrides,
})

export const makeCell = (amountMinor: number, overrides?: Partial<ForecastCell>): ForecastCell => ({
  amountMinor,
  source: 'entries',
  untypedMinor: amountMinor,
  ...overrides,
})

export const makeLineResult = (line: ForecastLine, overrides?: Partial<ForecastLineResult>): ForecastLineResult => ({
  line,
  soFar: makeCell(0),
  rest: makeCell(0),
  yearMinor: 0,
  ...overrides,
})

export const makeResult = (overrides?: Partial<TaxCalculationResult>): TaxCalculationResult => ({
  totalIncomeMinor: 100000,
  totalTaxDeductibleExpensesMinor: 0,
  totalNonTaxDeductibleExpensesMinor: 0,
  taxBasisMinor: 100000,
  taxAmountMinor: 15000,
  totalProfitMinor: 85000,
  reserveFundGenerationMinor: 0,
  dividendMinor: 0,
  withholdingTaxMinor: 0,
  netDividendMinor: 0,
  monthlyTaxBurdenMinor: 0,
  personType: 'physical',
  eventBreakdowns: [],
  actualsThroughMonth: 12,
  lines: FORECAST_LINES.map((line) => makeLineResult(line)),
  appliedFlatRateBps: null,
  computedTaxMinor: 15000,
  minimumTaxMinor: null,
  dividendFromYearMinor: 0,
  dividendFromYearTaxMinor: 0,
  earlierProfits: [],
  ...overrides,
})
