import { MAX_AMOUNT_MINOR, type BudgetCell, type MonthKey } from '@/shared/types'
import { yearOf } from './month'

export const DRAFT_ROWS = ['budget', 'vat'] as const
export type DraftRow = (typeof DRAFT_ROWS)[number]

export type DraftCell = Readonly<Record<DraftRow, string>>

/** What the user typed, as typed, until Save. A month with both cells blank has no entry. */
export type BudgetDraft = ReadonlyMap<MonthKey, DraftCell>

export type ParsedText = { kind: 'empty' } | { kind: 'value'; value: number } | { kind: 'invalid' }

export interface InvalidCell {
  month: MonthKey
  row: DraftRow
}

/** `cells` holds every month with a valid value. An invalid cell counts as empty there and is listed in `invalid`. */
export interface ParsedDraft {
  cells: BudgetCell[]
  invalid: InvalidCell[]
}

export interface YearSpan {
  fromYear: number
  toYear: number
}

const EMPTY_CELL: DraftCell = { budget: '', vat: '' }
const EMPTY: ParsedText = { kind: 'empty' }
const INVALID: ParsedText = { kind: 'invalid' }
const VAT_SCALE = 2
const MAX_VAT_BPS = 10000
const MAX_EXACT_DIGITS = 15
// The preview command refuses a year outside these and a span of more years.
const FIRST_YEAR = 1990
const LAST_YEAR = 2200
const MAX_YEARS = 40

const isBlank = (text: string) => text.trim() === ''

export const cellText = (draft: BudgetDraft, month: MonthKey, row: DraftRow): string => draft.get(month)?.[row] ?? ''

export const setCell = (draft: BudgetDraft, month: MonthKey, row: DraftRow, text: string): BudgetDraft => {
  const cell = { ...(draft.get(month) ?? EMPTY_CELL), [row]: text }
  const next = new Map(draft)
  if (isBlank(cell.budget) && isBlank(cell.vat)) next.delete(month)
  else next.set(month, cell)
  return next
}

const splitDigits = (value: number, scale: number): [string, string] => {
  const digits = String(value).padStart(scale + 1, '0')
  return [digits.slice(0, digits.length - scale), digits.slice(digits.length - scale)]
}

const amountText = (minor: number, minorUnits: number): string => {
  const [whole, fraction] = splitDigits(minor, minorUnits)
  return /^0*$/.test(fraction) ? whole : `${whole}.${fraction}`
}

const vatText = (bps: number): string => {
  const [whole, fraction] = splitDigits(bps, VAT_SCALE)
  const significant = fraction.replace(/0+$/, '')
  return significant === '' ? whole : `${whole}.${significant}`
}

export const draftFromCells = (cells: readonly BudgetCell[], minorUnits: number): BudgetDraft =>
  new Map(
    cells.map((cell) => [
      cell.month,
      {
        budget: cell.budgetMinor === null ? '' : amountText(cell.budgetMinor, minorUnits),
        vat: cell.vatBps === null ? '' : vatText(cell.vatBps),
      },
    ]),
  )

const DECIMAL = /^(\d+)(?:[.,](\d+))?$/

// Digits only, so "1,234.50" or "1.234,50" is invalid instead of misread, and the value is exact without floating point.
const parseDecimal = (text: string, scale: number, max: number): ParsedText => {
  const compact = text.replace(/\s/g, '')
  if (compact === '') return EMPTY
  const match = DECIMAL.exec(compact)
  if (!match) return INVALID
  const [, whole, fraction = ''] = match
  if (fraction.length > scale) return INVALID
  const digits = `${whole}${fraction.padEnd(scale, '0')}`.replace(/^0+(?=\d)/, '')
  if (digits.length > MAX_EXACT_DIGITS) return INVALID
  const value = Number(digits)
  return value > max ? INVALID : { kind: 'value', value }
}

/** A budget without VAT: digits with at most one decimal comma or dot and no sign. Spaces are ignored. */
export const parseAmount = (text: string, minorUnits: number): ParsedText => parseDecimal(text, minorUnits, MAX_AMOUNT_MINOR)

/** A VAT rate in percent, 0 to 100, with an optional trailing %, as basis points. */
export const parseVatBps = (text: string): ParsedText => parseDecimal(text.trim().replace(/%$/, ''), VAT_SCALE, MAX_VAT_BPS)

export const parseCell = (row: DraftRow, text: string, minorUnits: number): ParsedText => (row === 'budget' ? parseAmount(text, minorUnits) : parseVatBps(text))

const valueOrNull = (parsed: ParsedText): number | null => (parsed.kind === 'value' ? parsed.value : null)

export const parseDraft = (draft: BudgetDraft, minorUnits: number): ParsedDraft => {
  const cells: BudgetCell[] = []
  const invalid: InvalidCell[] = []
  for (const [month, cell] of [...draft].sort(([a], [b]) => (a < b ? -1 : 1))) {
    const budget = parseAmount(cell.budget, minorUnits)
    const vat = parseVatBps(cell.vat)
    if (budget.kind === 'invalid') invalid.push({ month, row: 'budget' })
    if (vat.kind === 'invalid') invalid.push({ month, row: 'vat' })
    const budgetMinor = valueOrNull(budget)
    const vatBps = valueOrNull(vat)
    if (budgetMinor !== null || vatBps !== null) cells.push({ month, budgetMinor, vatBps })
  }
  return { cells, invalid }
}

/** The first to the last year that holds a value, or only the moment's year when none does. */
export const spanOf = (draft: BudgetDraft, momentYear: number): YearSpan => {
  const years = [...draft.keys()].map(yearOf)
  if (years.length === 0) return { fromYear: momentYear, toYear: momentYear }
  return { fromYear: Math.min(...years), toYear: Math.max(...years) }
}

/** The span one year wider on that side, or null where the preview would refuse it. */
export const widen = (span: YearSpan, side: 'before' | 'after'): YearSpan | null => {
  const next = side === 'before' ? { ...span, fromYear: span.fromYear - 1 } : { ...span, toYear: span.toYear + 1 }
  if (next.fromYear < FIRST_YEAR || next.toYear > LAST_YEAR || next.toYear - next.fromYear >= MAX_YEARS) return null
  return next
}
