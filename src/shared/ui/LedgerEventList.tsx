import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { EventWithData, SnapshotRow, Currency } from '../types'
import { formatDisplayDate } from '../utils/format'
import { cn } from '@/shared/lib/utils'
import NumberValue from './NumberValue'
import BucketAmountWithTooltip from '../../features/buckets/BucketAmountWithTooltip'
import { Button } from './button'
import { Card, CardContent } from './card'
import { Pencil, Trash2, ArrowUpDown, Receipt, ChevronDown } from 'lucide-react'
import { groupSplitEvents } from './splitGroupUtils'
import type { SplitGroupRow } from './splitGroupUtils'

export type { SplitGroupRow } from './splitGroupUtils'

interface Props {
  events: EventWithData[]
  accounts: SnapshotRow[]
  consolidationCurrency?: Currency | null
  onEditEvent: (event: EventWithData) => void
  onDeleteEvent: (eventId: number) => void
  onDeleteTransferEvent?: (eventId: number, linkedEventId: number) => void
}

const LedgerEventList = ({ events, accounts, consolidationCurrency, onEditEvent, onDeleteEvent, onDeleteTransferEvent }: Props) => {
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

  // Build account position map from snapshot order
  const accountPosition = new Map<number, number>()
  accounts.forEach((a, i) => accountPosition.set(a.accountId, i))

  // Build lookup map from accountId to SnapshotRow
  const accountMap = new Map<number, SnapshotRow>()
  accounts.forEach((a) => accountMap.set(a.accountId, a))

  // Merge split group legs into SplitGroupRow items
  const allItems = groupSplitEvents(events)

  // Group items by date
  const groupMap = new Map<string, (EventWithData | SplitGroupRow)[]>()
  for (const item of allItems) {
    const dateKey = 'type' in item ? item.groupDate.substring(0, 10) : item.eventDate.substring(0, 10)
    if (!groupMap.has(dateKey)) {
      groupMap.set(dateKey, [])
    }
    groupMap.get(dateKey)!.push(item)
  }

  // Sort date groups: most recent first
  const sortedGroups = [...groupMap.entries()].sort(([a], [b]) => b.localeCompare(a))

  // Sort items within each group by account position
  for (const [, groupItems] of sortedGroups) {
    groupItems.sort((a, b) => (accountPosition.get(a.accountId) ?? 0) - (accountPosition.get(b.accountId) ?? 0))
  }

  // Identify the most recent event per bucket (for showing current balance tooltip)
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
                const isExpanded = expandedGroups.has(item.splitGroupId)
                return (
                  <div key={`sg-${item.splitGroupId}`}>
                    <Card className="relative group">
                      <CardContent className="flex items-center justify-between p-4">
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2 flex-wrap">
                            <p className="text-sm font-semibold truncate">{accountMap.get(item.accountId)?.accountName ?? ''}</p>
                            <span className="inline-flex items-center gap-1 rounded-full bg-purple-100 px-2 py-0.5 text-xs font-medium text-purple-700 dark:bg-purple-900/30 dark:text-purple-400">
                              {t('ledger.splitGroup.badge', { n: item.legCount })}
                            </span>
                          </div>
                          {item.groupNote && <p className="text-xs text-muted-foreground italic truncate">{item.groupNote}</p>}
                        </div>
                        <div className="flex items-center gap-6">
                          <NumberValue
                            value={item.groupTotal}
                            currencyCode={item.currencyCode}
                            minorUnits={item.currencyMinorUnits}
                            className={cn('text-sm font-bold tabular-nums', item.groupTotal < 0 && 'text-destructive')}
                          />
                          <div className="flex items-center gap-1">
                            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={() => item.legs.forEach((leg) => onDeleteEvent(leg.id))}>
                              <Trash2 className="h-4 w-4" />
                            </Button>
                          </div>
                        </div>
                      </CardContent>
                      <button
                        className="absolute bottom-0 left-1/2 -translate-x-1/2 translate-y-1/2 flex items-center justify-center h-5 w-5 rounded-full bg-background border border-border text-muted-foreground hover:text-foreground opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer z-10"
                        onClick={() => toggleGroup(item.splitGroupId)}
                      >
                        <ChevronDown className={cn('h-3 w-3 transition-transform', isExpanded && 'rotate-180')} />
                      </button>
                    </Card>
                    {isExpanded &&
                      item.legs.map((leg, index) => (
                        <Card key={`sg-${item.splitGroupId}-leg-${leg.id}`} className="ml-4 mt-1 border-l-2 border-l-purple-200 dark:border-l-purple-800">
                          <CardContent className="flex items-center justify-between p-4">
                            <div className="flex-1 min-w-0">
                              <div className="flex items-center gap-2 flex-wrap">
                                <p className="text-sm font-semibold truncate">{t('ledger.splitGroup.splitLabel', { n: index + 1 })}</p>
                                {leg.eventType === 'transfer' && (
                                  <span className="inline-flex items-center gap-1 rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-700 dark:bg-blue-900/30 dark:text-blue-400">
                                    <ArrowUpDown className="h-3 w-3" />
                                    {t('events.type.transfer')}
                                  </span>
                                )}
                                {leg.eventType === 'cashflow' && (
                                  <span className="inline-flex items-center gap-1 rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-700 dark:bg-green-900/30 dark:text-green-400">
                                    <Receipt className="h-3 w-3" />
                                    {t('events.type.cashflow')}
                                  </span>
                                )}
                              </div>
                              {leg.counterpartAccountName && (
                                <p className="text-xs text-muted-foreground truncate">
                                  {leg.eventType === 'transfer' ? '↔' : '→'} {leg.counterpartAccountName}
                                </p>
                              )}
                              {leg.note && <p className="text-xs text-muted-foreground italic truncate">{leg.note}</p>}
                              {leg.bucketName && (
                                <span className="inline-flex items-center rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground mt-0.5">{leg.bucketName}</span>
                              )}
                            </div>
                            <div className="flex items-center gap-6">
                              <NumberValue
                                value={leg.amountMinor}
                                currencyCode={item.currencyCode}
                                minorUnits={item.currencyMinorUnits}
                                className={cn('text-sm font-bold tabular-nums', leg.amountMinor < 0 && 'text-destructive')}
                              />
                            </div>
                          </CardContent>
                        </Card>
                      ))}
                  </div>
                )
              }

              // Standalone event — rendered exactly as before
              const ev = item
              const bucketSnap = latestBucketEventIds.has(ev.id) ? accountMap.get(ev.accountId) : undefined
              return (
                <Card key={ev.id}>
                  <CardContent className="flex items-center justify-between p-4">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <p className="text-sm font-semibold truncate">{ev.accountName}</p>
                        {ev.eventType === 'transfer' && (
                          <span className="inline-flex items-center gap-1 rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-700 dark:bg-blue-900/30 dark:text-blue-400">
                            <ArrowUpDown className="h-3 w-3" />
                            {t('events.type.transfer')}
                          </span>
                        )}
                        {ev.eventType === 'cashflow' && (
                          <span className="inline-flex items-center gap-1 rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-700 dark:bg-green-900/30 dark:text-green-400">
                            <Receipt className="h-3 w-3" />
                            {t('events.type.cashflow')}
                          </span>
                        )}
                      </div>
                      {ev.accountType === 'asset' && <p className="text-xs text-muted-foreground truncate">{t('ledger.valueUpdate')}</p>}
                      {ev.counterpartAccountName && (
                        <p className="text-xs text-muted-foreground truncate">
                          {ev.eventType === 'transfer' ? '↔' : '→'} {ev.counterpartAccountName}
                        </p>
                      )}
                      {ev.note && <p className="text-xs text-muted-foreground italic truncate">{ev.note}</p>}
                      {ev.bucketName && <span className="inline-flex items-center rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground mt-0.5">{ev.bucketName}</span>}
                    </div>
                    <div className="flex items-center gap-6">
                      <div className="flex flex-col items-end gap-0.5">
                        {bucketSnap ? (
                          <BucketAmountWithTooltip
                            totalMinor={bucketSnap.convertedBalanceMinor}
                            manualBalanceMinor={bucketSnap.balanceMinor}
                            bucketLinks={bucketSnap.bucketLinks}
                            currencyCode={consolidationCurrency?.code ?? bucketSnap.currencyCode}
                            minorUnits={consolidationCurrency?.minorUnits ?? bucketSnap.currencyMinorUnits}
                            manualCurrencyCode={bucketSnap.currencyCode}
                            manualMinorUnits={bucketSnap.currencyMinorUnits}
                            className={cn('text-sm font-bold tabular-nums', bucketSnap.convertedBalanceMinor < 0 && 'text-destructive')}
                          />
                        ) : (
                          <NumberValue
                            value={ev.amountMinor}
                            currencyCode={ev.currencyCode}
                            minorUnits={ev.currencyMinorUnits}
                            className={cn('text-sm font-bold tabular-nums', ev.amountMinor < 0 && 'text-destructive')}
                          />
                        )}
                        {ev.originalCurrencyCode && ev.originalCurrencyCode !== ev.currencyCode && ev.originalAmountMinor !== null && ev.originalCurrencyMinorUnits !== null && (
                          <NumberValue
                            value={ev.originalAmountMinor}
                            currencyCode={ev.originalCurrencyCode}
                            minorUnits={ev.originalCurrencyMinorUnits}
                            className="text-xs text-muted-foreground tabular-nums"
                          />
                        )}
                      </div>
                      <div className="flex items-center gap-1">
                        {(ev.eventType === 'balance_update' || ev.eventType === 'transfer') && (
                          <Button variant="ghost" size="icon" className="h-8 w-8" onClick={() => onEditEvent(ev)}>
                            <Pencil className="h-4 w-4" />
                          </Button>
                        )}
                        <Button
                          variant="ghost"
                          size="icon"
                          className="h-8 w-8"
                          onClick={() => {
                            if (ev.linkedEventId !== null && onDeleteTransferEvent) {
                              onDeleteTransferEvent(ev.id, ev.linkedEventId)
                            } else {
                              onDeleteEvent(ev.id)
                            }
                          }}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                    </div>
                  </CardContent>
                </Card>
              )
            })}
          </div>
        </div>
      ))}
    </div>
  )
}

export default LedgerEventList
