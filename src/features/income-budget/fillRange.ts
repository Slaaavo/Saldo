import type { MonthKey } from '@/shared/types'
import { cellText, setCell, type BudgetDraft, type DraftRow } from './budgetDraft'

export interface FillSpan {
  row: DraftRow
  originIdx: number
  targetIdx: number
}

export interface TableGeometry {
  edges: readonly number[]
  stickyWidth: number
}

export interface ScrollerPointer {
  viewportX: number
  scrollLeft: number
  viewportWidth: number
}

const EDGE_ZONE = 48
const MAX_SCROLL_STEP = 24
const MONTHS_BEFORE_MOMENT = 4

export const fillRange = (draft: BudgetDraft, months: readonly MonthKey[], { row, originIdx, targetIdx }: FillSpan): BudgetDraft => {
  const text = cellText(draft, months[originIdx], row)
  const from = Math.min(originIdx, targetIdx)
  const to = Math.max(originIdx, targetIdx)
  return months.slice(from, to + 1).reduce((next, month) => setCell(next, month, row, text), draft)
}

export const columnAtX = ({ edges, stickyWidth }: TableGeometry, { viewportX, scrollLeft }: ScrollerPointer): number => {
  const x = Math.max(viewportX, stickyWidth) + scrollLeft
  const last = edges.length - 2
  let index = 0
  while (index < last && x >= edges[index + 1]) index++
  return index
}

export const edgeScrollDelta = ({ stickyWidth }: TableGeometry, { viewportX, viewportWidth }: ScrollerPointer): number => {
  const step = (depth: number) => Math.ceil(Math.min(depth / EDGE_ZONE, 1) * MAX_SCROLL_STEP)
  const right = viewportX - (viewportWidth - EDGE_ZONE)
  if (right > 0) return step(right)
  const left = stickyWidth + EDGE_ZONE - viewportX
  if (left > 0) return -step(left)
  return 0
}

export const scrollLeftForMoment = ({ edges, stickyWidth }: TableGeometry, months: readonly MonthKey[], moment: MonthKey): number => {
  const shown = months.indexOf(moment)
  const index = shown >= 0 ? shown : moment > months[months.length - 1] ? months.length - 1 : 0
  return Math.max(0, edges[Math.max(0, index - MONTHS_BEFORE_MOMENT)] - stickyWidth)
}
