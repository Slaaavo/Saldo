import { useTranslation } from 'react-i18next';
import type { BucketAllocation } from '../../shared/types';
import NumberValue from '../../shared/ui/NumberValue';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../../shared/ui/tooltip';
import { cn } from '@/shared/lib/utils';

interface Props {
  totalMinor: number;
  manualBalanceMinor: number;
  allocations: BucketAllocation[];
  currencyCode: string;
  minorUnits: number;
  manualCurrencyCode: string;
  manualMinorUnits: number;
  className?: string;
}

export default function BucketAmountWithTooltip({
  totalMinor,
  manualBalanceMinor,
  allocations,
  currencyCode,
  minorUnits,
  manualCurrencyCode,
  manualMinorUnits,
  className,
}: Props) {
  const { t } = useTranslation();

  const amount = (
    <NumberValue
      value={totalMinor}
      currencyCode={currencyCode}
      minorUnits={minorUnits}
      className={className}
    />
  );

  if (allocations.length === 0) {
    return amount;
  }

  return (
    <TooltipProvider delayDuration={300}>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className={cn('cursor-help', className && 'inline-block')}>
            <NumberValue
              value={totalMinor}
              currencyCode={currencyCode}
              minorUnits={minorUnits}
              className={className}
            />
          </span>
        </TooltipTrigger>
        <TooltipContent className="min-w-[200px]">
          <p className="mb-2 font-medium text-xs text-muted-foreground">
            {t('buckets.balanceBreakdown')}
          </p>
          <div className="flex flex-col gap-1">
            {allocations.map((alloc) => (
              <div key={alloc.id} className="flex items-center justify-between gap-4 text-xs">
                <span className="text-muted-foreground truncate max-w-[120px]">
                  {alloc.sourceAccountName}
                </span>
                <NumberValue
                  value={alloc.amountMinor}
                  currencyCode={alloc.sourceCurrencyCode}
                  minorUnits={alloc.sourceCurrencyMinorUnits}
                  className="shrink-0"
                />
              </div>
            ))}
            <div className="my-1 border-t border-border" />
            <div className="flex items-center justify-between gap-4 text-xs">
              <span className="text-muted-foreground">{t('buckets.additionalBalance')}</span>
              <NumberValue
                value={manualBalanceMinor}
                currencyCode={manualCurrencyCode}
                minorUnits={manualMinorUnits}
                className="shrink-0"
              />
            </div>
          </div>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}
