import { useTranslation } from 'react-i18next'
import type { SnapshotRow, Currency } from '@/shared/types'
import NumberValue from '@/shared/ui/NumberValue'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/shared/ui/tooltip'

interface EquityTooltipProps {
  row: SnapshotRow
  equityMinor: number
  equityLinkedRows: SnapshotRow[]
  consolidationCurrency: Currency
}

export const EquityTooltip = ({ row, equityMinor, equityLinkedRows, consolidationCurrency }: EquityTooltipProps) => {
  const { t } = useTranslation()

  return (
    <TooltipProvider delayDuration={300}>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="cursor-help mt-1 text-xs text-muted-foreground inline-flex items-center gap-1">
            {t('assets.equity')}:{' '}
            <NumberValue value={equityMinor} currencyCode={consolidationCurrency.code} minorUnits={consolidationCurrency.minorUnits} className="font-medium text-foreground" />
          </span>
        </TooltipTrigger>
        <TooltipContent>
          <div className="text-xs space-y-1 min-w-[160px]">
            <div className="flex justify-between gap-4">
              <span>{row.accountName}</span>
              <NumberValue value={row.convertedBalanceMinor} currencyCode={consolidationCurrency.code} minorUnits={consolidationCurrency.minorUnits} />
            </div>
            {equityLinkedRows.map((a) => (
              <div key={a.accountId} className="flex justify-between gap-4">
                <span className="text-muted-foreground">{a.accountName}</span>
                <NumberValue value={a.convertedBalanceMinor} currencyCode={consolidationCurrency.code} minorUnits={consolidationCurrency.minorUnits} />
              </div>
            ))}
            <div className="flex justify-between gap-4 border-t border-border pt-1 font-medium">
              <span>{t('assets.equity')}</span>
              <NumberValue value={equityMinor} currencyCode={consolidationCurrency.code} minorUnits={consolidationCurrency.minorUnits} />
            </div>
          </div>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}
