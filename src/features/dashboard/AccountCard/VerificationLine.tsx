import { Trans, useTranslation } from 'react-i18next'
import { Check } from 'lucide-react'
import type { SnapshotRow } from '@/shared/types'
import NumberValue from '@/shared/ui/NumberValue'
import { formatShortDay } from '@/shared/utils/format'

interface Props {
  row: SnapshotRow
}

export const VerificationLine = ({ row }: Props) => {
  const { t, i18n } = useTranslation()
  const { verification } = row
  if (!verification) return null

  const date = formatShortDay(verification.day, i18n.language)

  if (verification.state === 'verified') {
    return (
      <span className="flex items-center gap-1 text-xs text-success">
        <Check className="h-3 w-3 shrink-0" aria-hidden="true" />
        {t('bankCheck.verified', { date })}
      </span>
    )
  }

  const amount = <NumberValue value={Math.abs(verification.differenceMinor)} currencyCode={row.currencyCode} minorUnits={row.currencyMinorUnits} />
  const stillVerifiedTo = verification.lastVerifiedOn ? t('bankCheck.stillVerifiedTo', { date: formatShortDay(verification.lastVerifiedOn, i18n.language) }) : undefined

  return (
    <span className="text-xs text-amber-700 dark:text-amber-400" title={stillVerifiedTo}>
      <Trans i18nKey="bankCheck.differs" values={{ date }} components={{ amount }} />
    </span>
  )
}
