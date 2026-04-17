import { useTranslation } from 'react-i18next'
import type { EventWithData, SnapshotRow, Currency } from '../../types'
import { cn } from '@/shared/lib/utils'
import NumberValue from '../NumberValue'
import BucketAmountWithTooltip from '../../../features/buckets/BucketAmountWithTooltip'
import { Button } from '../button'
import { Card, CardContent } from '../card'
import { Pencil, Trash2 } from 'lucide-react'
import EventTypeBadge from './EventTypeBadge'
import TaxMetadataLines from './TaxMetadataLines'
import LinkedCashflowIndicator from './LinkedCashflowIndicator'

interface Props {
  ev: EventWithData
  bucketSnap?: SnapshotRow
  consolidationCurrency?: Currency | null
  onEditEvent: (event: EventWithData) => void
  onDeleteEvent: (eventId: number, eventType?: string) => void
  onDeleteTransferEvent?: (eventId: number, linkedEventId: number) => void
}

const StandaloneEventCard = ({ ev, bucketSnap, consolidationCurrency, onEditEvent, onDeleteEvent, onDeleteTransferEvent }: Props) => {
  const { t } = useTranslation()

  return (
    <Card>
      <CardContent className="flex items-center justify-between p-4">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <p className="text-sm font-semibold truncate">{ev.accountName}</p>
            <EventTypeBadge eventType={ev.eventType} />
            {ev.eventType === 'cashflow' && ev.isLinkedToTaxable && <LinkedCashflowIndicator tooltip={t('taxable.linked')} />}
            {(ev.eventType === 'revenue' || ev.eventType === 'expense') && ev.hasLinkedCashflows && (
              <LinkedCashflowIndicator tooltip={t('taxable.linkedCount', { count: ev.linkedCashflowCount })} />
            )}
            {ev.isSystemGenerated && (
              <span className="inline-flex items-center rounded-full bg-gray-100 px-2 py-0.5 text-xs font-medium text-gray-600 dark:bg-gray-800/50 dark:text-gray-400">
                {t('assets.systemGenerated')}
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
          <TaxMetadataLines
            eventType={ev.eventType}
            vatRateBps={ev.vatRateBps}
            vatReclaimablePctBps={ev.vatReclaimablePctBps}
            expenseDeductiblePctBps={ev.expenseDeductiblePctBps}
            prepaidUntil={ev.prepaidUntil}
            reclaimedVat={ev.reclaimedVat}
          />
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
            {!ev.isSystemGenerated &&
              (ev.eventType === 'balance_update' ||
                ev.eventType === 'transfer' ||
                ev.eventType === 'revenue' ||
                ev.eventType === 'expense' ||
                ev.eventType === 'prepaid_expense') && (
                <Button variant="ghost" size="icon" className="h-8 w-8" onClick={() => onEditEvent(ev)}>
                  <Pencil className="h-4 w-4" />
                </Button>
              )}
            {!ev.isSystemGenerated && (
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                onClick={() => {
                  if (ev.linkedEventId !== null && onDeleteTransferEvent) {
                    onDeleteTransferEvent(ev.id, ev.linkedEventId)
                  } else {
                    onDeleteEvent(ev.id, ev.eventType)
                  }
                }}
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

export default StandaloneEventCard
