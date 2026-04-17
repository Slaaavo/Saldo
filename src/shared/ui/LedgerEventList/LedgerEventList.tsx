import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { EventWithData, SnapshotRow, Currency } from '../../types'
import { formatDisplayDate } from '../../utils/format'
import { groupSplitEvents } from '../splitGroupUtils'
import type { SplitGroupRow } from '../splitGroupUtils'
import SplitGroupCard from './SplitGroupCard'
import StandaloneEventCard from './StandaloneEventCard'

interface Props {
  events: EventWithData[]
  accounts: SnapshotRow[]
  consolidationCurrency?: Currency | null
  onEditEvent: (event: EventWithData) => void
  onDeleteEvent: (eventId: number, eventType?: string) => void
  onDeleteTransferEvent?: (eventId: number, linkedEventId: number) => void
  onDeleteSplitGroup?: (splitGroupId: number) => void
  onEditTaxableSplitGroup?: (splitGroupId: number, eventType: string, legs: EventWithData[], groupNote: string | null, accountId: number) => void
}

const LedgerEventList = ({ events, accounts, consolidationCurrency, onEditEvent, onDeleteEvent, onDeleteTransferEvent, onDeleteSplitGroup, onEditTaxableSplitGroup }: Props) => {
  const { t } = useTranslation()
  const [expandedGroups, setExpandedGroups] = useState<Set<number>>(new Set())

  const toggleGroup = (id: number) =>
    setExpandedGroups((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })

  const accountPosition = new Map<number, number>()
  accounts.forEach((a, i) => accountPosition.set(a.accountId, i))

  const accountMap = new Map<number, SnapshotRow>()
  accounts.forEach((a) => accountMap.set(a.accountId, a))

  const allItems = groupSplitEvents(events)

  const groupMap = new Map<string, (EventWithData | SplitGroupRow)[]>()
  for (const item of allItems) {
    const dateKey = 'type' in item ? item.groupDate.substring(0, 10) : item.eventDate.substring(0, 10)
    if (!groupMap.has(dateKey)) groupMap.set(dateKey, [])
    groupMap.get(dateKey)!.push(item)
  }

  const sortedGroups = [...groupMap.entries()].sort(([a], [b]) => b.localeCompare(a))

  for (const [, groupItems] of sortedGroups) {
    groupItems.sort((a, b) => (accountPosition.get(a.accountId) ?? 0) - (accountPosition.get(b.accountId) ?? 0))
  }

  const latestBucketEventIds = new Set<number>()
  const seenBucketAccountIds = new Set<number>()
  for (const [, groupItems] of sortedGroups) {
    for (const item of groupItems) {
      if (!('type' in item) && item.accountType === 'bucket' && !seenBucketAccountIds.has(item.accountId)) {
        latestBucketEventIds.add(item.id)
        seenBucketAccountIds.add(item.accountId)
      }
    }
  }

  if (events.length === 0) {
    return <p className="text-sm text-muted-foreground">{t('ledger.empty')}</p>
  }

  return (
    <div className="flex flex-col gap-6">
      {sortedGroups.map(([dateKey, groupItems]) => (
        <div key={dateKey}>
          <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2">{formatDisplayDate(dateKey)}</h3>
          <div className="flex flex-col gap-2">
            {groupItems.map((item) => {
              if ('type' in item) {
                return (
                  <SplitGroupCard
                    key={`sg-${item.splitGroupId}`}
                    item={item}
                    isExpanded={expandedGroups.has(item.splitGroupId)}
                    onToggle={() => toggleGroup(item.splitGroupId)}
                    onEditTaxableSplitGroup={onEditTaxableSplitGroup}
                    onDeleteSplitGroup={onDeleteSplitGroup}
                  />
                )
              }
              const bucketSnap = latestBucketEventIds.has(item.id) ? accountMap.get(item.accountId) : undefined
              return (
                <StandaloneEventCard
                  key={item.id}
                  ev={item}
                  bucketSnap={bucketSnap}
                  consolidationCurrency={consolidationCurrency}
                  onEditEvent={onEditEvent}
                  onDeleteEvent={onDeleteEvent}
                  onDeleteTransferEvent={onDeleteTransferEvent}
                />
              )
            })}
          </div>
        </div>
      ))}
    </div>
  )
}

export default LedgerEventList
