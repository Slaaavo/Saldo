import { useTranslation } from 'react-i18next'
import type { BucketLink, SnapshotRow } from '../../shared/types'
import NumberValue from '../../shared/ui/NumberValue'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../../shared/ui/tooltip'
import { cn } from '@/shared/lib/utils'

interface Props {
  totalMinor: number
  manualBalanceMinor: number
  bucketLinks: BucketLink[]
  currencyCode: string
  minorUnits: number
  manualCurrencyCode: string
  manualMinorUnits: number
  className?: string
  allAccounts?: SnapshotRow[]
  consolidationCurrencyCode?: string
  cashflowTaggedMinor?: number
}

const BucketAmountWithTooltip = ({
  totalMinor,
  manualBalanceMinor,
  bucketLinks,
  currencyCode,
  minorUnits,
  manualCurrencyCode,
  manualMinorUnits,
  className,
  allAccounts,
  consolidationCurrencyCode,
  cashflowTaggedMinor,
}: Props) => {
  const { t } = useTranslation()

  const amount = <NumberValue value={totalMinor} currencyCode={currencyCode} minorUnits={minorUnits} className={className} />

  if (bucketLinks.length === 0 && !cashflowTaggedMinor) {
    return amount
  }

  return (
    <TooltipProvider delayDuration={300}>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className={cn('cursor-help', className && 'inline-block')}>
            <NumberValue value={totalMinor} currencyCode={currencyCode} minorUnits={minorUnits} className={className} />
          </span>
        </TooltipTrigger>
        <TooltipContent className="min-w-[200px]">
          <p className="mb-2 font-medium text-xs text-muted-foreground">{t('buckets.balanceBreakdown')}</p>
          <div className="flex flex-col gap-1">
            {bucketLinks.map((link) => {
              const sourceAccount = allAccounts?.find((a) => a.accountId === link.sourceAccountId)
              return (
                <div key={link.id} className="flex justify-between gap-4">
                  <span className="text-muted-foreground italic text-xs">{link.sourceAccountName}</span>
                  <NumberValue value={sourceAccount?.convertedBalanceMinor ?? 0} currencyCode={consolidationCurrencyCode} minorUnits={minorUnits} />
                </div>
              )
            })}
            <div className="my-1 border-t border-border" />
            <div className="flex items-center justify-between gap-4 text-xs">
              <span className="text-muted-foreground">{t('buckets.additionalBalance')}</span>
              <NumberValue value={manualBalanceMinor} currencyCode={manualCurrencyCode} minorUnits={manualMinorUnits} className="shrink-0" />
            </div>
            {!!cashflowTaggedMinor && (
              <div className="flex items-center justify-between gap-4 text-xs">
                <span className="text-muted-foreground">{t('buckets.cashflowTaggedBalance')}</span>
                <NumberValue value={cashflowTaggedMinor} currencyCode={consolidationCurrencyCode} minorUnits={minorUnits} className="shrink-0" />
              </div>
            )}
          </div>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

export default BucketAmountWithTooltip
