import type { TFunction } from 'i18next'
import type { MonthKey } from '@/shared/types'
import type { DraftRow } from './budgetDraft'
import { formatMonth } from './month'

const CELL_KEYS = { budget: 'incomeBudget.table.budgetCell', vat: 'incomeBudget.table.vatCell' } as const satisfies Record<DraftRow, string>
const FILL_KEYS = { budget: 'incomeBudget.table.fillBudget', vat: 'incomeBudget.table.fillVat' } as const satisfies Record<DraftRow, string>

/** "Budget, November 2026": the input's accessible name, also used to name an invalid cell. */
export const cellLabel = (t: TFunction, language: string, row: DraftRow, month: MonthKey): string => t(CELL_KEYS[row], { month: formatMonth(month, language, 'long') })

export const fillLabel = (t: TFunction, language: string, row: DraftRow, month: MonthKey): string => t(FILL_KEYS[row], { month: formatMonth(month, language, 'long') })
