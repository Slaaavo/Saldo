import { useTranslation } from 'react-i18next'
import type { Currency, Metric } from '../../shared/types'
import { cn } from '@/shared/lib/utils'
import NumberValue from '../../shared/ui/NumberValue'

interface Props {
  metric: Metric
  currency: Currency | null
  onOpen: () => void
}

const MetricCard = ({ metric, currency, onOpen }: Props) => {
  const { t } = useTranslation()

  return (
    <button
      type="button"
      aria-haspopup="dialog"
      title={t('metrics.openBreakdown')}
      onClick={onOpen}
      className="flex flex-col items-center rounded-lg py-2 transition-colors hover:bg-muted/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background"
    >
      <span className="text-sm font-medium uppercase tracking-widest text-muted-foreground mb-1">{t(`metrics.${metric.kind}`)}</span>
      <span className={cn('text-4xl font-extrabold', metric.totalMinor < 0 && 'text-destructive')}>
        <NumberValue value={metric.totalMinor} currencyCode={currency?.code} minorUnits={currency?.minorUnits ?? 2} />
      </span>
    </button>
  )
}

export default MetricCard
