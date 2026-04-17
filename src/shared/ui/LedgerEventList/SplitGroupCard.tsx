import { useTranslation } from 'react-i18next'
import type { EventWithData } from '../../types'
import type { SplitGroupRow } from '../splitGroupUtils'
import { cn } from '@/shared/lib/utils'
import NumberValue from '../NumberValue'
import { Button } from '../button'
import { Card, CardContent } from '../card'
import { Pencil, Trash2, ChevronDown } from 'lucide-react'
import EventTypeBadge from './EventTypeBadge'
import TaxMetadataLines from './TaxMetadataLines'
import LinkedCashflowIndicator from './LinkedCashflowIndicator'

interface Props {
  item: SplitGroupRow
  isExpanded: boolean
  onToggle: () => void
  onEditTaxableSplitGroup?: (splitGroupId: number, eventType: string, legs: EventWithData[], groupNote: string | null, accountId: number) => void
  onDeleteSplitGroup?: (splitGroupId: number) => void
}

const SplitGroupCard = ({ item, isExpanded, onToggle, onEditTaxableSplitGroup, onDeleteSplitGroup }: Props) => {
  const { t } = useTranslation()

  return (
    <div>
      <Card className="relative group">
        <CardContent className="flex items-center justify-between p-4">
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <p className="text-sm font-semibold truncate">{item.legs[0]?.accountName ?? ''}</p>
              <span className="inline-flex items-center gap-1 rounded-full bg-purple-100 px-2 py-0.5 text-xs font-medium text-purple-700 dark:bg-purple-900/30 dark:text-purple-400">
                {t('ledger.splitGroup.badge', { n: item.legCount })}
              </span>
              {(item.legs[0]?.eventType === 'revenue' || item.legs[0]?.eventType === 'expense') && <EventTypeBadge eventType={item.legs[0].eventType} />}
              {item.hasLinkedCashflows && <LinkedCashflowIndicator tooltip={t('taxable.linkedCount', { count: item.linkedCashflowCount })} />}
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
              {onEditTaxableSplitGroup && (item.legs[0]?.eventType === 'revenue' || item.legs[0]?.eventType === 'expense') && (
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8"
                  onClick={() => onEditTaxableSplitGroup(item.splitGroupId, item.legs[0].eventType, item.legs, item.groupNote, item.accountId)}
                >
                  <Pencil className="h-4 w-4" />
                </Button>
              )}
              <Button variant="ghost" size="icon" className="h-8 w-8" onClick={() => onDeleteSplitGroup?.(item.splitGroupId)}>
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </CardContent>
        <button
          className="absolute bottom-0 left-1/2 -translate-x-1/2 translate-y-1/2 flex items-center justify-center h-5 w-5 rounded-full bg-background border border-border text-muted-foreground hover:text-foreground opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer z-10"
          onClick={onToggle}
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
                  <EventTypeBadge eventType={leg.eventType} />
                </div>
                {leg.counterpartAccountName && (
                  <p className="text-xs text-muted-foreground truncate">
                    {leg.eventType === 'transfer' ? '↔' : '→'} {leg.counterpartAccountName}
                  </p>
                )}
                {leg.note && <p className="text-xs text-muted-foreground italic truncate">{leg.note}</p>}
                <TaxMetadataLines
                  eventType={leg.eventType}
                  vatRateBps={leg.vatRateBps}
                  vatReclaimablePctBps={leg.vatReclaimablePctBps}
                  expenseDeductiblePctBps={leg.expenseDeductiblePctBps}
                  prepaidUntil={leg.prepaidUntil}
                  reclaimedVat={leg.reclaimedVat}
                />
                {leg.bucketName && <span className="inline-flex items-center rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground mt-0.5">{leg.bucketName}</span>}
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

export default SplitGroupCard
