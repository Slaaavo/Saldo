import { useTranslation } from 'react-i18next'
import type { Currency, SnapshotRow, Valuation } from '../../shared/types'
import NumberValue from '../../shared/ui/NumberValue'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../../shared/ui/tooltip'
import { cn } from '@/shared/lib/utils'
import { valueOf } from '../../shared/utils/snapshot'

interface Props {
  row: SnapshotRow
  currency: Currency | null | undefined
  className?: string
}

const BucketAmountWithTooltip = ({ row, currency, className }: Props) => {
  const { t } = useTranslation()

  const currencyCode = currency?.code ?? row.currencyCode
  const minorUnits = currency?.minorUnits ?? row.currencyMinorUnits
  const tagged = row.taggedCashflows.filter((c) => c.amountMinor !== 0)

  const amount = <NumberValue value={row.countedMinor} currencyCode={currencyCode} minorUnits={minorUnits} className={className} />

  if (row.bucketLinks.length === 0 && tagged.length === 0) {
    return amount
  }

  const counted = (valuation: Valuation) => {
    const value = valueOf(valuation)
    return value === null ? (
      <span className="text-xs text-muted-foreground">{t('buckets.notCounted')}</span>
    ) : (
      <NumberValue value={value} currencyCode={currencyCode} minorUnits={minorUnits} className="shrink-0" />
    )
  }

  return (
    <TooltipProvider delayDuration={300}>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className={cn('cursor-help', className && 'inline-block')}>{amount}</span>
        </TooltipTrigger>
        <TooltipContent className="min-w-[200px]">
          <p className="mb-2 font-medium text-xs text-muted-foreground">{t('buckets.balanceBreakdown')}</p>
          <div className="flex flex-col gap-1">
            {row.bucketLinks.map((link) => (
              <div key={link.id} className="flex justify-between gap-4">
                <span className="text-muted-foreground italic text-xs">{link.sourceAccountName}</span>
                {counted(link.valuation)}
              </div>
            ))}
            <div className="my-1 border-t border-border" />
            <div className="flex items-center justify-between gap-4 text-xs">
              <span className="text-muted-foreground">{t('buckets.additionalBalance')}</span>
              <span className="flex items-center gap-2">
                {row.valuation.kind === 'noRate' && <span className="text-muted-foreground">{t('buckets.notCounted')}</span>}
                <NumberValue value={row.balanceMinor} currencyCode={row.currencyCode} minorUnits={row.currencyMinorUnits} className="shrink-0" />
              </span>
            </div>
            {tagged.map((cashflows) => (
              <div key={cashflows.sourceAccountId} className="flex items-center justify-between gap-4 text-xs">
                <span className="text-muted-foreground">{`${t('buckets.cashflowTaggedBalance')}: ${cashflows.sourceAccountName}`}</span>
                {counted(cashflows.valuation)}
              </div>
            ))}
          </div>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

export default BucketAmountWithTooltip
