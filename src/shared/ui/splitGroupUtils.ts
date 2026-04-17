import type { EventWithData } from '../types'

export interface SplitGroupRow {
  type: 'splitGroup'
  splitGroupId: number
  groupDate: string
  groupTotal: number
  legCount: number
  groupNote: string | null
  legs: EventWithData[]
  accountId: number
  currencyCode: string
  currencyMinorUnits: number
  hasLinkedCashflows: boolean
  linkedCashflowCount: number
}

export const groupSplitEvents = (events: EventWithData[]): (EventWithData | SplitGroupRow)[] => {
  const standalone: EventWithData[] = []
  const grouped = new Map<number, EventWithData[]>()

  for (const ev of events) {
    if (ev.splitGroupId === null) {
      standalone.push(ev)
    } else {
      if (!grouped.has(ev.splitGroupId)) {
        grouped.set(ev.splitGroupId, [])
      }
      grouped.get(ev.splitGroupId)!.push(ev)
    }
  }

  const splitGroupRows: SplitGroupRow[] = []
  for (const [splitGroupId, legs] of grouped.entries()) {
    const first = legs[0]
    let groupTotal = 0
    let hasLinkedCashflows = false
    let linkedCashflowCount = 0
    for (const leg of legs) {
      groupTotal += leg.amountMinor
      if (leg.hasLinkedCashflows) hasLinkedCashflows = true
      linkedCashflowCount += leg.linkedCashflowCount
    }
    splitGroupRows.push({
      type: 'splitGroup',
      splitGroupId,
      groupDate: first.eventDate,
      groupTotal,
      legCount: legs.length,
      groupNote: first.splitGroupNote,
      legs,
      accountId: first.accountId,
      currencyCode: first.currencyCode,
      currencyMinorUnits: first.currencyMinorUnits,
      hasLinkedCashflows,
      linkedCashflowCount,
    })
  }

  return [...standalone, ...splitGroupRows]
}
