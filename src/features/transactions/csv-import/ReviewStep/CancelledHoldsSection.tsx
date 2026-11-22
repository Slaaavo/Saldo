import { useTranslation } from 'react-i18next'
import type { CancelledHoldRow } from '../types'
import { cn } from '../../../../shared/lib/utils'
import { Checkbox } from '../../../../shared/ui/checkbox'
import NumberValue from '../../../../shared/ui/NumberValue'

interface CancelledHoldsSectionProps {
  holds: CancelledHoldRow[]
  currencyCode: string
  minorUnits: number
  onToggle: (eventId: number) => void
}

export const CancelledHoldsSection = ({ holds, currencyCode, minorUnits, onToggle }: CancelledHoldsSectionProps) => {
  const { t } = useTranslation()

  return (
    <section className="flex flex-col gap-2" aria-labelledby="cancelled-holds-title">
      <div>
        <p id="cancelled-holds-title" className="text-sm font-medium">
          {t('import.reviewStep.cancelledHolds.title')}
        </p>
        <p className="text-sm text-muted-foreground">{t('import.reviewStep.cancelledHolds.description')}</p>
      </div>
      <ul className="border border-border rounded-[var(--radius)] bg-background divide-y divide-border">
        {holds.map((hold) => {
          const label = t('import.reviewStep.cancelledHolds.remove', { day: hold.eventDate.substring(0, 10) })
          return (
            <li key={hold.eventId} className={cn('flex items-center gap-3 px-2 py-2 text-sm', !hold.remove && 'opacity-50')}>
              <Checkbox checked={hold.remove} onCheckedChange={() => onToggle(hold.eventId)} aria-label={label} />
              <span className="w-24 whitespace-nowrap">{hold.eventDate.substring(0, 10)}</span>
              <NumberValue className="w-32 text-right whitespace-nowrap" value={hold.amountMinor} minorUnits={minorUnits} currencyCode={currencyCode} />
              <span className="flex-1 truncate text-muted-foreground" title={hold.note ?? ''}>
                {hold.note ?? '—'}
              </span>
            </li>
          )
        })}
      </ul>
    </section>
  )
}
