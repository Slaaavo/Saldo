import { useTranslation } from 'react-i18next'
import type { SnapshotRow, Currency } from '@/shared/types'
import NumberValue from '@/shared/ui/NumberValue'
import BucketAmountWithTooltip from '../../buckets/BucketAmountWithTooltip'
import { cn } from '@/shared/lib/utils'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/shared/ui/tooltip'

interface BalanceDisplayProps {
  row: SnapshotRow
  consolidationCurrency: Currency | null | undefined
  allAccounts?: SnapshotRow[]
}

export const BalanceDisplay = ({ row, consolidationCurrency, allAccounts }: BalanceDisplayProps) => {
  const { t } = useTranslation()

  if (row.accountType === 'bucket') {
    return (
      <BucketAmountWithTooltip
        totalMinor={row.convertedBalanceMinor}
        manualBalanceMinor={row.balanceMinor}
        bucketLinks={row.bucketLinks}
        currencyCode={consolidationCurrency?.code ?? row.currencyCode}
        minorUnits={consolidationCurrency?.minorUnits ?? row.currencyMinorUnits}
        manualCurrencyCode={row.currencyCode}
        manualMinorUnits={row.currencyMinorUnits}
        allAccounts={allAccounts}
        consolidationCurrencyCode={consolidationCurrency?.code ?? row.currencyCode}
        cashflowTaggedMinor={row.cashflowTaggedMinor}
        className={cn('text-2xl font-bold', row.convertedBalanceMinor < 0 && 'text-destructive')}
      />
    )
  }

  if (row.isCustom && consolidationCurrency) {
    return (
      <TooltipProvider delayDuration={300}>
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="cursor-help inline-block">
              <NumberValue
                value={row.convertedBalanceMinor}
                currencyCode={consolidationCurrency.code}
                minorUnits={consolidationCurrency.minorUnits}
                className={cn('text-2xl font-bold', row.convertedBalanceMinor < 0 && 'text-destructive')}
              />
            </span>
          </TooltipTrigger>
          <TooltipContent>
            <p className="text-xs flex flex-wrap items-center gap-1">
              <NumberValue value={row.balanceMinor} minorUnits={row.currencyMinorUnits} currencyCode={row.currencyCode} />
              <span>{row.currencyCode} = </span>
              <NumberValue value={row.convertedBalanceMinor} minorUnits={consolidationCurrency.minorUnits} currencyCode={consolidationCurrency.code} />
            </p>
            {row.fxRateMissing && <p className="text-xs text-muted-foreground mt-1">{t('accounts.fxRateMissingTooltip')}</p>}
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>
    )
  }

  if (consolidationCurrency && row.currencyCode !== consolidationCurrency.code) {
    return (
      <TooltipProvider delayDuration={300}>
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="cursor-help inline-block">
              <NumberValue
                value={row.balanceMinor}
                currencyCode={row.currencyCode}
                minorUnits={row.currencyMinorUnits}
                className={cn('text-2xl font-bold', row.balanceMinor < 0 && 'text-destructive')}
              />
            </span>
          </TooltipTrigger>
          <TooltipContent>
            <p className="text-xs">
              <span>≈ </span>
              <NumberValue value={row.convertedBalanceMinor} minorUnits={consolidationCurrency.minorUnits} currencyCode={consolidationCurrency.code} />
            </p>
            {row.fxRateMissing && <p className="text-xs text-muted-foreground mt-1">{t('accounts.fxRateMissingTooltip')}</p>}
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>
    )
  }

  return (
    <NumberValue
      value={row.balanceMinor}
      currencyCode={row.currencyCode}
      minorUnits={row.currencyMinorUnits}
      className={cn('text-2xl font-bold', row.balanceMinor < 0 && 'text-destructive')}
    />
  )
}
