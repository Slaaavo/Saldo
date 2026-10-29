import { FORECAST_LINES, MAX_AMOUNT_MINOR, type EarlierProfit, type ForecastFigures, type ForecastLine, type ForecastLineResult, type TypedPair } from '@/shared/types'
import { bpsToPct, fromMinorUnits, pctToBps, toMinorUnits } from '@/shared/utils/format'

export const PERIODS = ['soFar', 'rest'] as const
export type Period = (typeof PERIODS)[number]

const PAIR_KEY = { soFar: 'soFarMinor', rest: 'restMinor' } as const satisfies Record<Period, keyof TypedPair>

export interface EarlierProfitDraft {
  profitYear: string
  amount: string
  dividendTaxPct: string
}

export interface FiguresDraft {
  actualsThroughMonth: number
  payrollMonthly: string
  typed: Partial<Record<ForecastLine, Partial<Record<Period, string>>>>
  earlierProfits: EarlierProfitDraft[]
}

export type AmountField = { target: 'cell'; line: ForecastLine; period: Period } | { target: 'payrollMonthly' } | { target: 'earlierProfit'; row: number }

export type FiguresDraftError =
  | { kind: 'amountInvalid' | 'amountNegative'; field: AmountField }
  | { kind: 'yearInvalid' | 'rateInvalid' | 'yearDuplicate'; row: number }
  | { kind: 'yearNotBefore'; row: number; calendarYear: number }

export type FiguresFromDraft = { ok: true; figures: ForecastFigures } | { ok: false; error: FiguresDraftError }

type ParsedAmount = { ok: true; minor: number | null } | { ok: false; kind: 'amountInvalid' | 'amountNegative' }

const MAX_BPS = 10000

export const emptyEarlierProfit = (): EarlierProfitDraft => ({ profitYear: '', amount: '', dividendTaxPct: '' })

export const cellText = (draft: FiguresDraft, line: ForecastLine, period: Period): string => draft.typed[line]?.[period] ?? ''

export const withCell = (draft: FiguresDraft, line: ForecastLine, period: Period, text: string): FiguresDraft => ({
  ...draft,
  typed: { ...draft.typed, [line]: { ...draft.typed[line], [period]: text } },
})

export const coversNoMonths = (actualsThroughMonth: number, period: Period): boolean => (period === 'soFar' ? actualsThroughMonth === 0 : actualsThroughMonth === 12)

export const draftFromFigures = (figures: ForecastFigures, minorUnits: number): FiguresDraft => {
  const typed: FiguresDraft['typed'] = {}
  for (const line of FORECAST_LINES) {
    const pair = figures.typedValues[line]
    if (!pair) continue
    const cells: Partial<Record<Period, string>> = {}
    for (const period of PERIODS) {
      const minor = pair[PAIR_KEY[period]]
      if (minor !== null) cells[period] = fromMinorUnits(minor, minorUnits)
    }
    typed[line] = cells
  }
  return {
    actualsThroughMonth: figures.actualsThroughMonth,
    payrollMonthly: figures.payrollMonthlyMinor === 0 ? '' : fromMinorUnits(figures.payrollMonthlyMinor, minorUnits),
    typed,
    earlierProfits: figures.earlierProfits.map((row) => ({
      profitYear: String(row.profitYear),
      amount: fromMinorUnits(row.amountMinor, minorUnits),
      dividendTaxPct: bpsToPct(row.dividendTaxRateBps),
    })),
  }
}

const parseAmount = (text: string, minorUnits: number): ParsedAmount => {
  if (text.trim() === '') return { ok: true, minor: null }
  const minor = toMinorUnits(text, minorUnits)
  if (!Number.isFinite(minor)) return { ok: false, kind: 'amountInvalid' }
  if (minor < 0) return { ok: false, kind: 'amountNegative' }
  if (minor > MAX_AMOUNT_MINOR) return { ok: false, kind: 'amountInvalid' }
  return { ok: true, minor }
}

const parseEarlierProfit = (
  row: EarlierProfitDraft,
  index: number,
  minorUnits: number,
  calendarYear: number,
): { ok: true; value: EarlierProfit } | { ok: false; error: FiguresDraftError } => {
  const rowNumber = index + 1
  const year = row.profitYear.trim()
  if (!/^\d{4}$/.test(year)) return { ok: false, error: { kind: 'yearInvalid', row: rowNumber } }
  if (Number(year) >= calendarYear) return { ok: false, error: { kind: 'yearNotBefore', row: rowNumber, calendarYear } }
  const amount = parseAmount(row.amount, minorUnits)
  if (!amount.ok) return { ok: false, error: { kind: amount.kind, field: { target: 'earlierProfit', row: rowNumber } } }
  if (amount.minor === null) return { ok: false, error: { kind: 'amountInvalid', field: { target: 'earlierProfit', row: rowNumber } } }
  const rateBps = pctToBps(row.dividendTaxPct)
  if (rateBps === null || rateBps < 0 || rateBps > MAX_BPS) return { ok: false, error: { kind: 'rateInvalid', row: rowNumber } }
  return { ok: true, value: { profitYear: Number(year), amountMinor: amount.minor, dividendTaxRateBps: rateBps } }
}

const isBlankEarlierProfit = (row: EarlierProfitDraft): boolean => row.profitYear.trim() === '' && row.amount.trim() === '' && row.dividendTaxPct.trim() === ''

export const figuresFromDraft = (draft: FiguresDraft, minorUnits: number, calendarYear: number): FiguresFromDraft => {
  const typedValues: ForecastFigures['typedValues'] = {}
  for (const line of FORECAST_LINES) {
    const cells: Record<Period, number | null> = { soFar: null, rest: null }
    for (const period of PERIODS) {
      if (coversNoMonths(draft.actualsThroughMonth, period)) continue
      const parsed = parseAmount(cellText(draft, line, period), minorUnits)
      if (!parsed.ok) return { ok: false, error: { kind: parsed.kind, field: { target: 'cell', line, period } } }
      cells[period] = parsed.minor
    }
    if (cells.soFar !== null || cells.rest !== null) typedValues[line] = { soFarMinor: cells.soFar, restMinor: cells.rest }
  }

  const payroll = parseAmount(draft.payrollMonthly, minorUnits)
  if (!payroll.ok) return { ok: false, error: { kind: payroll.kind, field: { target: 'payrollMonthly' } } }

  const earlierProfits: EarlierProfit[] = []
  const seenYears = new Set<number>()
  for (const [index, row] of draft.earlierProfits.entries()) {
    if (isBlankEarlierProfit(row)) continue
    const parsed = parseEarlierProfit(row, index, minorUnits, calendarYear)
    if (!parsed.ok) return parsed
    if (seenYears.has(parsed.value.profitYear)) return { ok: false, error: { kind: 'yearDuplicate', row: index + 1 } }
    seenYears.add(parsed.value.profitYear)
    earlierProfits.push(parsed.value)
  }

  return {
    ok: true,
    figures: { actualsThroughMonth: draft.actualsThroughMonth, payrollMonthlyMinor: payroll.minor ?? 0, typedValues, earlierProfits },
  }
}

const sameText = (a: string | undefined, b: string | undefined): boolean => (a ?? '') === (b ?? '')

export const placeholdersCurrent = (draft: FiguresDraft, saved: FiguresDraft, period: Period): boolean => {
  if (draft.actualsThroughMonth !== saved.actualsThroughMonth) return false
  if (period === 'soFar') return true
  if (!sameText(draft.payrollMonthly, saved.payrollMonthly)) return false
  return FORECAST_LINES.every((line) => PERIODS.every((p) => sameText(draft.typed[line]?.[p], saved.typed[line]?.[p])))
}

export const placeholderFor = (lines: ForecastLineResult[] | undefined, line: ForecastLine, period: Period, minorUnits: number): string | undefined => {
  const untypedMinor = lines?.find((row) => row.line === line)?.[period].untypedMinor
  return untypedMinor === undefined ? undefined : fromMinorUnits(untypedMinor, minorUnits)
}
