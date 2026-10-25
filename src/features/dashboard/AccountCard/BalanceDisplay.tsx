import { useTranslation } from 'react-i18next'
import type { SnapshotRow, Currency } from '@/shared/types'
import NumberValue from '@/shared/ui/NumberValue'
import BucketAmountWithTooltip from '../../buckets/BucketAmountWithTooltip'
import { cn } from '@/shared/lib/utils'
import { formatDisplayDate } from '@/shared/utils/format'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/shared/ui/tooltip'

interface BalanceDisplayProps {
  row: SnapshotRow
  consolidationCurrency: Currency | null | undefined
}

export const BalanceDisplay = ({ row, consolidationCurrency }: BalanceDisplayProps) => {
  const { t, i18n } = useTranslation()
  const { valuation } = row

  if (row.accountType === 'bucket') {
    return <BucketAmountWithTooltip row={row} currency={consolidationCurrency} className={cn('text-2xl font-bold', row.countedMinor < 0 && 'text-destructive')} />
  }

  const native = (
    <NumberValue
      value={row.balanceMinor}
      currencyCode={row.currencyCode}
      minorUnits={row.currencyMinorUnits}
      className={cn('text-2xl font-bold', row.balanceMinor < 0 && 'text-destructive')}
    />
  )

  if (valuation.kind === 'noRate') {
    return (
      <div className="flex flex-col">
        {native}
        <span className="text-xs text-amber-700 dark:text-amber-400">{t('accounts.noRateTooltip')}</span>
      </div>
    )
  }

  if (valuation.kind === 'sameCurrency' || !consolidationCurrency) {
    return native
  }

  const { rate } = valuation
  const converted = <NumberValue value={valuation.valueMinor} minorUnits={consolidationCurrency.minorUnits} currencyCode={consolidationCurrency.code} />

  return (
    <TooltipProvider delayDuration={300}>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="cursor-help inline-block">
            {row.isCustom ? (
              <NumberValue
                value={valuation.valueMinor}
                currencyCode={consolidationCurrency.code}
                minorUnits={consolidationCurrency.minorUnits}
                className={cn('text-2xl font-bold', valuation.valueMinor < 0 && 'text-destructive')}
              />
            ) : (
              native
            )}
          </span>
        </TooltipTrigger>
        <TooltipContent>
          {row.isCustom ? (
            <p className="text-xs flex flex-wrap items-center gap-1">
              <NumberValue value={row.balanceMinor} minorUnits={row.currencyMinorUnits} currencyCode={row.currencyCode} />
              <span>=</span>
              {converted}
            </p>
          ) : (
            <p className="text-xs">
              <span>≈ </span>
              {converted}
            </p>
          )}
          {rate.stale && (
            <p className="text-xs text-muted-foreground mt-1">{t('accounts.staleRateTooltip', { date: formatDisplayDate(rate.date, i18n.language), count: rate.ageDays })}</p>
          )}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}
