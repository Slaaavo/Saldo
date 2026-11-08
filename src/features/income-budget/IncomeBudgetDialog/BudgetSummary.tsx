import { Trans, useTranslation } from 'react-i18next'
import type { IncomeBudgetPreview, MonthKey } from '@/shared/types'
import { cn } from '@/shared/lib/utils'
import NumberValue from '@/shared/ui/NumberValue'
import { formatDisplayDate } from '@/shared/utils/format'
import { effectOf, type Effect } from '../effect'
import { monthName } from '../month'

interface Money {
  code: string
  minorUnits: number
}

interface Props {
  preview: IncomeBudgetPreview | undefined
  bucketCurrency: Money
  consolidationCurrency: Money
  enabled: boolean
  moment: MonthKey
  day: string
}

const valueTone = (valueMinor: number, enabled: boolean) => {
  if (!enabled) return 'text-muted-foreground'
  if (valueMinor < 0) return 'text-destructive'
  return valueMinor > 0 ? 'text-success' : undefined
}

const EffectSentence = ({ effect, bucketCurrency, consolidationCurrency, moment }: { effect: Effect; bucketCurrency: Money; consolidationCurrency: Money; moment: MonthKey }) => {
  const { t, i18n } = useTranslation()
  switch (effect.kind) {
    case 'off':
      return t('incomeBudget.effect.off')
    case 'noRate':
      return t('incomeBudget.effect.noRate', { currency: bucketCurrency.code })
    case 'unchanged':
      return t('incomeBudget.effect.unchanged')
    case 'raises':
    case 'lowers': {
      const amount = (
        <NumberValue value={effect.amountMinor} currencyCode={consolidationCurrency.code} minorUnits={consolidationCurrency.minorUnits} className="font-semibold text-foreground" />
      )
      return (
        <>
          <Trans i18nKey={`incomeBudget.effect.${effect.kind}`} components={{ amount }} />
          {effect.kind === 'raises' && effect.currentMonthPending && ` ${t('incomeBudget.effect.currentMonthPending', { month: monthName(moment, i18n.language) })}`}
        </>
      )
    }
  }
}

export const BudgetSummary = ({ preview, bucketCurrency, consolidationCurrency, enabled, moment, day }: Props) => {
  const { t, i18n } = useTranslation()
  return (
    <div className="flex flex-col gap-0.5">
      <div className="h-8">
        {preview && (
          <NumberValue
            value={preview.valueMinor}
            currencyCode={bucketCurrency.code}
            minorUnits={bucketCurrency.minorUnits}
            className={cn('text-2xl font-bold tabular-nums', valueTone(preview.valueMinor, enabled))}
          />
        )}
      </div>
      <span className="text-xs text-muted-foreground">{t('incomeBudget.summary.on', { date: formatDisplayDate(day, i18n.language) })}</span>
      <p className="mt-1 min-h-10 max-w-[64ch] text-sm text-muted-foreground">
        {preview && (
          <EffectSentence effect={effectOf(preview, { enabled, moment })} bucketCurrency={bucketCurrency} consolidationCurrency={consolidationCurrency} moment={moment} />
        )}
      </p>
    </div>
  )
}
